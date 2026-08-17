use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use stormbuffer_core::{
    AdvisoryRelationProjection, InboxFilter, ProposalActor, RecordId, RecordKind, RecordRepository, RecordStatus,
    StoreInitMode, StorePaths, StoreScope, Timestamp, audit_store, candidate_inbox, initialize_store, parse_markdown,
    replace_advisory_relation_projection, sync_store,
};

fn temporary_paths() -> StorePaths {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("stormbuffer-maintenance-{suffix}"));
    StorePaths { scope: StoreScope::Global, records: root.join("records"), cache: root.join("cache"), root }
}

fn fixture() -> stormbuffer_core::Record {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid/fact.md");
    parse_markdown(&path, &fs::read_to_string(&path).expect("read fixture")).expect("parse fixture")
}

#[test]
fn inbox_filters_candidates_and_reports_possible_overlap() {
    let paths = temporary_paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize store");
    let repository = RecordRepository::new(paths.clone());
    let active = repository.add(fixture()).expect("add active");
    let mut candidate = fixture();
    candidate.id = RecordId::new_v7();
    candidate.body = "A different body that may refine the same titled fact.".to_owned();
    let outcome = repository
        .propose(candidate, ProposalActor::Agent)
        .expect("propose candidate");
    let entries = candidate_inbox(
        &paths,
        &InboxFilter { kind: Some(RecordKind::Fact), possible_overlap: true, ..InboxFilter::default() },
    )
    .expect("read inbox");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, outcome.record_id);
    assert_eq!(entries[0].possible_overlap_id, Some(active.record().id.to_string()));
    fs::remove_dir_all(paths.root).expect("remove store");
}

#[test]
fn audit_reports_deterministic_and_relation_supported_findings_without_mutation() {
    let paths = temporary_paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize store");
    let repository = RecordRepository::new(paths.clone());
    let mut checkpoint = fixture();
    checkpoint.id = RecordId::new_v7();
    checkpoint.kind = RecordKind::Checkpoint;
    checkpoint.title = "Old checkpoint".to_owned();
    checkpoint.created_at = Timestamp::parse("2020-01-01T00:00:00Z").expect("timestamp");
    checkpoint.updated_at = checkpoint.created_at;
    checkpoint.supersedes = vec![RecordId::new_v7()];
    let checkpoint = repository.add(checkpoint).expect("add checkpoint");

    let mut candidate = fixture();
    candidate.id = RecordId::new_v7();
    candidate.title = "Pending candidate".to_owned();
    candidate.body = "Pending candidate body.".to_owned();
    let candidate = repository
        .propose(candidate, ProposalActor::Agent)
        .expect("propose candidate");
    sync_store(&paths).expect("build projection");
    replace_advisory_relation_projection(
        &paths,
        &[AdvisoryRelationProjection {
            left_record_id: checkpoint.record().id.to_string(),
            right_record_id: candidate.record_id,
            relation: "entails".to_owned(),
            evidence_json: "{\"reason\":\"same outcome\"}".to_owned(),
            confidence: "high".to_owned(),
            analyzer_fingerprint: "test".to_owned(),
        }],
    )
    .expect("write advisory relation");
    let before: Vec<PathBuf> = repository
        .list(true)
        .expect("list before")
        .into_iter()
        .map(|stored| stored.path().to_path_buf())
        .collect();
    let report = audit_store(&paths, 30).expect("audit store");
    let kinds: Vec<_> = report.findings.iter().map(|finding| finding.kind.as_str()).collect();
    assert!(kinds.contains(&"unresolved_candidate"));
    assert!(kinds.contains(&"broken_link"));
    assert!(kinds.contains(&"stale_checkpoint"));
    assert!(kinds.contains(&"relation_duplicate_or_refinement"));
    assert!(
        report
            .findings
            .iter()
            .all(|finding| finding.follow_up.starts_with("sbuf --global "))
    );
    for path in before {
        assert!(path.is_file());
        let record = parse_markdown(&path, &fs::read_to_string(&path).expect("read after")).expect("parse after");
        assert!(matches!(record.status, RecordStatus::Active | RecordStatus::Candidate));
    }
    fs::remove_dir_all(paths.root).expect("remove store");
}

#[test]
fn audit_rejects_unrepresentable_staleness_without_panicking() {
    let paths = temporary_paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize store");
    let error = audit_store(&paths, u64::MAX)
        .expect_err("oversized duration must fail")
        .to_string();
    assert!(error.contains("--stale-after-days is too large"), "{error}");
    fs::remove_dir_all(paths.root).expect("remove store");
}
