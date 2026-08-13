use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::embedder::{LocalEmbedder, default_model_is_ready};
use crate::repository::acquire_store_mutation_lock;
use crate::{Embedder, Error, StorePaths};

use super::canonical::{collect_markdown_paths, read_canonical};
use super::projection::Index;
use super::{
    DoctorIssue, DoctorRepairReport, DoctorReport, active_index_path, content_hash, index_path,
    reindex_store_with_embedder,
};

pub fn doctor_store(paths: &StorePaths) -> crate::Result<DoctorReport> {
    let destination = active_index_path(paths)?;
    let semantic_model_ready = default_model_is_ready(paths);
    let mut report = DoctorReport {
        index_path: destination.display().to_string(),
        semantic_model_ready,
        failures: 0,
        warnings: 0,
        issues: Vec::new(),
    };
    let issue = |report: &mut DoctorReport, severity: &str, message: String, repair: &str| {
        if severity == "failure" {
            report.failures += 1;
        } else {
            report.warnings += 1;
        }
        report.issues.push(DoctorIssue {
            severity: severity.to_owned(),
            message,
            repair: repair.to_owned(),
        });
    };

    if !paths.root.join("store.toml").is_file() {
        issue(
            &mut report,
            "failure",
            "the selected store is not initialized".to_owned(),
            "run `sbuf init` (or `sbuf --project init`)",
        );
        return Ok(report);
    }
    if !paths.records.is_dir() {
        issue(
            &mut report,
            "failure",
            "the canonical records directory is missing".to_owned(),
            "restore the records directory or initialize the store again",
        );
        return Ok(report);
    }

    let expected_scope = match crate::record_scope(paths) {
        Ok(scope) => scope,
        Err(error) => {
            issue(
                &mut report,
                "failure",
                format!("canonical store metadata is invalid: {error}"),
                "repair store.toml, then run `sbuf doctor`",
            );
            return Ok(report);
        }
    };
    let canonical = match collect_markdown_paths(&paths.records) {
        Ok(paths) => paths,
        Err(error) => {
            issue(
                &mut report,
                "failure",
                format!("canonical records could not be scanned: {error}"),
                "restore access to the records directory, then run `sbuf doctor`",
            );
            return Ok(report);
        }
    };
    let mut valid = HashMap::new();
    let mut seen_ids = HashMap::new();
    for path in canonical {
        match read_canonical(&path) {
            Ok((record, markdown)) => {
                if record.scope != expected_scope {
                    issue(
                        &mut report,
                        "failure",
                        format!(
                            "canonical record {} is outside the selected store scope",
                            path.display()
                        ),
                        "correct the record scope, then run `sbuf sync`",
                    );
                    continue;
                }
                if let Some(first) = seen_ids.insert(record.id, path.clone()) {
                    issue(
                        &mut report,
                        "failure",
                        format!(
                            "canonical record {} duplicates the ID first seen at {}",
                            path.display(),
                            first.display()
                        ),
                        "give each record a unique ID, then run `sbuf sync`",
                    );
                    continue;
                }
                valid.insert(
                    path.display().to_string(),
                    (record.id.to_string(), content_hash(&markdown)),
                );
            }
            Err(error) => issue(
                &mut report,
                "failure",
                format!("canonical record {} is invalid: {error}", path.display()),
                "repair the Markdown, then run `sbuf sync`",
            ),
        }
    }

    if !destination.is_file() {
        issue(
            &mut report,
            "warning",
            "the SQLite projection is missing".to_owned(),
            "run `sbuf reindex`",
        );
    } else {
        match Index::open_at(&destination).and_then(|index| index.projected_records()) {
            Ok(projected) => {
                let mut projected_paths = HashSet::new();
                for record in projected {
                    projected_paths.insert(record.path.clone());
                    match valid.get(&record.path) {
                        Some((id, hash))
                            if id == &record.record_id && hash == &record.content_hash => {}
                        Some(_) => issue(
                            &mut report,
                            "warning",
                            format!("projection is stale for {}", record.path),
                            "run `sbuf sync`",
                        ),
                        None => issue(
                            &mut report,
                            "warning",
                            format!("projection contains deleted record {}", record.path),
                            "run `sbuf sync`",
                        ),
                    }
                }
                for path in valid.keys() {
                    if !projected_paths.contains(path) {
                        issue(
                            &mut report,
                            "warning",
                            format!("canonical record is not indexed: {path}"),
                            "run `sbuf sync`",
                        );
                    }
                }
            }
            Err(error) => issue(
                &mut report,
                "failure",
                format!("the SQLite projection cannot be opened: {error}"),
                "run `sbuf reindex`",
            ),
        }
    }

    if !report.semantic_model_ready {
        issue(
            &mut report,
            "warning",
            "semantic retrieval is not ready; search is using lexical matching only".to_owned(),
            "run `sbuf init` while online to download and verify the local model",
        );
    }
    for path in repairable_metadata(paths)? {
        issue(
            &mut report,
            "warning",
            format!("stale disposable metadata remains at {}", path.display()),
            "run `sbuf doctor --repair`",
        );
    }
    Ok(report)
}

