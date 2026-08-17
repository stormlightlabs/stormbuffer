use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use stormbuffer_core::run_synthetic_capture_policy_evaluation;

static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

#[test]
fn installed_skill_and_capture_evaluation_share_the_policy_contract() {
    let root = temporary_root();
    let skills = root.join("skills");
    fs::create_dir_all(&root).expect("create temporary root");

    let output = Command::new(env!("CARGO_BIN_EXE_sbuf"))
        .args([
            "--project",
            "skill",
            "install",
            "--directory",
            skills.to_str().expect("UTF-8 test path"),
        ])
        .current_dir(&root)
        .output()
        .expect("install packaged skill");
    assert!(
        output.status.success(),
        "skill installation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let installed = fs::read_to_string(skills.join("stormbuffer-memory/SKILL.md")).expect("read installed skill");
    for term in [
        "stormbuffer-capture-v1",
        "durable_correction",
        "accepted_decision",
        "tentative_discussion",
        "routine_completion",
        "repository_authoritative_knowledge",
        "confirmed_root_cause",
        "necessary_handoff",
        "abstain",
        "propose",
        "update",
        "checkpoint",
        "existing_memory_is_stale",
        "durable_accepted_decision",
        "tentative_or_unsettled",
        "no_capture_event",
        "repository_already_preserves_knowledge",
        "durable_confirmed_root_cause",
        "cross_session_state_is_not_recoverable",
    ] {
        assert!(installed.contains(term), "installed skill lacks {term}");
    }

    let report = run_synthetic_capture_policy_evaluation().expect("evaluate host assessments");
    assert_eq!(report.policy_revision, "stormbuffer-capture-v1");
    assert_eq!(report.scenario_count, 8);
    assert!(report.passed);

    fs::remove_dir_all(root).expect("remove temporary root");
}

fn temporary_root() -> PathBuf {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
    let counter = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "stormbuffer-skill-policy-{}-{stamp}-{counter}",
        std::process::id()
    ))
}
