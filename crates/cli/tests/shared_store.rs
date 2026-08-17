use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_root() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("stormbuffer-shared-store-{suffix}"));
    fs::create_dir_all(&root).expect("create temporary root");
    root
}

fn binary() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_sbuf")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .expect("locate test executable")
                .parent()
                .and_then(Path::parent)
                .expect("locate debug directory")
                .join("sbuf")
        })
}

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create destination");
    for entry in fs::read_dir(source).expect("read shared store") {
        let entry = entry.expect("read shared store entry");
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_directory(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy shared store file");
        }
    }
}

#[test]
fn committed_shared_store_rebuilds_from_tracked_files() {
    let root = temporary_root();
    let project = root.join("agent-memory");
    fs::create_dir_all(&project).expect("create project");
    let committed_store = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.sbuf");
    fs::create_dir_all(project.join(".sbuf/records")).expect("create store records");
    for name in ["store.toml", ".gitignore"] {
        fs::copy(committed_store.join(name), project.join(".sbuf").join(name)).expect("copy committed store metadata");
    }
    copy_directory(&committed_store.join("records"), &project.join(".sbuf/records"));

    let ignore = fs::read_to_string(project.join(".sbuf/.gitignore")).expect("read ignore rules");
    assert_eq!(
        ignore.lines().collect::<Vec<_>>(),
        [
            "# Track only configuration, ignore rules, and canonical Markdown records.",
            "*",
            "!.gitignore",
            "!store.toml",
            "!records/",
            "!records/**/",
            "!records/**/*.md",
        ]
    );

    const CANONICAL_RECORD_ID: &str = "019fd5d7-6e0c-7d93-b9fe-54b02f7f11e9";
    assert!(
        project
            .join(".sbuf/records")
            .join(format!("{CANONICAL_RECORD_ID}.md"))
            .is_file()
    );

    let command = |args: &[&str]| {
        Command::new(binary())
            .current_dir(&project)
            .args(args)
            .env("HOME", root.join("home"))
            .env("USERPROFILE", root.join("home"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env("STORMBUFFER_TEST_MODE", "1")
            .output()
            .expect("run shared-store command")
    };
    let sync = command(&["--project", "sync"]);
    assert!(sync.status.success(), "{}", String::from_utf8_lossy(&sync.stderr));
    let query = "canonical records survive projection failures";
    let search = command(&["--project", "search", query, "--json"]);
    assert!(search.status.success(), "{}", String::from_utf8_lossy(&search.stderr));
    let results: serde_json::Value = serde_json::from_slice(&search.stdout).expect("parse search");
    assert!(
        results
            .as_array()
            .is_some_and(|results| { results.iter().any(|result| result["record_id"] == CANONICAL_RECORD_ID) })
    );
    let context = command(&["--project", "context", query, "--budget", "80"]);
    assert!(context.status.success(), "{}", String::from_utf8_lossy(&context.stderr));
    let context: serde_json::Value = serde_json::from_slice(&context.stdout).expect("parse context");
    assert!(
        context["blocks"]
            .as_array()
            .is_some_and(|blocks| { blocks.iter().any(|block| block["record_id"] == CANONICAL_RECORD_ID) })
    );
    assert!(project.join(".sbuf/index.sqlite3").is_file());

    fs::remove_dir_all(root).expect("remove temporary root");
}
