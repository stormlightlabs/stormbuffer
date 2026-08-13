use std::fs;
use std::path::{Path, PathBuf};

use stormbuffer_core::{
    Access, RecordKind, RecordStatus, Scope, Timestamp, parse_markdown, render_markdown,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn valid_fixtures_round_trip_metadata_and_body() {
    let cases = [
        ("valid/fact.md", RecordKind::Fact),
        ("valid/decision.md", RecordKind::Decision),
        ("valid/procedure.md", RecordKind::Procedure),
        ("valid/checkpoint.md", RecordKind::Checkpoint),
    ];

    for (fixture_name, expected_kind) in cases {
        let path = fixture(fixture_name);
        let markdown = fs::read_to_string(&path).expect("read valid fixture");
        let record = parse_markdown(&path, &markdown).expect("parse valid fixture");
        assert_eq!(record.kind, expected_kind, "{fixture_name}");

        let rendered = render_markdown(&record).expect("render valid fixture");
        let reparsed = parse_markdown(&path, &rendered).expect("parse rendered fixture");
        assert_eq!(
            reparsed, record,
            "metadata or body changed for {fixture_name}"
        );
    }
}

#[test]
fn fixture_preserves_unicode_code_blocks_and_multiple_sources() {
    let path = fixture("valid/decision.md");
    let markdown = fs::read_to_string(&path).expect("read decision fixture");
    let record = parse_markdown(&path, &markdown).expect("parse decision fixture");

    assert_eq!(record.sources.len(), 2);
    assert_eq!(
        record.sources[0].observed_at.map(|value| value.to_string()),
        Some("2026-08-05T20:08:00-05:00".to_owned())
    );
    assert_eq!(
        record.sources[0].revision.as_deref(),
        Some("session-revision-7")
    );
    assert_eq!(
        record.sources[0].content_hash.as_deref(),
        Some("blake3:4d8f1c")
    );
    assert_eq!(record.sources[1].revision.as_deref(), Some("git:9f2c11a"));
    assert!(record.sources[1].observed_at.is_none());
    assert!(record.sources[1].content_hash.is_none());
    assert!(record.aliases.iter().any(|alias| alias.contains("唯一")));
    assert!(record.body.contains("```rust\n"));
    assert!(record.body.contains("record.validate()?"));

    let rendered = render_markdown(&record).expect("render decision fixture");
    assert!(rendered.contains("唯一の書き込み境界"));
    assert!(rendered.contains("fn commit(record: &Record)"));
}

#[test]
fn malformed_fixtures_fail_with_field_and_file_context() {
    let cases = [
        ("malformed/missing-title.md", "missing field `title`"),
        ("malformed/unknown-field.md", "unknown field `importance`"),
        ("malformed/incompatible-version.md", "unsupported"),
        (
            "malformed/invalid-status.md",
            "field `status` is invalid: must be one of",
        ),
    ];

    for (fixture_name, expected_message) in cases {
        let path = fixture(fixture_name);
        let markdown = fs::read_to_string(&path).expect("read malformed fixture");
        let error = parse_markdown(&path, &markdown)
            .expect_err("malformed fixture unexpectedly parsed")
            .to_string();
        assert!(
            error.contains(path.file_name().unwrap().to_string_lossy().as_ref()),
            "{error}"
        );
        assert!(error.contains(expected_message), "{error}");
        assert!(!error.contains(path.parent().unwrap().to_string_lossy().as_ref()));
    }
}

#[test]
fn lifecycle_transitions_follow_the_record_contract() {
    assert!(RecordStatus::Candidate.can_transition_to(RecordStatus::Active));
    assert!(!RecordStatus::Candidate.can_transition_to(RecordStatus::Superseded));
    assert!(RecordStatus::Active.can_transition_to(RecordStatus::Archived));
    assert!(RecordStatus::Archived.can_transition_to(RecordStatus::Active));
    assert!(!RecordStatus::Superseded.can_transition_to(RecordStatus::Active));

    let path = fixture("valid/procedure.md");
    let markdown = fs::read_to_string(&path).expect("read procedure fixture");
    let mut record = parse_markdown(&path, &markdown).expect("parse procedure fixture");
    record
        .transition_to(RecordStatus::Active)
        .expect("activate record");
    record
        .transition_to(RecordStatus::Archived)
        .expect("archive record");
    record
        .transition_to(RecordStatus::Active)
        .expect("restore record");
    assert!(record.transition_to(RecordStatus::Superseded).is_ok());
    assert!(record.transition_to(RecordStatus::Active).is_err());
}

#[test]
fn typed_boundaries_reject_invalid_values() {
    assert!(stormbuffer_core::RecordId::parse("").is_err());
    assert!(Scope::parse("project:bad scope").is_err());
    assert!(Timestamp::parse("not-a-timestamp").is_err());
    assert!("operator".parse::<Access>().is_err());
}
