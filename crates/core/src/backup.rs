use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::repository::{RecordRepository, StoredRecord, write_atomic};
use crate::{
    Error, Record, RecordId, RecordStatus, StorePaths, StoreScope, parse_markdown, render_markdown,
};

pub const EXPORT_FORMAT_VERSION: u32 = 1;
pub const MAX_EXPORT_ARCHIVE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdCollisionPolicy {
    Fail,
    Skip,
    Overwrite,
    Remap,
}

impl IdCollisionPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fail => "fail",
            Self::Skip => "skip",
            Self::Overwrite => "overwrite",
            Self::Remap => "remap",
        }
    }
}

impl FromStr for IdCollisionPolicy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "fail" => Ok(Self::Fail),
            "skip" => Ok(Self::Skip),
            "overwrite" => Ok(Self::Overwrite),
            "remap" => Ok(Self::Remap),
            _ => Err("must be one of fail, skip, overwrite, or remap".to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeCollisionPolicy {
    Fail,
    Skip,
    Remap,
}

impl ScopeCollisionPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fail => "fail",
            Self::Skip => "skip",
            Self::Remap => "remap",
        }
    }
}

impl FromStr for ScopeCollisionPolicy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "fail" => Ok(Self::Fail),
            "skip" => Ok(Self::Skip),
            "remap" => Ok(Self::Remap),
            _ => Err("must be one of fail, skip, or remap".to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExistingRecordPolicy {
    Fail,
    Skip,
    Overwrite,
}

impl ExistingRecordPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fail => "fail",
            Self::Skip => "skip",
            Self::Overwrite => "overwrite",
        }
    }
}

