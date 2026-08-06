use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

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
    Command::new(binary(name))
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("run CLI process")
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
        .env("XDG_CACHE_HOME", &cache);
}

#[test]
fn init_root_and_status_work_for_project_and_global_stores() {
    let root = temporary_directory("stores");
    let project = root.join("project");
    fs::create_dir_all(&project).expect("create project directory");

    let project_init = run("stormbuffer", &project, ["--project", "init"]);
    assert_eq!(project_init.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&project_init.stdout).contains("Initialized project store"));
    assert!(project.join(".stormbuffer/store.toml").is_file());

    let project_root = run("stormbuffer", &project, ["--project", "root"]);
    assert_eq!(project_root.status.code(), Some(0));
    let expected_project_root = project
        .join(".stormbuffer")
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
fn invalid_input_and_unfinished_commands_are_explicit_and_safe() {
    let root = temporary_directory("errors");

    let invalid = run("stormbuffer", &root, ["status", "--project", "extra"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("unexpected argument"));
    assert!(!String::from_utf8_lossy(&invalid.stderr).contains("panicked"));

    let stub = run("stormbuffer", &root, ["--project", "add"]);
    assert_eq!(stub.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&stub.stderr).contains("not implemented yet"));
    assert!(stub.stdout.is_empty());
    assert!(!root.join(".stormbuffer").exists());

    let forget = run("stormbuffer", &root, ["forget", "memory-id"]);
    assert_eq!(forget.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&forget.stderr).contains("--destroy"));
    assert!(!root.join(".stormbuffer").exists());

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
