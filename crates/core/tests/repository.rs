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
    DestructionAcknowledgement, Error, ProposalActor, ProposalOutcome, RecordId, RecordRepository, RecordStatus,
    StoreInitMode, StorePaths, StoreScope, Timestamp, initialize_store, parse_markdown, render_markdown,
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
fn repository_rejects_records_outside_the_selected_store_scope() {
    let store = TempStore::new();
    let repository = store.repository();
    let stored = repository.add(fixture_record()).expect("add fixture");
    let mut foreign = stored.record().clone();
    foreign.scope = "project:01989af2-4305-7b19-88b1-e8ae4ea9a099"
        .parse()
        .expect("foreign scope");
    fs::write(stored.path(), render_markdown(&foreign).expect("render foreign record"))
        .expect("move record outside the store scope");

    assert!(matches!(
        repository.find(foreign.id),
        Err(Error::Repository { source: stormbuffer_core::RepositoryError::ScopeDenied { .. } })
    ));
    assert!(matches!(
        repository.archive(foreign.id),
        Err(Error::Repository { source: stormbuffer_core::RepositoryError::ScopeDenied { .. } })
    ));
}

#[test]
fn replacement_cannot_move_a_record_outside_the_selected_store_scope() {
    let store = TempStore::new();
    let repository = store.repository();
    let stored = repository.add(fixture_record()).expect("add fixture");
    let mut replacement = stored.record().clone();
    replacement.scope = "project:01989af2-4305-7b19-88b1-e8ae4ea9a099"
        .parse()
        .expect("foreign scope");

    assert!(matches!(
        repository.replace_if_unchanged(&stored, replacement),
        Err(Error::Repository { source: stormbuffer_core::RepositoryError::ScopeDenied { .. } })
    ));
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

    let error = repository.add(fixture_record()).expect_err("lock must contend");
    assert!(matches!(
        error,
        Error::Repository { source: stormbuffer_core::RepositoryError::MutationBusy { .. } }
    ));
    assert!(!error.to_string().contains(store.root.to_string_lossy().as_ref()));
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
    fs::write(&journal_path, toml::to_string(&journal).expect("render test journal"))
        .expect("write interrupted journal");

    let records = repository.list(true).expect("recover supersession");
    assert!(!journal_path.exists());
    assert!(
        records.iter().any(|record| {
            record.record().id == old.record().id && record.record().status == RecordStatus::Superseded
        })
    );
    assert!(
        records
            .iter()
            .any(|record| { record.record().id == replacement.id && record.record().status == RecordStatus::Active })
    );
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
        Error::Repository { source: stormbuffer_core::RepositoryError::ConcurrentModification { .. } }
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

    let superseded = repository.find(old.record().id).expect("find superseded record");
    let edited = superseded.record().clone();
    let error = repository
        .replace_if_unchanged(&superseded, edited)
        .expect_err("superseded history must be immutable");
    assert!(matches!(
        error,
        Error::Repository { source: stormbuffer_core::RepositoryError::MustBeActive { .. } }
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

#[test]
fn agent_proposals_require_approval_and_rejection_archives_candidates() {
    let store = TempStore::new();
    let repository = store.repository();
    let mut candidate = fixture_record();
    candidate.id = RecordId::new_v7();
    candidate.status = RecordStatus::Candidate;
    candidate.title = "A sourced proposal".to_owned();
    candidate.body = "A distinct proposal body".to_owned();

    let proposal = repository
        .propose(candidate, ProposalActor::Agent)
        .expect("propose candidate");
    assert_eq!(proposal.outcome, ProposalOutcome::RequiresApproval);
    let id = proposal.record_id.parse().expect("proposal id");
    assert_eq!(
        repository.find(id).expect("find candidate").record().status,
        RecordStatus::Candidate
    );

    let approved = repository.approve(id).expect("approve candidate");
    assert_eq!(approved.outcome, ProposalOutcome::Accepted);
    assert_eq!(
        repository.find(id).expect("find approved").record().status,
        RecordStatus::Active
    );

    let mut rejected = fixture_record();
    rejected.id = RecordId::new_v7();
    rejected.title = "Another sourced proposal".to_owned();
    rejected.body = "Another distinct proposal body".to_owned();
    let rejected = repository
        .propose(rejected, ProposalActor::Agent)
        .expect("propose second candidate");
    let rejected_id = rejected.record_id.parse().expect("rejected id");
    let result = repository.reject(rejected_id).expect("reject candidate");
    assert_eq!(result.outcome, ProposalOutcome::Accepted);
    assert_eq!(result.status.as_deref(), Some("archived"));
}

#[test]
fn update_proposals_preserve_the_active_record_until_approval() {
    let store = TempStore::new();
    let repository = store.repository();
    let old = repository.add(fixture_record()).expect("add fixture");
    let old_id = old.record().id;
    let mut replacement = old.record().clone();
    replacement.id = RecordId::new_v7();
    replacement.created_at = Timestamp::now_utc();
    replacement.updated_at = replacement.created_at;
    replacement.body = "A sourced replacement body".to_owned();

    let proposed = repository.propose_update(old_id, replacement).expect("propose update");
    assert_eq!(proposed.outcome, ProposalOutcome::RequiresApproval);
    let replacement_id = proposed.record_id.parse().expect("replacement id");
    assert_eq!(
        repository.find(old_id).expect("find old").record().status,
        RecordStatus::Active
    );
    let candidate = repository.find(replacement_id).expect("find replacement candidate");
    assert_eq!(candidate.record().status, RecordStatus::Candidate);
    assert_eq!(candidate.record().supersedes, vec![old_id]);

    let approved = repository.approve(replacement_id).expect("approve replacement");
    assert_eq!(approved.outcome, ProposalOutcome::Accepted);
    assert_eq!(
        repository.find(old_id).expect("find old").record().status,
        RecordStatus::Superseded
    );
    assert_eq!(
        repository
            .find(replacement_id)
            .expect("find replacement")
            .record()
            .status,
        RecordStatus::Active
    );
}

#[test]
fn update_proposals_report_missing_evidence_without_writing() {
    let store = TempStore::new();
    let repository = store.repository();
    let old = repository.add(fixture_record()).expect("add fixture");
    let mut replacement = old.record().clone();
    replacement.id = RecordId::new_v7();
    replacement.sources.clear();

    let result = repository
        .propose_update(old.record().id, replacement)
        .expect("invalid update result");
    assert_eq!(result.outcome, ProposalOutcome::Invalid);
    assert_eq!(repository.list(true).expect("list records").len(), 1);
}

#[test]
fn approval_revalidates_user_edited_candidate_provenance() {
    let store = TempStore::new();
    let repository = store.repository();
    let mut candidate = fixture_record();
    candidate.id = RecordId::new_v7();
    candidate.status = RecordStatus::Candidate;
    candidate.title = "Candidate edited after proposal".to_owned();
    candidate.body = "A distinct candidate body".to_owned();

    let proposal = repository
        .propose(candidate, ProposalActor::Agent)
        .expect("propose candidate");
    let id = proposal.record_id.parse().expect("proposal id");
    let stored = repository.find(id).expect("find candidate");
    let mut edited = stored.record().clone();
    edited.sources[0].actor = "inference".to_owned();
    fs::write(
        stored.path(),
        render_markdown(&edited).expect("render edited candidate"),
    )
    .expect("edit candidate markdown");

    let error = repository.approve(id).expect_err("reject invalid provenance");
    assert!(error.to_string().contains("inference"));
    assert_eq!(
        repository.find(id).expect("candidate remains").record().status,
        RecordStatus::Candidate
    );
}

#[test]
fn missing_provenance_is_invalid_and_same_title_human_proposals_require_review() {
    let store = TempStore::new();
    let repository = store.repository();
    let existing = repository.add(fixture_record()).expect("add fixture");

    let mut missing_source = existing.record().clone();
    missing_source.id = RecordId::new_v7();
    missing_source.sources.clear();
    let invalid = repository
        .propose(missing_source, ProposalActor::Agent)
        .expect("invalid proposal result");
    assert_eq!(invalid.outcome, ProposalOutcome::Invalid);
    assert_eq!(repository.list(true).expect("list records").len(), 1);

    let mut overlap = existing.record().clone();
    overlap.id = RecordId::new_v7();
    overlap.body = "A different claim with the same title".to_owned();
    let result = repository
        .propose(overlap, ProposalActor::Human)
        .expect("overlap proposal result");
    assert_eq!(result.outcome, ProposalOutcome::PossibleOverlap);
    let id = result.record_id.parse().expect("overlap id");
    assert_eq!(
        repository.find(id).expect("find overlap").record().status,
        RecordStatus::Candidate
    );
    assert_eq!(
        repository.approve(id).expect("approve after review").outcome,
        ProposalOutcome::Accepted
    );
}
