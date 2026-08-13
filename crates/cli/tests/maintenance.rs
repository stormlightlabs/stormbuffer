use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_root() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let root = env::temp_dir().join(format!("stormbuffer-cli-maintenance-{suffix}"));
    fs::create_dir_all(&root).expect("create temporary root");
    root
}

fn binary() -> PathBuf {
    env::var_os("CARGO_BIN_EXE_sbuf")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::current_exe()
                .expect("locate test executable")
                .parent()
                .and_then(Path::parent)
                .expect("locate debug directory")
                .join("sbuf")
        })
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(binary())
        .current_dir(root)
        .args(args)
        .env("HOME", root.join("home"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("STORMBUFFER_TEST_MODE", "1")
        .env("EDITOR", "true")
        .output()
        .expect("run CLI")
}

#[test]
fn inbox_and_audit_have_human_and_json_read_only_views() {
    let root = temporary_root();
    assert!(run(&root, &["--global", "init"]).status.success());
    let proposed = run(
        &root,
        &[
            "--global",
            "propose",
            "--title",
            "Review this candidate",
            "--kind",
            "fact",
            "--body",
            "A candidate remains pending until a person decides.",
        ],
    );
    assert!(
        proposed.status.success(),
        "{}",
        String::from_utf8_lossy(&proposed.stderr)
    );
    let records = root.join("data/stormbuffer/records");
    let record = records
        .read_dir()
        .expect("read records")
        .next()
        .expect("candidate")
        .expect("entry")
        .path();
    let before = fs::read(&record).expect("read before audit");

    let human = run(&root, &["--global", "inbox", "--kind", "fact"]);
    assert!(
        human.status.success(),
        "{}",
        String::from_utf8_lossy(&human.stderr)
    );
    assert!(String::from_utf8_lossy(&human.stdout).contains("Candidates: 1"));
    let json = run(
        &root,
        &["--global", "inbox", "--source", "conversation", "--json"],
    );
    assert!(
        json.status.success(),
        "{}",
        String::from_utf8_lossy(&json.stderr)
    );
    let inbox: serde_json::Value = serde_json::from_slice(&json.stdout).expect("parse inbox JSON");
    assert_eq!(inbox.as_array().expect("inbox array").len(), 1);
    assert_eq!(inbox[0]["kind"], "fact");

    let audit = run(&root, &["--global", "audit", "--json"]);
    assert!(
        audit.status.success(),
        "{}",
        String::from_utf8_lossy(&audit.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&audit.stdout).expect("parse audit JSON");
    assert_eq!(report["findings"][0]["kind"], "unresolved_candidate");
    assert!(
        report["findings"][0]["follow_up"]
            .as_str()
            .expect("follow up")
            .contains("sbuf --global approve")
    );
    assert_eq!(fs::read(&record).expect("read after audit"), before);
    fs::remove_dir_all(root).expect("remove temporary root");
}
