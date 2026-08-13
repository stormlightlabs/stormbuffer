use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use stormbuffer_core::{
    EvidenceOutcome, ProposalFeedbackOutcome, parse_receipt_feedback_file,
    rebuild_receipt_feedback_projection,
};

const FEEDBACK_JSON: &str = include_str!("fixtures/evaluation/receipt-feedback.json");
static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

#[test]
fn checked_in_judgments_cover_every_feedback_outcome() {
    let feedback = parse_receipt_feedback_file(FEEDBACK_JSON).expect("parse feedback judgments");

    let evidence = feedback
        .judgments
        .iter()
        .flat_map(|judgment| judgment.evidence.iter())
        .map(|feedback| feedback.outcome)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        evidence,
        BTreeSet::from([
            EvidenceOutcome::Included,
            EvidenceOutcome::Cited,
            EvidenceOutcome::Ignored,
            EvidenceOutcome::Corrected,
        ])
    );
    let proposals = feedback
        .judgments
        .iter()
        .filter_map(|judgment| judgment.proposal.as_ref())
        .map(|feedback| feedback.outcome)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        proposals,
        BTreeSet::from([
            ProposalFeedbackOutcome::Approved,
            ProposalFeedbackOutcome::Edited,
            ProposalFeedbackOutcome::Rejected,
            ProposalFeedbackOutcome::Superseding,
            ProposalFeedbackOutcome::Duplicate,
        ])
    );
}

#[test]
fn rebuilt_projection_joins_by_receipt_and_contains_no_content_fields() {
    let root = temporary_root();
    fs::create_dir_all(&root).expect("create test root");
    let projection = root.join("receipt-feedback.sqlite3");
    let feedback = parse_receipt_feedback_file(FEEDBACK_JSON).expect("parse feedback judgments");

    let first = rebuild_receipt_feedback_projection(&projection, &feedback)
        .expect("build feedback projection");
    let second = rebuild_receipt_feedback_projection(&projection, &feedback)
        .expect("rebuild feedback projection");
    assert_eq!(first, second);
    assert_eq!(second.receipt_count, 6);
    assert_eq!(second.evidence_count, 5);
    assert_eq!(second.proposal_count, 5);

    let connection = Connection::open(&projection).expect("open rebuilt projection");
    let joined: Vec<(String, String)> = connection
        .prepare(
            "SELECT e.outcome, p.outcome
             FROM receipt_feedback r
             JOIN evidence_feedback e USING (receipt_id)
             JOIN proposal_feedback p USING (receipt_id)
             ORDER BY r.recorded_at",
        )
        .expect("prepare joined inspection")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("inspect joined feedback")
        .collect::<Result<_, _>>()
        .expect("read joined feedback");
    assert_eq!(
        joined,
        vec![
            ("included".to_owned(), "edited".to_owned()),
            ("ignored".to_owned(), "rejected".to_owned()),
            ("corrected".to_owned(), "superseding".to_owned()),
            ("cited".to_owned(), "duplicate".to_owned()),
        ]
    );

    let columns: BTreeSet<String> = ["receipt_feedback", "evidence_feedback", "proposal_feedback"]
        .into_iter()
        .flat_map(|table| {
            let mut statement = connection
                .prepare(&format!("PRAGMA table_info({table})"))
                .expect("prepare column inspection");
            statement
                .query_map([], |row| row.get(1))
                .expect("inspect projection columns")
                .collect::<Result<Vec<String>, _>>()
                .expect("read projection columns")
        })
        .collect();
    assert!(!columns.contains("query"));
    assert!(!columns.contains("prompt"));
    assert!(!columns.contains("answer"));
    assert!(!columns.contains("transcript"));

    drop(connection);
    fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn content_fields_are_rejected_from_judgments() {
    let with_prompt = FEEDBACK_JSON.replacen(
        "\"recorded_at\": \"2026-08-12T14:00:30Z\"",
        "\"recorded_at\": \"2026-08-12T14:00:30Z\", \"prompt\": \"secret\"",
        1,
    );
    let error = parse_receipt_feedback_file(&with_prompt).expect_err("reject prompt content");
    assert!(error.to_string().contains("unknown field `prompt`"));
}

fn temporary_root() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let counter = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "stormbuffer-receipt-feedback-{}-{stamp}-{counter}",
        std::process::id()
    ))
}
