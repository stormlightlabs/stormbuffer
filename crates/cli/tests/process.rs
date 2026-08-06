use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

// TODO: use the tempdir crate
fn temporary_directory(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let path = env::temp_dir().join(format!("stormbuffer-cli-{name}-{suffix}"));
    fs::create_dir_all(&path).expect("create test directory");
    path
}

fn binary(name: &str) -> PathBuf {
    if let Some(path) = env::var_os(format!("CARGO_BIN_EXE_{name}")) {
        return PathBuf::from(path);
    }

    let test_binary = env::current_exe().expect("locate the process test binary");
    let target_debug = test_binary
        .parent()
        .and_then(Path::parent)
        .expect("locate Cargo's debug directory");
    let mut path = target_debug.join(name);
    if cfg!(windows) {
        path.set_extension("exe");
    }
    assert!(
        path.is_file(),
        "locate the {name} test binary at {}",
        path.display()
    );
    path
}

fn run<I, S>(name: &str, directory: &Path, arguments: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let home = directory.join("home");
    let data = directory.join("data");
    let cache = directory.join("cache");
    Command::new(binary(name))
        .current_dir(directory)
        .args(arguments)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("LOCALAPPDATA", &data)
        .env("APPDATA", &data)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CACHE_HOME", &cache)
        .env("STORMBUFFER_TEST_MODE", "1")
        .output()
        .expect("run CLI process")
}

fn run_with_editor<I, S>(name: &str, directory: &Path, arguments: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let home = directory.join("home");
    let data = directory.join("data");
    let cache = directory.join("cache");
    let mut command = Command::new(binary(name));
    command
        .current_dir(directory)
        .args(arguments)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("LOCALAPPDATA", &data)
        .env("APPDATA", &data)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CACHE_HOME", &cache)
        .env("EDITOR", "true")
        .env("STORMBUFFER_TEST_MODE", "1");
    command.output().expect("run CLI with editor")
}

fn with_store_environment(command: &mut Command, root: &Path) {
    let home = root.join("home");
    let data = root.join("data");
    let cache = root.join("cache");
    command
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("LOCALAPPDATA", &data)
        .env("APPDATA", &data)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CACHE_HOME", &cache)
        .env("STORMBUFFER_TEST_MODE", "1");
}

fn run_with_store_environment<I, S>(
    name: &str,
    directory: &Path,
    root: &Path,
    arguments: I,
) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(binary(name));
    command
        .current_dir(directory)
        .args(arguments)
        .env("EDITOR", "true");
    with_store_environment(&mut command, root);
    command.output().expect("run CLI with isolated store")
}

