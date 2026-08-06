use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use fs2::FileExt;
use serde::Serialize;
use stormbuffer_core::{
    DestructionAcknowledgement, Error, RecordId, RecordRepository, RecordStatus, StoreInitMode,
    StorePaths, StoreScope, Timestamp, initialize_store, parse_markdown, render_markdown,
};

struct TempStore {
    root: PathBuf,
}

static NEXT_TEMP_STORE: AtomicU64 = AtomicU64::new(0);

impl TempStore {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        for attempt in 0..100 {
            let counter = NEXT_TEMP_STORE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "stormbuffer-repository-test-{}-{timestamp}-{counter}-{attempt}",
                std::process::id(),
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Self { root },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create temporary store root: {error}"),
            }
        }
        panic!("could not find a unique temporary store root")
    }

    fn paths(&self) -> StorePaths {
        StorePaths {
            scope: StoreScope::Global,
            records: self.root.join("records"),
            cache: self.root.join("cache"),
            root: self.root.clone(),
        }
    }

    fn repository(&self) -> RecordRepository {
        let paths = self.paths();
        initialize_store(&paths, StoreInitMode::Default).expect("initialize temporary store");
        RecordRepository::new(paths)
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn fixture_record() -> stormbuffer_core::Record {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid/fact.md");
    let markdown = fs::read_to_string(&path).expect("read fixture");
    parse_markdown(&path, &markdown).expect("parse fixture")
}

#[test]
fn atomic_replacements_are_always_parseable_to_unlocked_readers() {
    let store = TempStore::new();
    let repository = store.repository();
    let stored = repository.add(fixture_record()).expect("add fixture");
    let path = stored.path().to_path_buf();
    let id = stored.record().id;
    let done = Arc::new(AtomicBool::new(false));
    let writer_done = Arc::clone(&done);
    let writer_repository = repository.clone();

    let writer = thread::spawn(move || {
        for index in 0..100 {
            let current = writer_repository.find(id).expect("find current record");
            let mut replacement = current.record().clone();
            replacement.body = format!("replacement body {index}");
            replacement.updated_at = Timestamp::now_utc();
            writer_repository
                .replace_if_unchanged(&current, replacement)
                .expect("replace record");
        }
        writer_done.store(true, Ordering::Release);
    });

    while !done.load(Ordering::Acquire) {
        let bytes = fs::read(&path).expect("read canonical record");
        let markdown = String::from_utf8(bytes).expect("canonical record is UTF-8");
        parse_markdown(&path, &markdown).expect("canonical record is never partial");
        thread::yield_now();
    }
    writer.join().expect("writer thread");
}

#[test]
fn competing_mutation_reports_busy_without_leaking_the_store_path() {
    let store = TempStore::new();
    let repository = store.repository();
    let lock_path = store.root.join("locks/mutation.lock");
    fs::create_dir_all(lock_path.parent().expect("lock parent")).expect("create lock directory");
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .expect("open mutation lock");
    lock_file.try_lock_exclusive().expect("hold mutation lock");

    let error = repository
        .add(fixture_record())
        .expect_err("lock must contend");
    assert!(matches!(
        error,
        Error::Repository {
            source: stormbuffer_core::RepositoryError::MutationBusy { .. }
        }
    ));
    assert!(
        !error
            .to_string()
            .contains(store.root.to_string_lossy().as_ref())
    );
    FileExt::unlock(&lock_file).expect("release mutation lock");
}

#[derive(Serialize)]
struct TestJournal {
    old_path: PathBuf,
    new_path: PathBuf,
    old_before: Vec<u8>,
    old_after: String,
    new_after: String,
}

#[test]
fn pending_supersession_journal_recovers_after_interruption() {
    let store = TempStore::new();
    let repository = store.repository();
    let old = repository.add(fixture_record()).expect("add fixture");
    let mut superseded = old.record().clone();
    superseded
        .transition_to(RecordStatus::Superseded)
        .expect("supersede old");
    superseded.updated_at = Timestamp::now_utc();

    let mut replacement = old.record().clone();
    replacement.id = RecordId::new_v7();
    replacement.supersedes = vec![old.record().id];
    replacement.created_at = Timestamp::now_utc();
    replacement.updated_at = replacement.created_at;
    replacement.body = "recovered replacement".to_owned();

    let old_after = render_markdown(&superseded).expect("render old record");
    let new_after = render_markdown(&replacement).expect("render replacement");
    let journal = TestJournal {
        old_path: old.path().to_path_buf(),
        new_path: store.paths().records.join(format!("{}.md", replacement.id)),
        old_before: old.markdown().as_bytes().to_vec(),
        old_after,
        new_after,
    };
    let journal_path = store.root.join("locks/supersede.toml");
    fs::write(
        &journal_path,
        toml::to_string(&journal).expect("render test journal"),
    )
    .expect("write interrupted journal");

    let records = repository.list(true).expect("recover supersession");
    assert!(!journal_path.exists());
    assert!(records.iter().any(|record| {
        record.record().id == old.record().id && record.record().status == RecordStatus::Superseded
    }));
    assert!(records.iter().any(|record| {
        record.record().id == replacement.id && record.record().status == RecordStatus::Active
    }));
}

#[test]
fn concurrent_replacement_is_rejected_without_overwriting_new_bytes() {
    let store = TempStore::new();
    let repository = store.repository();
    let current = repository.add(fixture_record()).expect("add fixture");
    let mut newer = current.record().clone();
    newer.body = "newer authored body".to_owned();
    newer.updated_at = Timestamp::now_utc();
    repository
        .replace_if_unchanged(&current, newer)
        .expect("write newer record");

    let mut stale = current.record().clone();
    stale.body = "stale editor body".to_owned();
    stale.updated_at = Timestamp::now_utc();
    let error = repository
        .replace_if_unchanged(&current, stale)
        .expect_err("stale editor must not overwrite newer bytes");
    assert!(matches!(
        error,
        Error::Repository {
            source: stormbuffer_core::RepositoryError::ConcurrentModification { .. }
        }
    ));
    assert!(
        repository
            .find(current.record().id)
            .expect("read newer record")
            .record()
            .body
            .contains("newer authored body")
    );
}

#[test]
fn superseded_records_cannot_be_edited() {
    let store = TempStore::new();
    let repository = store.repository();
    let old = repository.add(fixture_record()).expect("add fixture");
    let mut replacement = old.record().clone();
    replacement.id = RecordId::new_v7();
    replacement.supersedes = vec![old.record().id];
    replacement.created_at = Timestamp::now_utc();
    replacement.updated_at = replacement.created_at;
    repository
        .supersede(old.record().id, replacement)
        .expect("supersede fixture");

    let superseded = repository
        .find(old.record().id)
        .expect("find superseded record");
    let edited = superseded.record().clone();
    let error = repository
        .replace_if_unchanged(&superseded, edited)
        .expect_err("superseded history must be immutable");
    assert!(matches!(
        error,
        Error::Repository {
            source: stormbuffer_core::RepositoryError::MustBeActive { .. }
        }
    ));
}

#[test]
fn forgetting_requires_a_typed_acknowledgement() {
    let store = TempStore::new();
    let repository = store.repository();
    let stored = repository.add(fixture_record()).expect("add fixture");
    let id = stored.record().id;
    repository
        .forget(id, DestructionAcknowledgement::deliberate())
        .expect("forget with acknowledgement");
    assert!(repository.find(id).is_err());
}
