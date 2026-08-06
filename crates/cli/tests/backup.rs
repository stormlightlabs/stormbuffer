use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_root() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let root = env::temp_dir().join(format!("stormbuffer-cli-backup-{suffix}"));
    fs::create_dir_all(&root).expect("create temporary root");
    root
}

fn binary() -> PathBuf {
    env::var_os("CARGO_BIN_EXE_stormbuffer")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::current_exe()
                .expect("locate test executable")
                .parent()
                .and_then(Path::parent)
                .expect("locate debug directory")
                .join("stormbuffer")
        })
}

fn run(root: &Path, args: &[&str]) -> Output {
    let home = root.join("home");
    let data = root.join("data");
    let cache = root.join("cache");
    Command::new(binary())
        .current_dir(root)
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", root.join("home"))
        .env("LOCALAPPDATA", &data)
        .env("APPDATA", &data)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CACHE_HOME", cache)
        .env("EDITOR", "true")
        .env("STORMBUFFER_TEST_MODE", "1")
        .output()
        .expect("run CLI")
}

#[test]
fn export_import_and_gc_are_explicit_and_safe() {
    let root = temporary_root();
    let source = root.join("source");
    let target = root.join("target");
    fs::create_dir_all(&source).expect("create source");
    fs::create_dir_all(&target).expect("create target");

    assert!(run(&source, &["--project", "init"]).status.success());
    let added = run(
        &source,
        &[
            "--project",
            "add",
            "--title",
            "Portable memory",
            "--kind",
            "fact",
            "--body",
            "The canonical body survives export.",
        ],
    );
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );

    let archive = root.join("backup.json");
    let archive_text = archive.to_string_lossy().into_owned();
    let exported = run(&source, &["--project", "export", &archive_text]);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let bundle: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&archive).expect("read archive"))
            .expect("parse archive");
    assert_eq!(bundle["format_version"], 1);
    assert!(
        bundle["records"][0]["markdown"]
            .as_str()
            .expect("archive Markdown")
            .contains("The canonical body")
    );

    let unsafe_archive = source.join(".sbuf/backup.json");
    let unsafe_archive_text = unsafe_archive.to_string_lossy().into_owned();
    let rejected = run(&source, &["--project", "export", &unsafe_archive_text]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("outside the selected store"));
    assert!(!unsafe_archive.exists());

    assert!(run(&target, &["--project", "init"]).status.success());
    let import_without_scope = run(&target, &["--project", "import", &archive_text]);
    assert!(!import_without_scope.status.success());
    assert!(String::from_utf8_lossy(&import_without_scope.stderr).contains("scope collision"));

    let remapped = run(
        &target,
        &["--project", "import", &archive_text, "--on-scope", "remap"],
    );
    assert!(
        remapped.status.success(),
        "{}",
        String::from_utf8_lossy(&remapped.stderr)
    );
    assert!(
        target
            .join(".sbuf/records")
            .read_dir()
            .expect("read imported records")
            .next()
            .is_some()
    );

    let target_archive = root.join("target.json");
    let target_archive_text = target_archive.to_string_lossy().into_owned();
    assert!(
        run(&target, &["--project", "export", &target_archive_text])
            .status
            .success()
    );
    let skipped = run(
        &target,
        &[
            "--project",
            "import",
            &target_archive_text,
            "--on-existing",
            "skip",
        ],
    );
    assert!(
        skipped.status.success(),
        "{}",
        String::from_utf8_lossy(&skipped.stderr)
    );
    assert!(String::from_utf8_lossy(&skipped.stdout).contains("Skipped: 1"));

    assert!(run(&target, &["--project", "sync"]).status.success());
    let index = target.join(".sbuf/index.sqlite3");
    assert!(index.is_file());
    let dry_run = run(&target, &["--project", "gc", "--dry-run"]);
    assert!(
        dry_run.status.success(),
        "{}",
        String::from_utf8_lossy(&dry_run.stderr)
    );
    assert!(index.is_file());
    let actual = run(&target, &["--project", "gc"]);
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert!(!index.exists());
    assert!(
        target
            .join(".sbuf/records")
            .read_dir()
            .unwrap()
            .next()
            .is_some()
    );

    fs::remove_dir_all(root).expect("remove temporary root");
}