impl FromStr for ExistingRecordPolicy {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "fail" => Ok(Self::Fail),
            "skip" => Ok(Self::Skip),
            "overwrite" => Ok(Self::Overwrite),
            _ => Err("must be one of fail, skip, or overwrite".to_owned()),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ImportOptions {
    pub id_collision: Option<IdCollisionPolicy>,
    pub scope_collision: Option<ScopeCollisionPolicy>,
    pub existing_record: Option<ExistingRecordPolicy>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExportBundle {
    pub format_version: u32,
    pub source_scope: String,
    pub records: Vec<ExportedRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExportedRecord {
    /// Relative to the store root. This is metadata only; imports never trust it as a path.
    pub path: String,
    /// The canonical Markdown bytes, including TOML provenance frontmatter.
    pub markdown: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("export format version {found} is unsupported; expected {expected}")]
    UnsupportedFormatVersion { found: u32, expected: u32 },
    #[error("export archive is invalid: {message}")]
    InvalidArchive { message: String },
    #[error("export record {index} is invalid: {message}")]
    InvalidArchiveRecord { index: usize, message: String },
    #[error("archive record {index} has an unsafe path")]
    UnsafeArchivePath { index: usize },
    #[error("{collision} collision requires an explicit policy")]
    PolicyRequired { collision: &'static str },
    #[error("record id collision for {id}")]
    IdCollision { id: RecordId },
    #[error("scope collision: archive has `{actual}`, selected store requires `{expected}`")]
    ScopeCollision { actual: String, expected: String },
    #[error("archive record {imported} already exists as record {existing}")]
    ExistingRecordCollision {
        imported: RecordId,
        existing: RecordId,
    },
    #[error("archive contains duplicate record id {id}")]
    DuplicateArchiveId { id: RecordId },
    #[error("archive import would write two records to the same destination")]
    DestinationCollision,
    #[error("export destination must be outside the selected store")]
    ExportInsideStore,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ImportReport {
    pub imported: usize,
    pub skipped: usize,
    pub overwritten: usize,
    pub remapped: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GcOptions {
    pub dry_run: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GcEntry {
    /// A store-relative label. It never exposes the host's absolute path.
    pub path: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct GcReport {
    pub dry_run: bool,
    pub candidates: Vec<GcEntry>,
    pub removed: usize,
    pub reclaimed_bytes: u64,
}

pub fn export_store(paths: &StorePaths) -> crate::Result<ExportBundle> {
    let repository = RecordRepository::new(paths.clone());
    let records = repository.list(true)?;
    let mut exported = records
        .into_iter()
        .map(|stored| {
            let path = stored
                .path()
                .strip_prefix(&paths.root)
                .map_err(|_| {
                    Error::backup(BackupError::InvalidArchive {
                        message: "record is outside the selected store".to_owned(),
                    })
                })?
                .components()
                .filter_map(|component| match component {
                    Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("/");
            Ok(ExportedRecord {
                path,
                markdown: stored.markdown().to_owned(),
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    exported.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(ExportBundle {
        format_version: EXPORT_FORMAT_VERSION,
        source_scope: expected_scope(paths),
        records: exported,
    })
}

pub fn encode_export(bundle: &ExportBundle) -> crate::Result<String> {
    validate_bundle_header(bundle)?;
    serde_json::to_string_pretty(bundle)
        .map(|encoded| format!("{encoded}\n"))
        .map_err(|source| Error::invalid_input(format!("could not encode export: {source}")))
}

pub fn decode_export(contents: &str) -> crate::Result<ExportBundle> {
    if contents.len() > MAX_EXPORT_ARCHIVE_BYTES {
        return Err(Error::backup(BackupError::InvalidArchive {
            message: format!("archive exceeds the {MAX_EXPORT_ARCHIVE_BYTES} byte limit"),
        }));
    }
    let bundle: ExportBundle = serde_json::from_str(contents).map_err(|source| {
        Error::backup(BackupError::InvalidArchive {
            message: format!("invalid JSON: {source}"),
        })
    })?;
    validate_bundle_header(&bundle)?;
    Ok(bundle)
}

pub fn write_export_archive(
    paths: &StorePaths,
    destination: &Path,
    contents: &[u8],
) -> crate::Result<()> {
    let store = fs::canonicalize(&paths.root)
        .map_err(|source| Error::io("resolve the selected store", source))?;
    let resolved_destination = if destination.exists() {
        fs::canonicalize(destination)
            .map_err(|source| Error::io("resolve the export destination", source))?
    } else {
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = fs::canonicalize(parent)
            .map_err(|source| Error::io("resolve the export destination directory", source))?;
        let name = destination
            .file_name()
            .ok_or_else(|| Error::invalid_input("export destination must name a file"))?;
        parent.join(name)
    };
    if resolved_destination.starts_with(store) {
        return Err(Error::backup(BackupError::ExportInsideStore));
    }
    write_atomic(&resolved_destination, contents)
}

pub fn import_store(
    paths: &StorePaths,
    bundle: &ExportBundle,
    options: &ImportOptions,
) -> crate::Result<ImportReport> {
    validate_bundle_header(bundle)?;
    let repository = RecordRepository::new(paths.clone());
    let _lock = repository.prepare_mutation()?;
    let existing = repository.scan_locked()?;
    let by_id: HashMap<_, _> = existing
        .iter()
        .map(|stored| (stored.record().id, stored.clone()))
        .collect();
    let expected_scope = expected_scope(paths);
    let mut archive_ids = HashSet::new();
    let mut plans = Vec::new();
    let mut report = ImportReport::default();

    for (index, item) in bundle.records.iter().enumerate() {
        validate_archive_path(index, &item.path)?;
        let mut record =
            parse_markdown(Path::new("<export>"), &item.markdown).map_err(|error| {
                Error::backup(BackupError::InvalidArchiveRecord {
                    index,
                    message: error.to_string(),
                })
            })?;
        if !archive_ids.insert(record.id) {
            return Err(Error::backup(BackupError::DuplicateArchiveId {
                id: record.id,
            }));
        }

        let mut changed = false;
        if record.scope.as_str() != expected_scope {
            match options.scope_collision {
                Some(ScopeCollisionPolicy::Skip) => {
                    report.skipped += 1;
                    continue;
                }
                Some(ScopeCollisionPolicy::Remap) => {
                    record.scope =
                        crate::Scope::parse(&expected_scope).map_err(Error::invalid_input)?;
                    changed = true;
                }
                Some(ScopeCollisionPolicy::Fail) => {
                    return Err(Error::backup(BackupError::ScopeCollision {
                        actual: record.scope.to_string(),
                        expected: expected_scope.clone(),
                    }));
                }
                None => {
                    return Err(Error::backup(BackupError::PolicyRequired {
                        collision: "scope",
                    }));
                }
            }
        }

        let original_id = record.id;
        let mut target_id = original_id;
        let mut destination = paths.records.join(format!("{target_id}.md"));
        let mut overwritten = false;
        let mut remapped = false;
        if let Some(current) = by_id.get(&record.id) {
            if !changed && current.markdown() == item.markdown {
                match options.existing_record {
                    Some(ExistingRecordPolicy::Skip) => {
                        report.skipped += 1;
                        continue;
                    }
                    Some(ExistingRecordPolicy::Overwrite) => {
                        destination = current.path().to_path_buf();
                        overwritten = true;
                    }
                    Some(ExistingRecordPolicy::Fail) => {
                        return Err(Error::backup(BackupError::ExistingRecordCollision {
                            imported: record.id,
                            existing: current.record().id,
                        }));
                    }
                    None => {
                        return Err(Error::backup(BackupError::PolicyRequired {
                            collision: "existing-record",
                        }));
                    }
                }
            } else {
                match options.id_collision {
                    Some(IdCollisionPolicy::Skip) => {
                        report.skipped += 1;
                        continue;
                    }
                    Some(IdCollisionPolicy::Overwrite) => {
                        destination = current.path().to_path_buf();
                        overwritten = true;
                    }
                    Some(IdCollisionPolicy::Remap) => {
                        target_id = fresh_id(&by_id, &plans);
                        record.id = target_id;
                        destination = paths.records.join(format!("{target_id}.md"));
                        remapped = true;
                        changed = true;
                    }
                    Some(IdCollisionPolicy::Fail) => {
                        return Err(Error::backup(BackupError::IdCollision { id: record.id }));
                    }
                    None => {
                        return Err(Error::backup(BackupError::PolicyRequired {
                            collision: "id",
                        }));
                    }
                }
            }
        } else if let Some(current) = existing_identity(&existing, &record) {
            match options.existing_record {
                Some(ExistingRecordPolicy::Skip) => {
                    report.skipped += 1;
                    continue;
                }
                Some(ExistingRecordPolicy::Overwrite) => {
                    destination = current.path().to_path_buf();
                    record.id = current.record().id;
                    changed = true;
                    overwritten = true;
                }
                Some(ExistingRecordPolicy::Fail) => {
                    return Err(Error::backup(BackupError::ExistingRecordCollision {
                        imported: record.id,
                        existing: current.record().id,
                    }));
                }
                None => {
                    return Err(Error::backup(BackupError::PolicyRequired {
                        collision: "existing-record",
                    }));
                }
            }
        }

        plans.push(ImportPlan {
            record,
            markdown: item.markdown.clone(),
            destination,
            overwritten,
            remapped,
            changed,
            original_id,
        });
    }

    let remaps: HashMap<_, _> = plans
        .iter()
        .filter(|plan| plan.remapped)
        .map(|plan| (plan.original_id, plan.record.id))
        .collect();
    let mut destinations = HashSet::new();
    for plan in &mut plans {
        for superseded in &mut plan.record.supersedes {
            if let Some(remapped) = remaps.get(superseded) {
                *superseded = *remapped;
                plan.changed = true;
            }
        }
        if plan.changed {
            plan.markdown = render_markdown(&plan.record)?;
        }
        if !destinations.insert(plan.destination.clone()) {
            return Err(Error::backup(BackupError::DestinationCollision));
        }
    }

    for plan in plans {
        write_atomic(&plan.destination, plan.markdown.as_bytes())?;
        report.imported += 1;
        if plan.overwritten {
            report.overwritten += 1;
        }
        if plan.remapped {
            report.remapped += 1;
        }
    }
    Ok(report)
}

pub fn gc_store(paths: &StorePaths, options: GcOptions) -> crate::Result<GcReport> {
    let repository = RecordRepository::new(paths.clone());
    let _lock = repository.prepare_mutation()?;
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let index = crate::index_path(paths);
    add_candidate(&index, "index.sqlite3", &mut candidates, &mut seen)?;
    for suffix in ["-wal", "-shm"] {
        let path = PathBuf::from(format!("{}{}", index.display(), suffix));
        add_candidate(
            &path,
            &format!("index.sqlite3{suffix}"),
            &mut candidates,
            &mut seen,
        )?;
    }

    collect_directory(
        &paths.root.join("locks"),
        "locks",
        Some("mutation.lock"),
        &mut candidates,
        &mut seen,
    )?;
    collect_directory(
        &paths.root.join("tmp"),
        "tmp",
        None,
        &mut candidates,
        &mut seen,
    )?;
    collect_directory(
        &paths.root.join("logs"),
        "logs",
        None,
        &mut candidates,
        &mut seen,
    )?;
    collect_suffixes(&paths.root, "", ".tmp", &mut candidates, &mut seen)?;
    collect_directory(
        &paths.cache.join("models"),
        "cache/models",
        Some(".lock"),
        &mut candidates,
        &mut seen,
    )?;
    candidates.sort_by(|left, right| left.path.cmp(&right.path));

    let reclaimed_bytes = candidates.iter().map(|entry| entry.bytes).sum();
    let mut report = GcReport {
        dry_run: options.dry_run,
        candidates,
        removed: 0,
        reclaimed_bytes,
    };
    if !options.dry_run {
        for entry in &report.candidates {
            let path = gc_path(paths, &entry.path);
            fs::remove_file(&path).map_err(|source| Error::io("remove disposable data", source))?;
            report.removed += 1;
        }
    }
    Ok(report)
}

#[derive(Clone, Debug)]
struct ImportPlan {
    record: Record,
    markdown: String,
    destination: PathBuf,
    overwritten: bool,
    remapped: bool,
    changed: bool,
    original_id: RecordId,
}

fn validate_bundle_header(bundle: &ExportBundle) -> crate::Result<()> {
    if bundle.format_version != EXPORT_FORMAT_VERSION {
        return Err(Error::backup(BackupError::UnsupportedFormatVersion {
            found: bundle.format_version,
            expected: EXPORT_FORMAT_VERSION,
        }));
    }
    if crate::Scope::parse(&bundle.source_scope).is_err() {
        return Err(Error::backup(BackupError::InvalidArchive {
            message: "source_scope is invalid".to_owned(),
        }));
    }
    Ok(())
}

fn validate_archive_path(index: usize, path: &str) -> crate::Result<()> {
    let path = Path::new(path);
    let components: Vec<_> = path.components().collect();
    let safe = components.len() >= 2
        && matches!(components[0], Component::Normal(value) if value == "records")
        && components[1..]
            .iter()
            .all(|component| matches!(component, Component::Normal(_)))
        && path.extension().is_some_and(|extension| extension == "md");
    if safe {
        Ok(())
    } else {
        Err(Error::backup(BackupError::UnsafeArchivePath { index }))
    }
}

fn expected_scope(paths: &StorePaths) -> String {
    match paths.scope {
        StoreScope::Global => "global".to_owned(),
        StoreScope::Project => {
            let name = paths
                .root
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .unwrap_or("local");
            let sanitized: String = name
                .chars()
                .map(|character| {
                    if character.is_whitespace() || character == ':' || character.is_control() {
                        '-'
                    } else {
                        character
                    }
                })
                .collect();
            format!(
                "project:{}",
                if sanitized.is_empty() {
                    "local"
                } else {
                    &sanitized
                }
            )
        }
    }
}

fn existing_identity<'a>(
    existing: &'a [StoredRecord],
    record: &Record,
) -> Option<&'a StoredRecord> {
    existing.iter().find(|stored| {
        let current = stored.record();
        current.id != record.id
            && current.status != RecordStatus::Superseded
            && current.status != RecordStatus::Archived
            && current.scope == record.scope
            && current.kind == record.kind
            && normalize(&current.title) == normalize(&record.title)
            && normalize(&current.body) == normalize(&record.body)
    })
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn fresh_id(existing: &HashMap<RecordId, StoredRecord>, plans: &[ImportPlan]) -> RecordId {
    loop {
        let id = RecordId::new_v7();
        if !existing.contains_key(&id) && !plans.iter().any(|plan| plan.record.id == id) {
            return id;
        }
    }
}

fn add_candidate(
    path: &Path,
    label: &str,
    candidates: &mut Vec<GcEntry>,
    seen: &mut HashSet<PathBuf>,
) -> crate::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(Error::io("inspect disposable data", source)),
    };
    if (metadata.is_file() || metadata.file_type().is_symlink()) && seen.insert(path.to_path_buf())
    {
        candidates.push(GcEntry {
            path: label.to_owned(),
            bytes: metadata.len(),
        });
    }
    Ok(())
}

fn collect_directory(
    directory: &Path,
    label: &str,
    skip_name: Option<&str>,
    candidates: &mut Vec<GcEntry>,
    seen: &mut HashSet<PathBuf>,
) -> crate::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(Error::io("scan disposable data", source)),
    };
    for entry in entries {
        let entry = entry.map_err(|source| Error::io("scan disposable data", source))?;
        let name = entry.file_name();
        if skip_name.is_some_and(|skip| name == skip) {
            continue;
        }
        let child_label = format!("{label}/{}", name.to_string_lossy());
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|source| Error::io("inspect disposable data", source))?;
        if metadata.is_dir() {
            collect_directory(&entry.path(), &child_label, skip_name, candidates, seen)?;
        } else {
            add_candidate(&entry.path(), &child_label, candidates, seen)?;
        }
    }
    Ok(())
}

fn collect_suffixes(
    directory: &Path,
    label: &str,
    suffix: &str,
    candidates: &mut Vec<GcEntry>,
    seen: &mut HashSet<PathBuf>,
) -> crate::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(Error::io("scan disposable data", source)),
    };
    for entry in entries {
        let entry = entry.map_err(|source| Error::io("scan disposable data", source))?;
        let name = entry.file_name();
        let child_label = if label.is_empty() {
            name.to_string_lossy().into_owned()
        } else {
            format!("{label}/{}", name.to_string_lossy())
        };
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|source| Error::io("inspect disposable data", source))?;
        if metadata.is_dir() {
            collect_suffixes(&entry.path(), &child_label, suffix, candidates, seen)?;
        } else if name.to_string_lossy().ends_with(suffix) {
            add_candidate(&entry.path(), &child_label, candidates, seen)?;
        }
    }
    Ok(())
}

fn gc_path(paths: &StorePaths, label: &str) -> PathBuf {
    if label == "index.sqlite3" {
        return crate::index_path(paths);
    }
    if let Some(suffix) = label.strip_prefix("index.sqlite3") {
        return PathBuf::from(format!("{}{}", crate::index_path(paths).display(), suffix));
    }
    if let Some(relative) = label.strip_prefix("cache/") {
        return paths.cache.join(relative);
    }
    paths.root.join(label)
}
