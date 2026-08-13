use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::repository::{RecordRepository, StoredRecord, write_atomic};
use crate::{
    DestructionAcknowledgement, Error, Record, RecordId, RecordStatus, StorePaths, StoreScope,
    parse_markdown, render_markdown,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExportVerificationReport {
    pub format_version: u32,
    pub source_scope: String,
    pub records: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImportPreviewEntry {
    pub source_id: String,
    pub target_id: String,
    pub scope: String,
    pub destination: String,
    pub action: String,
    pub equivalent_record_id: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ImportPreview {
    pub report: ImportReport,
    pub records: Vec<ImportPreviewEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoreDestructionPreview {
    pub store_id: String,
    pub scope: String,
    pub root: String,
    pub store_root_bytes: u64,
    pub records: usize,
    pub canonical_bytes: u64,
    pub disposable_bytes: u64,
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
    let records = repository.list_read_only(true)?;
    export_records(paths, records)
}

fn export_records(paths: &StorePaths, records: Vec<StoredRecord>) -> crate::Result<ExportBundle> {
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
        source_scope: expected_scope(paths)?,
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

pub fn verify_export(bundle: &ExportBundle) -> crate::Result<ExportVerificationReport> {
    validate_bundle_header(bundle)?;
    let mut ids = HashSet::new();
    for (index, item) in bundle.records.iter().enumerate() {
        validate_archive_path(index, &item.path)?;
        let record = parse_archive_record(index, item)?;
        record.validate_provenance().map_err(|error| {
            Error::backup(BackupError::InvalidArchiveRecord {
                index,
                message: error.to_string(),
            })
        })?;
        if !ids.insert(record.id) {
            return Err(Error::backup(BackupError::DuplicateArchiveId {
                id: record.id,
            }));
        }
    }
    Ok(ExportVerificationReport {
        format_version: bundle.format_version,
        source_scope: bundle.source_scope.clone(),
        records: bundle.records.len(),
    })
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
    let repository = RecordRepository::new(paths.clone());
    let _lock = repository.prepare_mutation()?;
    let existing = repository.scan_locked()?;
    let (plans, preview) = plan_import(paths, bundle, options, &existing)?;
    for plan in plans {
        write_atomic(&plan.destination, plan.markdown.as_bytes())?;
    }
    Ok(preview.report)
}

pub fn preview_import(
    paths: &StorePaths,
    bundle: &ExportBundle,
    options: &ImportOptions,
) -> crate::Result<ImportPreview> {
    let repository = RecordRepository::new(paths.clone());
    let existing = repository.list_read_only(true)?;
    plan_import(paths, bundle, options, &existing).map(|(_, preview)| preview)
}

fn plan_import(
    paths: &StorePaths,
    bundle: &ExportBundle,
    options: &ImportOptions,
    existing: &[StoredRecord],
) -> crate::Result<(Vec<ImportPlan>, ImportPreview)> {
    verify_export(bundle)?;
    let by_id: HashMap<_, _> = existing
        .iter()
        .map(|stored| (stored.record().id, stored.clone()))
        .collect();
    let expected_scope = expected_scope(paths)?;
    let mut archive_ids = HashSet::new();
    let mut plans = Vec::new();
    let mut preview = ImportPreview::default();

    for (index, item) in bundle.records.iter().enumerate() {
        let mut record = parse_archive_record(index, item)?;
        if !archive_ids.insert(record.id) {
            return Err(Error::backup(BackupError::DuplicateArchiveId {
                id: record.id,
            }));
        }

        let mut changed = false;
        if record.scope.as_str() != expected_scope {
            match options.scope_collision {
                Some(ScopeCollisionPolicy::Skip) => {
                    add_skipped_preview(&mut preview, &record, "scope_collision", None);
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
        let equivalent_record_id =
            existing_identity(existing, &record).map(|stored| stored.record().id);
        if let Some(current) = by_id.get(&record.id) {
            if !changed && current.markdown() == item.markdown {
                match options.existing_record {
                    Some(ExistingRecordPolicy::Skip) => {
                        add_skipped_preview(
                            &mut preview,
                            &record,
                            "existing_record",
                            Some(current),
                        );
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
                        add_skipped_preview(&mut preview, &record, "id_collision", Some(current));
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
        } else if let Some(current) = existing_identity(existing, &record) {
            match options.existing_record {
                Some(ExistingRecordPolicy::Skip) => {
                    add_skipped_preview(&mut preview, &record, "equivalent_record", Some(current));
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
            equivalent_record_id,
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

    for plan in &plans {
        preview.report.imported += 1;
        if plan.overwritten {
            preview.report.overwritten += 1;
        }
        if plan.remapped {
            preview.report.remapped += 1;
        }
        preview.records.push(ImportPreviewEntry {
            source_id: plan.original_id.to_string(),
            target_id: plan.record.id.to_string(),
            scope: plan.record.scope.to_string(),
            destination: format!("records/{}.md", plan.record.id),
            action: if plan.overwritten {
                "overwrite"
            } else if plan.remapped {
                "remap"
            } else {
                "import"
            }
            .to_owned(),
            equivalent_record_id: plan.equivalent_record_id.map(|id| id.to_string()),
        });
    }
    Ok((plans, preview))
}

pub fn preview_store_destruction(paths: &StorePaths) -> crate::Result<StoreDestructionPreview> {
    let status = crate::inspect_store(paths)?;
    if !status.initialized {
        return Err(Error::repository(
            crate::RepositoryError::StoreNotInitialized {
                root: paths.root.clone(),
            },
        ));
    }
    let store_id = status
        .project
        .map_or_else(|| "global".to_owned(), |project| project.id.to_string());
    Ok(StoreDestructionPreview {
        store_id,
        scope: paths.scope.to_string(),
        root: paths.root.display().to_string(),
        store_root_bytes: crate::directory_bytes(&paths.root)?,
        records: status.record_count,
        canonical_bytes: status.canonical_bytes,
        disposable_bytes: status.disposable_bytes,
    })
}

pub fn destroy_store(
    paths: &StorePaths,
    expected_store_id: &str,
    _acknowledgement: DestructionAcknowledgement,
    export_destination: Option<&Path>,
) -> crate::Result<()> {
    let repository = RecordRepository::new(paths.clone());
    let _lock = repository.prepare_mutation()?;
    let preview = preview_store_destruction(paths)?;
    if expected_store_id != preview.store_id {
        return Err(Error::invalid_input(format!(
            "store id mismatch: expected `{}`, got `{expected_store_id}`",
            preview.store_id
        )));
    }
    if let Some(destination) = export_destination {
        let bundle = export_records(paths, repository.scan_locked()?)?;
        verify_export(&bundle)?;
        let encoded = encode_export(&bundle)?;
        if encoded.len() > MAX_EXPORT_ARCHIVE_BYTES {
            return Err(Error::backup(BackupError::InvalidArchive {
                message: format!("archive exceeds the {MAX_EXPORT_ARCHIVE_BYTES} byte limit"),
            }));
        }
        write_export_archive(paths, destination, encoded.as_bytes())?;
    }
    if paths.scope == StoreScope::Global {
        remove_global_projection(paths)?;
    }
    fs::remove_dir_all(&paths.root)
        .map_err(|source| Error::io("destroy the selected store", source))?;
    Ok(())
}

fn remove_global_projection(paths: &StorePaths) -> crate::Result<()> {
    let index = crate::index::existing_index_path(paths);
    for suffix in ["", "-wal", "-shm"] {
        remove_file_if_present(&PathBuf::from(format!("{}{suffix}", index.display())))?;
    }
    if let Some(parent) = index.parent() {
        remove_file_if_present(&parent.join("projection.lock"))?;
    }
    Ok(())
}

fn remove_file_if_present(path: &Path) -> crate::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::io("remove the selected store projection", source)),
    }
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
    equivalent_record_id: Option<RecordId>,
}

fn parse_archive_record(index: usize, item: &ExportedRecord) -> crate::Result<Record> {
    parse_markdown(Path::new("<export>"), &item.markdown).map_err(|error| {
        Error::backup(BackupError::InvalidArchiveRecord {
            index,
            message: error.to_string(),
        })
    })
}

fn add_skipped_preview(
    preview: &mut ImportPreview,
    record: &Record,
    action: &str,
    equivalent: Option<&StoredRecord>,
) {
    preview.report.skipped += 1;
    preview.records.push(ImportPreviewEntry {
        source_id: record.id.to_string(),
        target_id: record.id.to_string(),
        scope: record.scope.to_string(),
        destination: format!("records/{}.md", record.id),
        action: format!("skip_{action}"),
        equivalent_record_id: equivalent.map(|stored| stored.record().id.to_string()),
    });
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

fn expected_scope(paths: &StorePaths) -> crate::Result<String> {
    crate::record_scope(paths).map(|scope| scope.as_str().to_owned())
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
