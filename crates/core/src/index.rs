use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

use crate::{Error, StorePaths, StoreScope};

mod canonical;
mod chunking;
mod doctor;
mod projection;
mod retrieval;
mod schema;
mod sync;
mod types;

pub use chunking::chunk_record;
pub use doctor::{doctor_store, repair_store};
pub use retrieval::*;
pub use sync::*;
pub use types::*;

use projection::{Index, ProjectionLock};

pub const INDEX_SCHEMA_VERSION: u32 = 6;

/// Version of the provider-neutral evidence envelope returned by `context`.
pub const CONTEXT_CONTRACT_VERSION: &str = "stormbuffer-context-v1";

const MAX_CHUNK_WORDS: usize = 160;
const MAX_CONTEXT_BLOCK_BYTES: usize = 64 * 1024;
const DEFAULT_SEARCH_LIMIT: usize = 20;
const MAX_SEARCH_LIMIT: usize = 100;

static NEXT_WRITE_PROBE: AtomicU64 = AtomicU64::new(0);

pub fn advisory_relations(paths: &StorePaths) -> crate::Result<Vec<AdvisoryRelationProjection>> {
    let path = existing_index_path(paths);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|source| db_error("open advisory relation projection", source))?;
    let mut statement = connection
        .prepare("SELECT left_record_id, right_record_id, relation, evidence_json, confidence, analyzer_fingerprint FROM advisory_relations ORDER BY left_record_id, right_record_id")
        .map_err(|source| db_error("read advisory relation projection", source))?;
    let rows = statement
        .query_map([], |row| {
            Ok(AdvisoryRelationProjection {
                left_record_id: row.get(0)?,
                right_record_id: row.get(1)?,
                relation: row.get(2)?,
                evidence_json: row.get(3)?,
                confidence: row.get(4)?,
                analyzer_fingerprint: row.get(5)?,
            })
        })
        .map_err(|source| db_error("read advisory relation projection", source))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|source| db_error("read advisory relation projection", source))
}

pub fn existing_index_path(paths: &StorePaths) -> PathBuf {
    let configured = index_path(paths);
    if configured.is_file() || paths.scope != StoreScope::Global {
        return configured;
    }
    let identity = blake3::hash(paths.root.to_string_lossy().as_bytes())
        .to_hex()
        .to_string();
    let fallback = std::env::temp_dir()
        .join(format!("stormbuffer-projection-{identity}"))
        .join("index.sqlite3");
    if fallback.is_file() {
        fallback
    } else {
        configured
    }
}

pub fn replace_advisory_relation_projection(
    paths: &StorePaths,
    relations: &[AdvisoryRelationProjection],
) -> crate::Result<()> {
    let destination = active_index_path(paths)?;
    let _lock = ProjectionLock::acquire(&destination)?;
    let mut index = Index::open_at(&destination)?;
    let transaction = index
        .connection
        .transaction()
        .map_err(|source| db_error("begin advisory relation projection", source))?;
    transaction
        .execute("DELETE FROM advisory_relations", [])
        .map_err(|source| db_error("clear advisory relation projection", source))?;
    for relation in relations {
        transaction
            .execute(
                "INSERT INTO advisory_relations(left_record_id, right_record_id, relation, evidence_json, confidence, analyzer_fingerprint) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    relation.left_record_id,
                    relation.right_record_id,
                    relation.relation,
                    relation.evidence_json,
                    relation.confidence,
                    relation.analyzer_fingerprint,
                ],
            )
            .map_err(|source| db_error("write advisory relation projection", source))?;
    }
    transaction
        .commit()
        .map_err(|source| db_error("commit advisory relation projection", source))
}

pub fn inspect_projection_status(paths: &StorePaths) -> ProjectionStatus {
    let path = match active_index_path(paths) {
        Ok(path) => path,
        Err(_) => return ProjectionStatus::default(),
    };
    let connection = match Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(connection) => connection,
        Err(_) => return ProjectionStatus::default(),
    };
    let index_version = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .ok();
    let last_successful_sync = connection
        .query_row(
            "SELECT value FROM index_metadata WHERE key = 'last_sync'",
            [],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten();
    let embedding_version = connection
        .query_row(
            "SELECT model_version FROM vector_indexes WHERE active = 1 ORDER BY index_id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten();
    ProjectionStatus {
        index_version,
        embedding_version,
        last_successful_sync,
    }
}

pub fn index_path(paths: &StorePaths) -> PathBuf {
    match paths.scope {
        StoreScope::Global => paths.cache.join("global.sqlite3"),
        StoreScope::Project | StoreScope::Local => paths.root.join("index.sqlite3"),
    }
}

pub fn active_index_path(paths: &StorePaths) -> crate::Result<PathBuf> {
    let configured = index_path(paths);
    if paths.scope != StoreScope::Global || directory_supports_writes(&paths.cache) {
        return Ok(configured);
    }
    fallback_global_index_path(paths)
}

pub fn content_hash(markdown: &str) -> String {
    blake3::hash(markdown.as_bytes()).to_hex().to_string()
}

fn directory_supports_writes(directory: &Path) -> bool {
    if let Err(error) = fs::create_dir_all(directory) {
        return error.kind() != io::ErrorKind::PermissionDenied;
    }
    let probe = directory.join(format!(
        ".write-probe-{}-{}",
        std::process::id(),
        NEXT_WRITE_PROBE.fetch_add(1, Ordering::Relaxed)
    ));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(file) => {
            drop(file);
            let _ = fs::remove_file(probe);
            true
        }
        Err(error) => error.kind() != io::ErrorKind::PermissionDenied,
    }
}

fn fallback_global_index_path(paths: &StorePaths) -> crate::Result<PathBuf> {
    let identity = blake3::hash(paths.root.to_string_lossy().as_bytes())
        .to_hex()
        .to_string();
    let directory = std::env::temp_dir().join(format!("stormbuffer-projection-{identity}"));
    create_private_directory(&directory)?;
    Ok(directory.join("index.sqlite3"))
}

#[cfg(unix)]
fn create_private_directory(directory: &Path) -> crate::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    match builder.create(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(directory)
                .map_err(|source| Error::io("inspect the fallback index directory", source))?;
            if !metadata.file_type().is_dir() {
                return Err(Error::io(
                    "secure the fallback index directory",
                    io::Error::other("fallback index path is not a directory"),
                ));
            }
        }
        Err(source) => return Err(Error::io("create the fallback index directory", source)),
    }
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
        .map_err(|source| Error::io("secure the fallback index directory", source))
}

#[cfg(not(unix))]
fn create_private_directory(directory: &Path) -> crate::Result<()> {
    fs::create_dir_all(directory)
        .map_err(|source| Error::io("create the fallback index directory", source))
}

fn db_error(operation: &'static str, source: rusqlite::Error) -> Error {
    Error::Index { operation, source }
}

#[cfg(test)]
mod tests;
