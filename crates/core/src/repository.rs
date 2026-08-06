use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use thiserror::Error as ThisError;

use super::{
    DestructionAcknowledgement, Error, Record, RecordError, RecordId, RecordStatus, StorePaths,
    Timestamp, parse_markdown, render_markdown,
};

const LOCK_DIRECTORY: &str = "locks";
const MUTATION_LOCK: &str = "mutation.lock";
const SUPERSEDE_JOURNAL: &str = "supersede.toml";

#[derive(Debug, ThisError)]
pub enum RepositoryError {
    #[error("store is not initialized")]
    StoreNotInitialized { root: PathBuf },
    #[error("record {id} was not found")]
    NotFound { id: RecordId },
    #[error("record {id} appears in multiple files")]
    DuplicateId {
        id: RecordId,
        first: PathBuf,
        second: PathBuf,
    },
    #[error("store mutation lock is busy")]
    MutationBusy { path: PathBuf },
    #[error("record changed while it was being edited")]
    ConcurrentModification { path: PathBuf },
    #[error("record id cannot change during an update")]
    ImmutableId,
    #[error("record {id} must be active, not {status}")]
    MustBeActive { id: RecordId, status: RecordStatus },
    #[error("record {id} must be archived, not {status}")]
    MustBeArchived { id: RecordId, status: RecordStatus },
    #[error("replacement {replacement} does not supersede {old}")]
    MissingSupersededLink {
        replacement: RecordId,
        old: RecordId,
    },
    #[error("record destination already exists")]
    DestinationExists { path: PathBuf },
    #[error("supersession recovery conflicts with an authored change")]
    RecoveryConflict { path: PathBuf },
    #[error("supersession journal is invalid")]
    InvalidJournal { message: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredRecord {
    path: PathBuf,
    markdown: String,
    record: Record,
}

impl StoredRecord {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn markdown(&self) -> &str {
        &self.markdown
    }

    pub fn record(&self) -> &Record {
        &self.record
    }
}

#[derive(Clone, Debug)]
pub struct RecordRepository {
    paths: StorePaths,
}

impl RecordRepository {
    pub fn new(paths: StorePaths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &StorePaths {
        &self.paths
    }

    pub fn add(&self, record: Record) -> Result<StoredRecord, Error> {
        record.validate()?;
        if record.status != RecordStatus::Active {
            return Err(Error::repository(RepositoryError::MustBeActive {
                id: record.id,
                status: record.status,
            }));
        }
        let _lock = self.prepare_mutation()?;
        let records = self.scan_locked()?;
        if let Some(existing) = records.iter().find(|stored| stored.record.id == record.id) {
            return Err(Error::repository(RepositoryError::DuplicateId {
                id: record.id,
                first: existing.path.clone(),
                second: self.record_path(record.id),
            }));
        }

        let path = self.record_path(record.id);
        if path.exists() {
            return Err(Error::repository(RepositoryError::DestinationExists {
                path,
            }));
        }
        let markdown = render_markdown(&record)?;
        write_atomic(&path, markdown.as_bytes())?;
        self.read_record(&path)
    }

    pub fn find(&self, id: RecordId) -> Result<StoredRecord, Error> {
        let _lock = self.prepare_mutation()?;
        self.find_locked(id)
    }

    pub fn list(&self, include_inactive: bool) -> Result<Vec<StoredRecord>, Error> {
        let _lock = self.prepare_mutation()?;
        let mut records = self.scan_locked()?;
        if !include_inactive {
            records.retain(|stored| stored.record.status == RecordStatus::Active);
        }
        records.sort_by_key(|stored| stored.record.id.to_string());
        Ok(records)
    }

    pub fn replace_if_unchanged(
        &self,
        current: &StoredRecord,
        replacement: Record,
    ) -> Result<StoredRecord, Error> {
        replacement.validate()?;
        if replacement.id != current.record.id {
            return Err(Error::repository(RepositoryError::ImmutableId));
        }

        let _lock = self.prepare_mutation()?;
        let latest = self.read_record(&current.path)?;
        if latest.markdown != current.markdown {
            return Err(Error::repository(RepositoryError::ConcurrentModification {
                path: current.path.clone(),
            }));
        }
        if latest.record.status != RecordStatus::Active {
            return Err(Error::repository(RepositoryError::MustBeActive {
                id: latest.record.id,
                status: latest.record.status,
            }));
        }
        if replacement.status != RecordStatus::Active {
            return Err(Error::repository(RepositoryError::MustBeActive {
                id: replacement.id,
                status: replacement.status,
            }));
        }

        let markdown = render_markdown(&replacement)?;
        write_atomic(&current.path, markdown.as_bytes())?;
        self.read_record(&current.path)
    }

    pub fn archive(&self, id: RecordId) -> Result<StoredRecord, Error> {
        self.transition(id, RecordStatus::Archived)
    }

    pub fn restore(&self, id: RecordId) -> Result<StoredRecord, Error> {
        self.transition(id, RecordStatus::Active)
    }

    pub fn supersede(
        &self,
        old_id: RecordId,
        mut replacement: Record,
    ) -> Result<StoredRecord, Error> {
        replacement.validate()?;
        let _lock = self.prepare_mutation()?;
        let records = self.scan_locked()?;
        let old = records
            .iter()
            .find(|stored| stored.record.id == old_id)
            .cloned()
            .ok_or_else(|| Error::repository(RepositoryError::NotFound { id: old_id }))?;
        if old.record.status != RecordStatus::Active {
            return Err(Error::repository(RepositoryError::MustBeActive {
                id: old_id,
                status: old.record.status,
            }));
        }
        if replacement.id == old_id {
            return Err(Error::repository(RepositoryError::ImmutableId));
        }
        if !replacement.supersedes.contains(&old_id) {
            return Err(Error::repository(RepositoryError::MissingSupersededLink {
                replacement: replacement.id,
                old: old_id,
            }));
        }
        if let Some(existing) = records
            .iter()
            .find(|stored| stored.record.id == replacement.id)
        {
            return Err(Error::repository(RepositoryError::DuplicateId {
                id: replacement.id,
                first: existing.path.clone(),
                second: self.record_path(replacement.id),
            }));
        }
        if replacement.status != RecordStatus::Active {
            return Err(Error::repository(RepositoryError::MustBeActive {
                id: replacement.id,
                status: replacement.status,
            }));
        }

        let now = Timestamp::now_utc();
        replacement.updated_at = now;
        let mut superseded = old.record.clone();
        superseded.transition_to(RecordStatus::Superseded)?;
        superseded.updated_at = now;

        let old_markdown = render_markdown(&superseded)?;
        let new_markdown = render_markdown(&replacement)?;
        let new_path = self.record_path(replacement.id);
        let journal = SupersedeJournal {
            old_path: old.path.clone(),
            new_path: new_path.clone(),
            old_before: old.markdown.into_bytes(),
            old_after: old_markdown,
            new_after: new_markdown,
        };
        self.write_journal(&journal)?;
        write_atomic(&journal.old_path, journal.old_after.as_bytes())?;
        write_atomic(&journal.new_path, journal.new_after.as_bytes())?;
        remove_file_sync(&self.journal_path())?;
        self.read_record(&journal.new_path)
    }

    pub fn forget(
        &self,
        id: RecordId,
        _acknowledgement: DestructionAcknowledgement,
    ) -> Result<(), Error> {
        let _lock = self.prepare_mutation()?;
        let stored = self.find_locked(id)?;
        fs::remove_file(&stored.path).map_err(|source| Error::io("delete the record", source))?;
        sync_parent_directory(&stored.path)
            .map_err(|source| Error::io("sync the records directory", source))?;
        Ok(())
    }

    fn transition(&self, id: RecordId, next: RecordStatus) -> Result<StoredRecord, Error> {
        let _lock = self.prepare_mutation()?;
        let current = self.find_locked(id)?;
        if next == RecordStatus::Active && current.record.status != RecordStatus::Archived {
            return Err(Error::repository(RepositoryError::MustBeArchived {
                id,
                status: current.record.status,
            }));
        }
        if next == RecordStatus::Archived && current.record.status != RecordStatus::Active {
            return Err(Error::repository(RepositoryError::MustBeActive {
                id,
                status: current.record.status,
            }));
        }

        let mut replacement = current.record.clone();
        replacement.transition_to(next)?;
        replacement.updated_at = Timestamp::now_utc();
        let markdown = render_markdown(&replacement)?;
        write_atomic(&current.path, markdown.as_bytes())?;
        self.read_record(&current.path)
    }

    fn prepare_mutation(&self) -> Result<MutationLock, Error> {
        let lock = acquire_store_mutation_lock(&self.paths)?;
        self.recover_supersession()?;
        Ok(lock)
    }

    fn scan_locked(&self) -> Result<Vec<StoredRecord>, Error> {
        let mut paths = Vec::new();
        collect_markdown_paths(&self.paths.records, &mut paths)?;
        let mut records = Vec::with_capacity(paths.len());
        let mut seen = HashMap::with_capacity(paths.len());
        for path in paths {
            let stored = self.read_record(&path)?;
            if let Some(first) = seen.insert(stored.record.id, path.clone()) {
                return Err(Error::repository(RepositoryError::DuplicateId {
                    id: stored.record.id,
                    first,
                    second: path,
                }));
            }
            records.push(stored);
        }
        Ok(records)
    }

    fn find_locked(&self, id: RecordId) -> Result<StoredRecord, Error> {
        self.scan_locked()?
            .into_iter()
            .find(|stored| stored.record.id == id)
            .ok_or_else(|| Error::repository(RepositoryError::NotFound { id }))
    }

    fn read_record(&self, path: &Path) -> Result<StoredRecord, Error> {
        let bytes = fs::read(path).map_err(|source| Error::io("read the record", source))?;
        let markdown = String::from_utf8(bytes).map_err(|_| {
            Error::invalid_record_at(
                path,
                RecordError::Markdown {
                    message: "record is not valid UTF-8".to_owned(),
                },
            )
        })?;
        let record = parse_markdown(path, &markdown)?;
        Ok(StoredRecord {
            path: path.to_path_buf(),
            markdown,
            record,
        })
    }

    fn record_path(&self, id: RecordId) -> PathBuf {
        self.paths.records.join(format!("{id}.md"))
    }

    fn journal_path(&self) -> PathBuf {
        self.paths.root.join(LOCK_DIRECTORY).join(SUPERSEDE_JOURNAL)
    }

    fn write_journal(&self, journal: &SupersedeJournal) -> Result<(), Error> {
        let contents = toml::to_string(journal).map_err(|source| {
            Error::repository(RepositoryError::InvalidJournal {
                message: source.to_string(),
            })
        })?;
        write_atomic(&self.journal_path(), contents.as_bytes())
    }

    fn recover_supersession(&self) -> Result<(), Error> {
        let path = self.journal_path();
        if !path.is_file() {
            return Ok(());
        }
        let contents = fs::read_to_string(&path)
            .map_err(|source| Error::io("read the supersession journal", source))?;
        let journal: SupersedeJournal = toml::from_str(&contents).map_err(|source| {
            Error::repository(RepositoryError::InvalidJournal {
                message: source.to_string(),
            })
        })?;

        let old_current = read_optional_bytes(&journal.old_path)?;
        if old_current.as_deref() != Some(journal.old_before.as_slice())
            && old_current.as_deref() != Some(journal.old_after.as_bytes())
        {
            return Err(Error::repository(RepositoryError::RecoveryConflict {
                path: journal.old_path.clone(),
            }));
        }
        let new_current = read_optional_bytes(&journal.new_path)?;
        if let Some(current) = new_current.as_deref() {
            if current != journal.new_after.as_bytes() {
                return Err(Error::repository(RepositoryError::RecoveryConflict {
                    path: journal.new_path.clone(),
                }));
            }
        }

        if old_current.as_deref() != Some(journal.old_after.as_bytes()) {
            write_atomic(&journal.old_path, journal.old_after.as_bytes())?;
        }
        if new_current.is_none() {
            write_atomic(&journal.new_path, journal.new_after.as_bytes())?;
        }
        remove_file_sync(&path)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct SupersedeJournal {
    old_path: PathBuf,
    new_path: PathBuf,
    old_before: Vec<u8>,
    old_after: String,
    new_after: String,
}

pub(crate) struct MutationLock {
    file: File,
}

impl MutationLock {
    fn acquire(path: &Path) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|source| Error::io("open the store mutation lock", source))?;
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Self { file }),
            Err(source) if source.kind() == io::ErrorKind::WouldBlock => {
                Err(Error::repository(RepositoryError::MutationBusy {
                    path: path.to_path_buf(),
                }))
            }
            Err(source) => Err(Error::io("lock the store", source)),
        }
    }
}

impl Drop for MutationLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

pub(crate) fn acquire_store_mutation_lock(paths: &StorePaths) -> Result<MutationLock, Error> {
    if !paths.root.join("store.toml").is_file() {
        return Err(Error::repository(RepositoryError::StoreNotInitialized {
            root: paths.root.clone(),
        }));
    }
    fs::create_dir_all(paths.root.join(LOCK_DIRECTORY))
        .map_err(|source| Error::io("create the store lock directory", source))?;
    MutationLock::acquire(&paths.root.join(LOCK_DIRECTORY).join(MUTATION_LOCK))
}

fn collect_markdown_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), Error> {
    let entries =
        fs::read_dir(directory).map_err(|source| Error::io("scan the records", source))?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::io("scan the records", source))?;
        let file_type = entry
            .file_type()
            .map_err(|source| Error::io("inspect a record entry", source))?;
        if file_type.is_dir() {
            collect_markdown_paths(&entry.path(), paths)?;
        } else if file_type.is_file() && entry.path().extension() == Some(OsStr::new("md")) {
            paths.push(entry.path());
        }
    }
    Ok(())
}

fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::io("read a recovery file", source)),
    }
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), Error> {
    let parent = path.parent().ok_or_else(|| {
        Error::io(
            "resolve the record parent",
            io::Error::other("missing parent"),
        )
    })?;
    fs::create_dir_all(parent).map_err(|source| Error::io("create the record parent", source))?;
    let (temp_path, mut temp_file) = create_temp_file(path)?;
    let mut cleanup = TempCleanup::new(temp_path.clone());
    let result = (|| {
        temp_file
            .write_all(contents)
            .map_err(|source| Error::io("write the temporary record", source))?;
        temp_file
            .sync_all()
            .map_err(|source| Error::io("sync the temporary record", source))?;
        drop(temp_file);
        replace_file(&temp_path, path)
            .map_err(|source| Error::io("atomically replace the record", source))?;
        sync_parent_directory(path).map_err(|source| Error::io("sync the record directory", source))
    })();
    if result.is_ok() {
        cleanup.disarm();
    }
    result
}

fn create_temp_file(path: &Path) -> Result<(PathBuf, File), Error> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("record");
    let parent = path.parent().ok_or_else(|| {
        Error::io(
            "resolve the temporary record parent",
            io::Error::other("missing parent"),
        )
    })?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| Error::io("create a temporary record name", io::Error::other(source)))?
        .as_nanos();
    for attempt in 0..1000u32 {
        let temp_path = parent.join(format!(
            ".{name}.{}.{}.tmp",
            std::process::id(),
            stamp + u128::from(attempt)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(Error::io("create the temporary record", source)),
        }
    }
    Err(Error::io(
        "create the temporary record",
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary file name exhausted",
        ),
    ))
}

struct TempCleanup {
    path: Option<PathBuf>,
}

impl TempCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for TempCleanup {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    fs::rename(from, to)
}

#[cfg(windows)]
pub(crate) fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        // The vectors are NUL-terminated and remain alive for the call.
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn remove_file_sync(path: &Path) -> Result<(), Error> {
    match fs::remove_file(path) {
        Ok(()) => {
            sync_parent_directory(path)
                .map_err(|source| Error::io("sync the store lock directory", source))?;
            Ok(())
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::io("remove the supersession journal", source)),
    }
}