pub fn repair_store(paths: &StorePaths) -> crate::Result<DoctorRepairReport> {
    let diagnosis = doctor_store(paths)?;
    if has_canonical_failure(&diagnosis) {
        return Ok(DoctorRepairReport {
            diagnosis,
            repaired: Vec::new(),
        });
    }

    let _mutation_lock = acquire_store_mutation_lock(paths)?;
    let diagnosis = doctor_store(paths)?;
    if has_canonical_failure(&diagnosis) {
        return Ok(DoctorRepairReport {
            diagnosis,
            repaired: Vec::new(),
        });
    }

    let mut repaired = Vec::new();
    let projection_needs_rebuild = diagnosis.issues.iter().any(|issue| {
        issue.message.contains("SQLite projection")
            || issue.message.contains("projection is stale")
            || issue.message.contains("projection contains deleted")
            || issue.message.contains("not indexed")
    });
    if projection_needs_rebuild {
        let embedder = LocalEmbedder::from_default_cache(paths).ok();
        reindex_store_with_embedder(
            paths,
            embedder.as_ref().map(|embedder| embedder as &dyn Embedder),
        )?;
        repaired.push("rebuilt the disposable search projection".to_owned());
    }
    for path in repairable_metadata(paths)? {
        match fs::remove_file(&path) {
            Ok(()) => repaired.push(format!(
                "removed stale disposable metadata {}",
                path.display()
            )),
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(Error::io("remove stale disposable metadata", source)),
        }
    }
    Ok(DoctorRepairReport {
        diagnosis: doctor_store(paths)?,
        repaired,
    })
}

fn has_canonical_failure(report: &DoctorReport) -> bool {
    report.issues.iter().any(|issue| {
        issue.severity == "failure"
            && (issue.message.contains("canonical")
                || issue.message.contains("not initialized")
                || issue.message.contains("records directory"))
    })
}

fn repairable_metadata(paths: &StorePaths) -> crate::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_stale_lock_files(&paths.root.join("locks"), &mut files)?;
    collect_disposable_files(&paths.root.join("tmp"), &mut files)?;
    let index = index_path(paths);
    if let Some(parent) = index.parent() {
        let prefix = format!(
            ".{}.",
            index
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("index.sqlite3")
        );
        let entries = match fs::read_dir(parent) {
            Ok(entries) => Some(entries),
            Err(source) if source.kind() == io::ErrorKind::NotFound => None,
            Err(source) => return Err(Error::io("inspect disposable metadata", source)),
        };
        if let Some(entries) = entries {
            for entry in entries {
                let entry =
                    entry.map_err(|source| Error::io("inspect disposable metadata", source))?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if entry.path().is_file() && name.starts_with(&prefix) && name.ends_with(".tmp") {
                    files.push(entry.path());
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

fn collect_disposable_files(directory: &Path, files: &mut Vec<PathBuf>) -> crate::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(Error::io("inspect disposable metadata", source)),
    };
    for entry in entries {
        let entry = entry.map_err(|source| Error::io("inspect disposable metadata", source))?;
        let file_type = entry
            .file_type()
            .map_err(|source| Error::io("inspect disposable metadata", source))?;
        if file_type.is_dir() {
            collect_disposable_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn collect_stale_lock_files(directory: &Path, files: &mut Vec<PathBuf>) -> crate::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(Error::io("inspect disposable metadata", source)),
    };
    for entry in entries {
        let entry = entry.map_err(|source| Error::io("inspect disposable metadata", source))?;
        let file_type = entry
            .file_type()
            .map_err(|source| Error::io("inspect disposable metadata", source))?;
        if file_type.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "lock")
            && entry.file_name() != "mutation.lock"
        {
            files.push(entry.path());
        }
    }
    Ok(())
}