#[test]
fn init_root_and_status_work_for_project_and_global_stores() {
    let root = temporary_directory("stores");
    let project = root.join("project");
    fs::create_dir_all(&project).expect("create project directory");

    let project_init = run("stormbuffer", &project, ["--project", "init"]);
    assert_eq!(project_init.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&project_init.stdout).contains("Initialized project store"));
    assert!(project.join(".sbuf/store.toml").is_file());

    let project_root = run("stormbuffer", &project, ["--project", "root"]);
    assert_eq!(project_root.status.code(), Some(0));
    let expected_project_root = project
        .join(".sbuf")
        .canonicalize()
        .expect("canonicalize project root");
    assert_eq!(
        String::from_utf8_lossy(&project_root.stdout).trim(),
        expected_project_root.to_string_lossy()
    );

    let mut global_command = Command::new(binary("stormbuffer"));
    global_command.current_dir(&project).arg("init");
    with_store_environment(&mut global_command, &root);
    let global_init = global_command.output().expect("run global init");
    assert_eq!(global_init.status.code(), Some(0));
    assert!(root.join("data/stormbuffer/store.toml").is_file());

    let mut global_status_command = Command::new(binary("stormbuffer"));
    global_status_command
        .current_dir(&project)
        .args(["status", "--json"]);
    with_store_environment(&mut global_status_command, &root);
    let global_status = global_status_command.output().expect("run global status");
    assert_eq!(global_status.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&global_status.stdout).contains("\"initialized\":true"));

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn shared_project_init_is_explicit_and_global_shared_is_rejected() {
    let root = temporary_directory("shared");
    let project = root.join("project");
    fs::create_dir_all(&project).expect("create project directory");

    let global_shared = run("stormbuffer", &root, ["init", "--shared"]);
    assert_eq!(global_shared.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&global_shared.stderr).contains("project scope"));
    assert!(!root.join("data/stormbuffer").exists());

    let global_flag = run("stormbuffer", &root, ["--shared", "init"]);
    assert_eq!(global_flag.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&global_flag.stderr).contains("unexpected argument"));
    assert!(!root.join("data/stormbuffer").exists());

    let shared_init = run("stormbuffer", &project, ["--project", "init", "--shared"]);
    assert_eq!(shared_init.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&shared_init.stdout).contains("shared"));
    assert!(project.join(".sbuf/store.toml").is_file());
    let ignore =
        fs::read_to_string(project.join(".sbuf/.gitignore")).expect("read shared ignore rules");
    assert!(ignore.contains("/models/"));
    assert!(ignore.contains("**/*.sqlite*"));
    assert!(ignore.contains("**/*.lock"));
    assert!(ignore.contains("**/*.tmp"));
    assert!(ignore.contains("**/*.log"));

    let status = run("stormbuffer", &project, ["--project", "status"]);
    assert_eq!(status.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&status.stdout).contains("Visibility: shared"));

    let status_json = run("stormbuffer", &project, ["--project", "status", "--json"]);
    assert_eq!(status_json.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&status_json.stdout).contains("\"visibility\":\"shared\""));

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn invalid_input_and_unfinished_commands_are_explicit_and_safe() {
    let root = temporary_directory("errors");

    let invalid = run("stormbuffer", &root, ["status", "--project", "extra"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("unexpected argument"));
    assert!(!String::from_utf8_lossy(&invalid.stderr).contains("panicked"));

    let add = run("stormbuffer", &root, ["--project", "add"]);
    assert_eq!(add.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&add.stderr).contains("not initialized"));
    assert!(add.stdout.is_empty());
    assert!(!root.join(".sbuf").exists());

    let forget = run("stormbuffer", &root, ["forget", "memory-id"]);
    assert_eq!(forget.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&forget.stderr).contains("--destroy"));
    assert!(!root.join(".sbuf").exists());

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn lifecycle_commands_preserve_records_and_use_tab_delimited_output() {
    let root = temporary_directory("lifecycle");
    let init = run("stormbuffer", &root, ["--project", "init"]);
    assert_eq!(init.status.code(), Some(0));

    let add = run_with_editor(
        "stormbuffer",
        &root,
        [
            "--project",
            "add",
            "--title",
            "A durable fact",
            "--kind",
            "fact",
            "--body",
            "The body stays readable.",
        ],
    );
    assert_eq!(
        add.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let id = String::from_utf8_lossy(&add.stdout).trim().to_owned();
    assert!(!id.is_empty());

    let list = run("stormbuffer", &root, ["--project", "list"]);
    assert_eq!(list.status.code(), Some(0));
    let line = String::from_utf8_lossy(&list.stdout);
    let fields: Vec<_> = line.trim_end().split('\t').collect();
    assert_eq!(fields.len(), 5, "unexpected list output: {line:?}");
    assert_eq!(fields[0], id);
    assert_eq!(fields[1], "active");
    assert_eq!(fields[2], "fact");
    assert!(fields[3].starts_with("project:"));
    assert_eq!(fields[4], "A durable fact");
    assert!(!line.contains("\\t"));

    let show = run("stormbuffer", &root, ["--project", "show", &id]);
    assert_eq!(show.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&show.stdout).contains("The body stays readable."));

    let edit = run_with_editor("stormbuffer", &root, ["--project", "edit", &id]);
    assert_eq!(edit.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&edit.stdout).trim(), id);

    let archive = run("stormbuffer", &root, ["--project", "archive", &id]);
    assert_eq!(archive.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&archive.stdout).trim(),
        format!("{id}\tarchived")
    );
    assert!(
        run("stormbuffer", &root, ["--project", "list"])
            .stdout
            .is_empty()
    );
    let all = run("stormbuffer", &root, ["--project", "list", "--all"]);
    assert!(String::from_utf8_lossy(&all.stdout).contains(&format!("{id}\tarchived")));

    let restore = run("stormbuffer", &root, ["--project", "restore", &id]);
    assert_eq!(restore.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&restore.stdout).trim(),
        format!("{id}\tactive")
    );

    let supersede = run_with_editor("stormbuffer", &root, ["--project", "supersede", &id]);
    assert_eq!(
        supersede.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&supersede.stderr)
    );
    let replacement = String::from_utf8_lossy(&supersede.stdout).trim().to_owned();
    assert_ne!(replacement, id);
    let active = run("stormbuffer", &root, ["--project", "list"]);
    let active_output = String::from_utf8_lossy(&active.stdout);
    assert!(active_output.contains(&replacement));
    assert!(!active_output.contains(&id));

    let blocked_forget = run(
        "stormbuffer",
        &root,
        ["--project", "forget", &replacement, "--destroy"],
    );
    assert_eq!(blocked_forget.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&blocked_forget.stderr).contains("--yes"));
    let forgotten = run(
        "stormbuffer",
        &root,
        ["--project", "forget", &replacement, "--destroy", "--yes"],
    );
    assert_eq!(forgotten.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&forgotten.stdout).contains("Forgot"));

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn project_search_reconciles_and_prioritizes_initialized_global_memory() {
    let root = temporary_directory("search-scopes");
    let project = root.join("demo");
    fs::create_dir_all(&project).expect("create project directory");

    for arguments in [vec!["init"], vec!["--project", "init"]] {
        let output = run_with_store_environment("stormbuffer", &project, &root, arguments);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let global_add = run_with_store_environment(
        "stormbuffer",
        &project,
        &root,
        [
            "add",
            "--title",
            "Global collision",
            "--kind",
            "fact",
            "--body",
            "scope collision from global memory",
        ],
    );
    assert_eq!(global_add.status.code(), Some(0));
    let project_add = run_with_store_environment(
        "stormbuffer",
        &project,
        &root,
        [
            "--project",
            "add",
            "--title",
            "Project collision",
            "--kind",
            "fact",
            "--body",
            "scope collision from project memory",
        ],
    );
    assert_eq!(project_add.status.code(), Some(0));

    let search = run_with_store_environment(
        "stormbuffer",
        &project,
        &root,
        ["--project", "search", "scope collision", "--json"],
    );
    assert_eq!(
        search.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&search.stderr)
    );
    let results: serde_json::Value =
        serde_json::from_slice(&search.stdout).expect("parse search results");
    let results = results.as_array().expect("search result array");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["scope"], "project:demo");
    assert_eq!(results[1]["scope"], "global");

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn aliases_share_commands_and_help_uses_the_invoked_name() {
    let root = temporary_directory("aliases");
    for name in ["stormbuffer", "stormbuf", "sbuf"] {
        let version = run(name, &root, ["--version"]);
        assert_eq!(version.status.code(), Some(0), "{name}");
        assert!(String::from_utf8_lossy(&version.stdout).contains("0.1.0"));

        let help = run(name, &root, ["--help"]);
        assert_eq!(help.status.code(), Some(0), "{name}");
        assert!(
            String::from_utf8_lossy(&help.stdout).contains(&format!("Usage: {name}")),
            "{}",
            String::from_utf8_lossy(&help.stdout)
        );

        let status = run(name, &root, ["--project", "status"]);
        assert_eq!(status.status.code(), Some(0), "{name}");
        assert!(String::from_utf8_lossy(&status.stdout).contains("State: not initialized"));
    }
    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn color_modes_no_color_and_json_output_follow_the_contract() {
    let root = temporary_directory("color");
    let init = run("stormbuffer", &root, ["--project", "init"]);
    assert_eq!(init.status.code(), Some(0));
    assert!(!init.stdout.contains(&0x1b));

    let auto = run(
        "stormbuffer",
        &root,
        ["--project", "--color", "auto", "status"],
    );
    assert_eq!(auto.status.code(), Some(0));
    assert!(!auto.stdout.contains(&0x1b));

    let never = run(
        "stormbuffer",
        &root,
        ["--project", "--color", "never", "status"],
    );
    assert_eq!(never.status.code(), Some(0));
    assert!(!never.stdout.contains(&0x1b));

    let always = run(
        "stormbuffer",
        &root,
        ["--project", "--color", "always", "status"],
    );
    assert_eq!(always.status.code(), Some(0));
    assert!(always.stdout.contains(&0x1b));

    let mut no_color_command = Command::new(binary("stormbuffer"));
    no_color_command
        .current_dir(&root)
        .args(["--project", "--color", "auto", "status"])
        .env("NO_COLOR", "1");
    let no_color = no_color_command.output().expect("run NO_COLOR status");
    assert_eq!(no_color.status.code(), Some(0));
    assert!(!no_color.stdout.contains(&0x1b));

    let json = run(
        "stormbuffer",
        &root,
        ["--project", "--color", "always", "status", "--json"],
    );
    assert_eq!(json.status.code(), Some(0));
    assert!(!json.stdout.contains(&0x1b));
    assert!(String::from_utf8_lossy(&json.stdout).starts_with('{'));

    fs::remove_dir_all(root).expect("remove test directory");
}
