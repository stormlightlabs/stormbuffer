use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;

use crate::repository::replace_file;
use crate::{Embedder, Error, StorePaths, VectorMetadata};

use super::projection::{Index, ProjectionLock};
use super::{SemanticIndexReport, SyncReport, WatchOptions, WatchReport, active_index_path};

pub fn sync_store(paths: &StorePaths) -> crate::Result<SyncReport> {
    crate::record_scope(paths)?;
    let destination = active_index_path(paths)?;
    let _lock = ProjectionLock::acquire(&destination)?;
    let mut index = Index::open_at(&destination)?;
    index.sync_canonical(paths)
}

pub fn reindex_store(paths: &StorePaths) -> crate::Result<SyncReport> {
    reindex_store_with_embedder(paths, None)
}

pub fn reindex_store_with_embedder(
    paths: &StorePaths,
    embedder: Option<&dyn Embedder>,
) -> crate::Result<SyncReport> {
    crate::record_scope(paths)?;
    let destination = active_index_path(paths)?;
    let _lock = ProjectionLock::acquire(&destination)?;
    let parent = destination.parent().ok_or_else(|| {
        Error::io(
            "resolve the index directory",
            io::Error::other("index path has no parent"),
        )
    })?;
    fs::create_dir_all(parent).map_err(|source| Error::io("create the index directory", source))?;
    let temporary = parent.join(format!(
        ".{}.{}.{}.tmp",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("index.sqlite3"),
        std::process::id(),
        std::thread::current().name().unwrap_or("reindex")
    ));
    let _ = fs::remove_file(&temporary);

    let result = (|| {
        let mut fresh = Index::open_at(&temporary)?;
        let mut report = fresh.sync_canonical(paths)?;
        report.semantic = Some(match embedder {
            Some(embedder) => {
                fresh.rebuild_vectors(paths, embedder)?;
                SemanticIndexReport {
                    status: "rebuilt".to_owned(),
                    model_version: Some(embedder.model_version().to_owned()),
                    message: None,
                }
            }
            None => SemanticIndexReport {
                status: "unavailable".to_owned(),
                model_version: None,
                message: Some(
                    "no verified embedding model was supplied; run `sbuf init` when online, then `sbuf reindex`".to_owned(),
                ),
            },
        });
        fresh.checkpoint()?;
        drop(fresh);

        replace_file(&temporary, &destination)
            .map_err(|source| Error::io("switch the rebuilt index", source))?;
        remove_sqlite_sidecars(&destination);
        sync_parent_directory(&destination)
            .map_err(|source| Error::io("sync the index directory", source))?;
        Ok(report)
    })();
    let _ = fs::remove_file(&temporary);
    result
}

/// Reconciles canonical Markdown and rebuilds the semantic projection.
///
/// Returns an error if any canonical record is invalid; callers never receive
/// metadata for a partial semantic index.
pub fn rebuild_vector_index(
    paths: &StorePaths,
    embedder: &dyn Embedder,
) -> crate::Result<VectorMetadata> {
    crate::record_scope(paths)?;
    let destination = active_index_path(paths)?;
    let _lock = ProjectionLock::acquire(&destination)?;
    let mut index = Index::open_at(&destination)?;
    let report = index.sync_canonical(paths)?;
    if !report.is_complete() {
        return Err(Error::invalid_record(
            "canonical store",
            "one or more canonical records are invalid",
        ));
    }
    index.rebuild_vectors(paths, embedder)
}

pub fn watch_store(paths: &StorePaths, options: WatchOptions) -> crate::Result<WatchReport> {
    let mut aggregate = WatchReport {
        cycles: 0,
        indexed: 0,
        skipped: 0,
        removed: 0,
        invalid_files: Vec::new(),
    };
    loop {
        let report = sync_store(paths)?;
        aggregate.cycles += 1;
        aggregate.indexed += report.indexed;
        aggregate.skipped += report.skipped;
        aggregate.removed += report.removed;
        aggregate.invalid_files.extend(report.invalid_files);
        if options.once {
            return Ok(aggregate);
        }
        thread::sleep(options.interval);
    }
}

fn remove_sqlite_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(path.with_extension(format!("sqlite3{suffix}")));
        let sidecar = PathBuf::from(format!("{}{}", path.display(), suffix));
        let _ = fs::remove_file(sidecar);
    }
}

fn sync_parent_directory(path: &Path) -> io::Result<()> {
    File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()
}
