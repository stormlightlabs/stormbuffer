use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
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

fn binary() -> PathBuf {
    if let Some(path) = env::var_os("CARGO_BIN_EXE_sbuf") {
        return PathBuf::from(path);
    }

    let test_binary = env::current_exe().expect("locate the process test binary");
    let target_debug = test_binary
        .parent()
        .and_then(Path::parent)
        .expect("locate Cargo's debug directory");
    let mut path = target_debug.join("sbuf");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    assert!(path.is_file(), "locate the sbuf test binary at {}", path.display());
    path
}

fn run<I, S>(directory: &Path, arguments: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let home = directory.join("home");
    let data = directory.join("data");
    let cache = directory.join("cache");
    Command::new(binary())
        .current_dir(directory)
        .args(arguments)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("LOCALAPPDATA", &data)
        .env("APPDATA", &data)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CACHE_HOME", &cache)
        .env("STORMBUFFER_TEST_MODE", "1")
        .env_remove("NO_COLOR")
        .output()
        .expect("run CLI process")
}

fn run_json<I, S>(directory: &Path, arguments: I, input: &str) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_json_with_store_environment(directory, directory, arguments, input)
}

fn run_json_with_store_environment<I, S>(directory: &Path, root: &Path, arguments: I, input: &str) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(binary());
    command
        .current_dir(directory)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    with_store_environment(&mut command, root);
    let mut child = command.spawn().expect("run CLI JSON protocol");
    child
        .stdin
        .take()
        .expect("open JSON stdin")
        .write_all(input.as_bytes())
        .expect("write JSON request");
    child.wait_with_output().expect("collect JSON protocol output")
}

fn run_with_editor<I, S>(directory: &Path, arguments: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let home = directory.join("home");
    let data = directory.join("data");
    let cache = directory.join("cache");
    let mut command = Command::new(binary());
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

fn run_with_store_environment<I, S>(directory: &Path, root: &Path, arguments: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(binary());
    command.current_dir(directory).args(arguments).env("EDITOR", "true");
    with_store_environment(&mut command, root);
    command.output().expect("run CLI with isolated store")
}

#[test]
fn init_root_and_status_work_for_project_and_global_stores() {
    let root = temporary_directory("stores");
    let project = root.join("project");
    fs::create_dir_all(&project).expect("create project directory");

    let project_init = run(&project, ["--project", "init"]);
    assert_eq!(project_init.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&project_init.stdout).contains("Initialized project store"));
    assert!(project.join(".sbuf/store.toml").is_file());
    assert!(project.join(".sbuf/index.sqlite3").is_file());

    let project_root = run(&project, ["--project", "root"]);
    assert_eq!(project_root.status.code(), Some(0));
    let expected_project_root = project.join(".sbuf").canonicalize().expect("canonicalize project root");
    assert_eq!(
        String::from_utf8_lossy(&project_root.stdout).trim(),
        expected_project_root.to_string_lossy()
    );
    let project_status = run(&project, ["--project", "status", "--json"]);
    let project_status: serde_json::Value =
        serde_json::from_slice(&project_status.stdout).expect("parse project status");
    assert_eq!(project_status["view"], "project with applicable global memory");
    assert_eq!(project_status["scope"], "project");
    assert!(project_status["lifecycle"].is_object());
    assert!(project_status["disk_usage"].is_object());
    assert!(project_status["index_version"].is_number());

    let local_status = run(&project, ["--local", "status", "--json"]);
    let local_status: serde_json::Value = serde_json::from_slice(&local_status.stdout).expect("parse local status");
    assert_eq!(local_status["view"], "strict local");
    assert_eq!(local_status["scope"], "local");

    let mut global_command = Command::new(binary());
    global_command.current_dir(&project).args(["-g", "init"]);
    with_store_environment(&mut global_command, &root);
    let global_init = global_command.output().expect("run global init");
    assert_eq!(global_init.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&global_init.stdout).contains("private"));
    assert!(root.join("data/stormbuffer/store.toml").is_file());
    assert!(root.join("cache/stormbuffer/global.sqlite3").is_file());

    let mut global_status_command = Command::new(binary());
    global_status_command.current_dir(&project).args(["status", "--json"]);
    with_store_environment(&mut global_status_command, &root);
    let global_status = global_status_command.output().expect("run global status");
    assert_eq!(global_status.status.code(), Some(0));
    let global_status: serde_json::Value = serde_json::from_slice(&global_status.stdout).expect("parse global status");
    assert_eq!(global_status["view"], "global");
    assert_eq!(global_status["scope"], "global");
    assert_eq!(global_status["initialized"], true);

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn doctor_reports_readiness_and_an_actionable_semantic_setup() {
    let root = temporary_directory("doctor");
    let init = run(&root, ["--project", "init"]);
    assert_eq!(init.status.code(), Some(0));

    let doctor = run(&root, ["--project", "--color", "always", "doctor"]);
    assert_eq!(doctor.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&doctor.stdout);
    assert!(
        stdout
            .contains("\u{1b}[1m\u{1b}[36mSemantic retrieval\u{1b}[39m\u{1b}[0m: \u{1b}[1m\u{1b}[33mlexical fallback"),
        "{stdout}"
    );
    assert!(stdout.contains("0 failure(s), 1 warning(s)"), "{stdout}");
    assert!(
        stdout.contains("semantic retrieval is not ready; search is using lexical matching only"),
        "{stdout}"
    );
    assert!(
        stdout.contains("run `sbuf init` while online to download and verify the local model"),
        "{stdout}"
    );
    assert!(doctor.stdout.contains(&0x1b), "warning should use Echo color");
    assert!(!stdout.contains("(repair:"), "{stdout}");

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn doctor_repair_recovers_disposable_projection_and_metadata() {
    let root = temporary_directory("doctor-repair");
    let init = run(&root, ["--project", "init"]);
    assert_eq!(init.status.code(), Some(0));
    let store = root.join(".sbuf");
    fs::write(store.join("index.sqlite3"), b"corrupt").expect("corrupt projection");
    fs::create_dir_all(store.join("tmp")).expect("create temp directory");
    fs::write(store.join("tmp/stale"), b"stale").expect("write stale metadata");

    let repair = run(&root, ["--project", "doctor", "--repair"]);
    assert_eq!(
        repair.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&repair.stderr)
    );
    let output = String::from_utf8_lossy(&repair.stdout);
    assert!(output.contains("Repaired: rebuilt the disposable search projection"));
    assert!(output.contains("Repaired: removed stale disposable metadata"));
    assert!(!store.join("tmp/stale").exists());

    let repeated = run(&root, ["--project", "doctor", "--repair"]);
    assert_eq!(repeated.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&repeated.stdout).contains("Repaired:"));
    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn shared_project_init_is_explicit_and_global_shared_is_rejected() {
    let root = temporary_directory("shared");
    let project = root.join("project");
    fs::create_dir_all(&project).expect("create project directory");

    let global_shared = run(&root, ["init", "--shared"]);
    assert_eq!(global_shared.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&global_shared.stderr).contains("project scope"));
    assert!(!root.join("data/stormbuffer").exists());

    let global_flag = run(&root, ["--shared", "init"]);
    assert_eq!(global_flag.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&global_flag.stderr).contains("unexpected argument"));
    assert!(!root.join("data/stormbuffer").exists());

    let shared_init = run(&project, ["--project", "init", "--shared"]);
    assert_eq!(shared_init.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&shared_init.stdout).contains("shared"));
    assert!(project.join(".sbuf/store.toml").is_file());
    let ignore = fs::read_to_string(project.join(".sbuf/.gitignore")).expect("read shared ignore rules");
    for pattern in [
        "*",
        "!.gitignore",
        "!store.toml",
        "!records/",
        "!records/**/",
        "!records/**/*.md",
    ] {
        assert!(ignore.lines().any(|line| line == pattern));
    }

    let status = run(&project, ["--project", "status"]);
    assert_eq!(status.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&status.stdout).contains("Visibility: shared"));

    let status_json = run(&project, ["--project", "status", "--json"]);
    assert_eq!(status_json.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&status_json.stdout).contains("\"visibility\":\"shared\""));

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn invalid_input_and_unfinished_commands_are_explicit_and_safe() {
    let root = temporary_directory("errors");

    let invalid = run(&root, ["status", "--project", "extra"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("unexpected argument"));
    assert!(!String::from_utf8_lossy(&invalid.stderr).contains("panicked"));

    let add = run(&root, ["--project", "add"]);
    assert_eq!(add.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&add.stderr).contains("not initialized"));
    assert!(add.stdout.is_empty());
    assert!(!root.join(".sbuf").exists());

    let invoke = run_json(
        &root,
        ["--project", "invoke", "search"],
        r#"{"version":1,"query":"anything"}"#,
    );
    assert_eq!(invoke.status.code(), Some(1));
    let envelope: serde_json::Value = serde_json::from_slice(&invoke.stdout).expect("parse not initialized envelope");
    assert_eq!(envelope["error"]["code"], "not_initialized");
    assert!(!root.join(".sbuf").exists());

    let forget = run(&root, ["forget", "memory-id"]);
    assert_eq!(forget.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&forget.stderr).contains("--destroy"));
    assert!(!root.join(".sbuf").exists());

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn lifecycle_commands_preserve_records_and_use_tab_delimited_output() {
    let root = temporary_directory("lifecycle");
    let init = run(&root, ["--project", "init"]);
    assert_eq!(init.status.code(), Some(0));

    let add = run_with_editor(
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
    assert_eq!(add.status.code(), Some(0), "{}", String::from_utf8_lossy(&add.stderr));
    let id = String::from_utf8_lossy(&add.stdout).trim().to_owned();
    assert!(!id.is_empty());

    let list = run(&root, ["--project", "list"]);
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

    let show = run(&root, ["--project", "show", &id]);
    assert_eq!(show.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&show.stdout).contains("The body stays readable."));

    let edit = run_with_editor(&root, ["--project", "edit", &id]);
    assert_eq!(edit.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&edit.stdout).trim(), id);

    let archive = run(&root, ["--project", "archive", &id]);
    assert_eq!(archive.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&archive.stdout).trim(),
        format!("{id}\tarchived")
    );
    assert!(run(&root, ["--project", "list"]).stdout.is_empty());
    let all = run(&root, ["--project", "list", "--all"]);
    assert!(String::from_utf8_lossy(&all.stdout).contains(&format!("{id}\tarchived")));

    let restore = run(&root, ["--project", "restore", &id]);
    assert_eq!(restore.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&restore.stdout).trim(), format!("{id}\tactive"));

    let supersede = run_with_editor(&root, ["--project", "supersede", &id]);
    assert_eq!(
        supersede.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&supersede.stderr)
    );
    let replacement = String::from_utf8_lossy(&supersede.stdout).trim().to_owned();
    assert_ne!(replacement, id);
    let active = run(&root, ["--project", "list"]);
    let active_output = String::from_utf8_lossy(&active.stdout);
    assert!(active_output.contains(&replacement));
    assert!(!active_output.contains(&id));

    let blocked_forget = run(&root, ["--project", "forget", &replacement, "--destroy"]);
    assert_eq!(blocked_forget.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&blocked_forget.stderr).contains("--yes"));
    let forgotten = run(&root, ["--project", "forget", &replacement, "--destroy", "--yes"]);
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
        let output = run_with_store_environment(&project, &root, arguments);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let global_add = run_with_store_environment(
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
        &project,
        &root,
        [
            "--project",
            "add",
            "--title",
            "Project\u{202e} collision\u{2028}card",
            "--kind",
            "fact",
            "--body",
            "scope collision from project memory",
        ],
    );
    assert_eq!(project_add.status.code(), Some(0));
    let status = run_with_store_environment(&project, &root, ["--project", "status", "--json"]);
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("parse project status");
    let project_scope = format!("project:{}", status["project_id"].as_str().expect("project id"));

    let search = run_with_store_environment(&project, &root, ["--project", "search", "scope collision", "--json"]);
    assert_eq!(
        search.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&search.stderr)
    );
    let results: serde_json::Value = serde_json::from_slice(&search.stdout).expect("parse search results");
    let results = results.as_array().expect("search result array");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["scope"], project_scope);
    assert_eq!(results[1]["scope"], "global");

    let human_search = run_with_store_environment(&project, &root, ["--project", "search", "scope collision"]);
    assert_eq!(human_search.status.code(), Some(0));
    let human_search = String::from_utf8_lossy(&human_search.stdout);
    assert!(human_search.contains("Project collision card\n  ID: "));
    assert!(human_search.contains(&format!("  Kind: fact  Scope: {project_scope}")));
    assert!(human_search.contains("\n  Score: "));
    assert!(!human_search.contains('\t'));
    assert!(!human_search.contains('\u{202e}'));
    assert!(!human_search.contains('\u{2028}'));

    let local_proposal = run_json_with_store_environment(
        &project,
        &root,
        ["--local", "invoke", "propose"],
        r#"{"version":1,"title":"Local agent scope","kind":"fact","access":"agent","body":"strict local protocol memory","sources":[{"kind":"document","reference":"test.md","actor":"human"}]}"#,
    );
    assert_eq!(local_proposal.status.code(), Some(0));
    let local_proposal: serde_json::Value =
        serde_json::from_slice(&local_proposal.stdout).expect("parse local proposal");
    let local_candidate = local_proposal["result"]["record_id"]
        .as_str()
        .expect("local candidate id");
    let local_approval = run_with_store_environment(&project, &root, ["--local", "approve", local_candidate]);
    assert_eq!(local_approval.status.code(), Some(0));

    fs::write(
        root.join("data/stormbuffer/records/invalid.md"),
        "invalid global record",
    )
    .expect("write invalid global record");
    let local = run_with_store_environment(&project, &root, ["--local", "search", "from project memory", "--json"]);
    assert_eq!(local.status.code(), Some(0));
    let local_results: serde_json::Value = serde_json::from_slice(&local.stdout).expect("parse local results");
    let local_results = local_results.as_array().expect("local result array");
    assert!(!local_results.is_empty());
    assert!(local_results.iter().all(|result| result["scope"] == project_scope));

    let local_invoke = run_json_with_store_environment(
        &project,
        &root,
        ["--local", "invoke", "search"],
        r#"{"version":1,"query":"strict local protocol memory"}"#,
    );
    assert_eq!(local_invoke.status.code(), Some(0));
    let local_envelope: serde_json::Value =
        serde_json::from_slice(&local_invoke.stdout).expect("parse local invoke result");
    assert_eq!(
        local_envelope["result"]
            .as_array()
            .expect("local invoke result array")
            .len(),
        1
    );

    let project_with_invalid_global =
        run_with_store_environment(&project, &root, ["--project", "search", "scope collision", "--json"]);
    assert_eq!(project_with_invalid_global.status.code(), Some(1));

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn retrieval_commands_fail_when_canonical_records_are_invalid() {
    let root = temporary_directory("invalid-retrieval");
    assert!(run(&root, ["--project", "init"]).status.success());
    let added = run_with_editor(
        &root,
        [
            "--project",
            "add",
            "--title",
            "Soon invalid",
            "--kind",
            "fact",
            "--body",
            "canonical content",
        ],
    );
    assert_eq!(
        added.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    let indexed = run(&root, ["--project", "sync"]);
    assert_eq!(indexed.status.code(), Some(0));
    let record_path = fs::read_dir(root.join(".sbuf/records"))
        .expect("read records")
        .map(|entry| entry.expect("record entry").path())
        .find(|path| path.extension() == Some(OsStr::new("md")))
        .expect("canonical record");
    fs::write(record_path, "not frontmatter").expect("corrupt canonical record");

    for arguments in [
        vec!["--project", "sync"],
        vec!["--project", "watch", "--once"],
        vec!["--project", "reindex"],
        vec!["--project", "search", "anything", "--json"],
        vec!["--project", "context", "anything"],
    ] {
        let output = run(&root, arguments);
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("invalid canonical record"));
    }

    let invoked = run_json(
        &root,
        ["--project", "invoke", "search"],
        r#"{"version":1,"query":"anything"}"#,
    );
    assert_eq!(invoked.status.code(), Some(1));
    assert!(invoked.stderr.is_empty());
    let envelope: serde_json::Value = serde_json::from_slice(&invoked.stdout).expect("invalid record envelope");
    assert_eq!(envelope["error"]["code"], "invalid_record");

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn invoke_protocol_covers_operations_and_safe_error_envelopes() {
    let root = temporary_directory("invoke-contract");
    let init = run(&root, ["--project", "init"]);
    assert_eq!(init.status.code(), Some(0));

    let proposal = run_json(
        &root,
        ["--project", "invoke", "propose"],
        r#"{"version":1,"title":"Protocol memory","kind":"fact","access":"agent","body":"A sourced protocol memory.","sources":[{"kind":"document","reference":"ROADMAP.md#agent-writes","actor":"human"}]}"#,
    );
    assert_eq!(
        proposal.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&proposal.stderr)
    );
    assert!(proposal.stderr.is_empty());
    let envelope: serde_json::Value = serde_json::from_slice(&proposal.stdout).expect("proposal envelope");
    assert_eq!(envelope["version"], 1);
    assert_eq!(envelope["operation"], "propose");
    assert_eq!(envelope["result"]["outcome"], "requires_approval");
    let id = envelope["result"]["record_id"].as_str().expect("candidate id");

    let approve = run(&root, ["--project", "approve", id]);
    assert_eq!(
        approve.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&approve.stderr)
    );

    for request in [
        r#"{"version":1,"access":"human","query":"protocol memory"}"#,
        r#"{"version":1,"actor":"human","approved":true,"title":"Impersonated memory","kind":"fact","access":"agent","body":"Must remain a candidate.","sources":[{"kind":"document","reference":"ROADMAP.md","actor":"human"}]}"#,
        r#"{"version":1,"title":"Hidden agent memory","kind":"fact","access":"human","body":"Agents must not create human-only records.","sources":[{"kind":"document","reference":"ROADMAP.md","actor":"human"}]}"#,
    ] {
        let operation = if request.contains("query") { "search" } else { "propose" };
        let denied = run_json(&root, ["--project", "invoke", operation], request);
        assert_eq!(denied.status.code(), Some(1));
        let envelope: serde_json::Value = serde_json::from_slice(&denied.stdout).expect("permission denial envelope");
        assert_eq!(envelope["error"]["code"], "permission_denied");
    }

    let denied_supersede = run_json(
        &root,
        ["--project", "invoke", "supersede"],
        &format!(
            r#"{{"version":1,"id":"{id}","title":"Hidden replacement","kind":"fact","access":"human","body":"Agents must not hide replacements.","sources":[{{"kind":"document","reference":"ROADMAP.md","actor":"human"}}]}}"#
        ),
    );
    assert_eq!(denied_supersede.status.code(), Some(1));
    let denied_supersede: serde_json::Value =
        serde_json::from_slice(&denied_supersede.stdout).expect("supersede denial envelope");
    assert_eq!(denied_supersede["error"]["code"], "permission_denied");

    let get = run_json(
        &root,
        ["--project", "invoke", "get"],
        &format!(r#"{{"version":1,"id":"{id}"}}"#),
    );
    assert_eq!(get.status.code(), Some(0), "{}", String::from_utf8_lossy(&get.stderr));
    let get_envelope: serde_json::Value = serde_json::from_slice(&get.stdout).expect("get envelope");
    assert_eq!(get_envelope["result"]["id"], id);
    assert!(get_envelope["result"].get("path").is_none());

    for (operation, request) in [
        ("search", r#"{"version":1,"query":"protocol memory"}"#),
        ("context", r#"{"version":1,"query":"protocol memory","budget":128}"#),
    ] {
        let output = run_json(&root, ["--project", "invoke", operation], request);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let response: serde_json::Value = serde_json::from_slice(&output.stdout).expect("operation envelope");
        assert_eq!(response["operation"], operation);
        assert_eq!(response["ok"], true);
        if operation == "context" {
            let receipt = &response["result"]["receipt"];
            stormbuffer_core::ReceiptId::parse(receipt["receipt_id"].as_str().expect("receipt id"))
                .expect("valid receipt id");
            stormbuffer_core::Timestamp::parse(receipt["retrieved_at"].as_str().expect("retrieval time"))
                .expect("valid retrieval time");
        }
    }

    let supersede = run_json(
        &root,
        ["--project", "invoke", "supersede"],
        &format!(
            r#"{{"version":1,"id":"{id}","title":"Updated protocol memory","kind":"fact","access":"agent","body":"An updated sourced protocol memory.","sources":[{{"kind":"document","reference":"ROADMAP.md#agent-writes","actor":"human"}}]}}"#
        ),
    );
    assert_eq!(
        supersede.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&supersede.stderr)
    );
    let supersede_envelope: serde_json::Value = serde_json::from_slice(&supersede.stdout).expect("supersede envelope");
    let replacement_id = supersede_envelope["result"]["id"].as_str().expect("replacement id");

    let archive = run_json(
        &root,
        ["--project", "invoke", "archive"],
        &format!(r#"{{"version":1,"id":"{replacement_id}"}}"#),
    );
    assert_eq!(archive.status.code(), Some(0));
    let archive_envelope: serde_json::Value = serde_json::from_slice(&archive.stdout).expect("archive envelope");
    assert_eq!(archive_envelope["result"]["status"], "archived");

    let malformed = run_json(&root, ["--project", "invoke", "search"], "{not-json");
    assert_eq!(malformed.status.code(), Some(1));
    let malformed_envelope: serde_json::Value = serde_json::from_slice(&malformed.stdout).expect("malformed envelope");
    assert_eq!(malformed_envelope["error"]["code"], "invalid_json");

    let denied = run_json(
        &root,
        ["--project", "invoke", "search"],
        r#"{"version":1,"query":"protocol","path":"/private/record.md"}"#,
    );
    assert_eq!(denied.status.code(), Some(1));
    assert!(denied.stderr.is_empty());
    let denied_envelope: serde_json::Value = serde_json::from_slice(&denied.stdout).expect("denial envelope");
    assert_eq!(denied_envelope["ok"], false);
    assert_eq!(denied_envelope["error"]["code"], "path_denied");
    assert!(!String::from_utf8_lossy(&denied.stdout).contains("private/record"));

    let index = root.join(".sbuf/index.sqlite3");
    fs::remove_file(&index).expect("remove fixture index");
    fs::create_dir(&index).expect("block fixture index");
    let internal = run_json(
        &root,
        ["--project", "invoke", "search"],
        r#"{"version":1,"query":"protocol"}"#,
    );
    assert_eq!(internal.status.code(), Some(1));
    assert!(internal.stderr.is_empty());
    let internal_text = String::from_utf8_lossy(&internal.stdout);
    let internal_envelope: serde_json::Value =
        serde_json::from_slice(&internal.stdout).expect("internal error envelope");
    assert_eq!(internal_envelope["error"]["code"], "internal_error");
    assert!(!internal_text.contains(root.to_string_lossy().as_ref()));
    assert!(!internal_text.contains("backtrace"));

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn invoke_remember_and_update_enforce_candidate_review() {
    let root = temporary_directory("invoke-memory-intents");
    assert!(run(&root, ["--project", "init"]).status.success());

    let missing_evidence = run_json(
        &root,
        ["--project", "invoke", "remember"],
        r#"{"version":1,"title":"Unsourced","kind":"fact","body":"No evidence."}"#,
    );
    assert_eq!(missing_evidence.status.code(), Some(0));
    let missing_evidence: serde_json::Value =
        serde_json::from_slice(&missing_evidence.stdout).expect("validation envelope");
    assert_eq!(missing_evidence["result"]["outcome"], "invalid");

    let denied = run_json(
        &root,
        ["--project", "invoke", "remember"],
        r#"{"version":1,"actor":"human","title":"Impersonated","kind":"fact","body":"Must not activate.","source":{"kind":"document","reference":"ROADMAP.md","actor":"human"}}"#,
    );
    assert_eq!(denied.status.code(), Some(1));
    let denied: serde_json::Value = serde_json::from_slice(&denied.stdout).expect("permission envelope");
    assert_eq!(denied["error"]["code"], "permission_denied");

    let remembered = run_json(
        &root,
        ["--project", "invoke", "remember"],
        r#"{"version":1,"title":"Intent memory","kind":"fact","body":"Original sourced memory.","source":{"kind":"document","reference":"ROADMAP.md#agent-capture","actor":"human"}}"#,
    );
    assert_eq!(remembered.status.code(), Some(0));
    let remembered: serde_json::Value = serde_json::from_slice(&remembered.stdout).expect("remember envelope");
    assert_eq!(remembered["operation"], "remember");
    assert_eq!(remembered["result"]["outcome"], "requires_approval");
    let old_id = remembered["result"]["record_id"].as_str().expect("remembered id");
    assert!(run(&root, ["--project", "approve", old_id]).status.success());

    let duplicate = run_json(
        &root,
        ["--project", "invoke", "remember"],
        r#"{"version":1,"title":"Intent memory","kind":"fact","body":"Original sourced memory.","source":{"kind":"document","reference":"ROADMAP.md#agent-capture","actor":"human"}}"#,
    );
    let duplicate: serde_json::Value = serde_json::from_slice(&duplicate.stdout).expect("duplicate envelope");
    assert_eq!(duplicate["result"]["outcome"], "duplicate_of");

    let overlap = run_json(
        &root,
        ["--project", "invoke", "remember"],
        r#"{"version":1,"title":"Intent memory","kind":"fact","body":"A different memory.","source":{"kind":"document","reference":"TODO.md#SB-501","actor":"human"}}"#,
    );
    let overlap: serde_json::Value = serde_json::from_slice(&overlap.stdout).expect("overlap envelope");
    assert_eq!(overlap["result"]["outcome"], "possible_overlap");
    let overlap_id = overlap["result"]["record_id"].as_str().expect("overlap id");
    assert!(run(&root, ["--project", "reject", overlap_id]).status.success());

    let updated = run_json(
        &root,
        ["--project", "invoke", "update"],
        &format!(
            r#"{{"version":1,"id":"{old_id}","body":"Replacement sourced memory.","source":{{"kind":"document","reference":"TODO.md#SB-501","actor":"human"}}}}"#
        ),
    );
    assert_eq!(updated.status.code(), Some(0));
    let updated: serde_json::Value = serde_json::from_slice(&updated.stdout).expect("update envelope");
    assert_eq!(updated["operation"], "update");
    assert_eq!(updated["result"]["outcome"], "requires_approval");
    let replacement_id = updated["result"]["record_id"].as_str().expect("replacement id");

    for (id, expected_status, expected_supersedes) in
        [(old_id, "active", None), (replacement_id, "candidate", Some(old_id))]
    {
        let get = run_json(
            &root,
            ["--project", "invoke", "get"],
            &format!(r#"{{"version":1,"id":"{id}"}}"#),
        );
        let get: serde_json::Value = serde_json::from_slice(&get.stdout).expect("get intent record");
        assert_eq!(get["result"]["status"], expected_status);
        if let Some(superseded_id) = expected_supersedes {
            assert_eq!(get["result"]["supersedes"][0], superseded_id);
        }
    }

    assert!(run(&root, ["--project", "approve", replacement_id]).status.success());
    let old = run_json(
        &root,
        ["--project", "invoke", "get"],
        &format!(r#"{{"version":1,"id":"{old_id}"}}"#),
    );
    let old: serde_json::Value = serde_json::from_slice(&old.stdout).expect("superseded record");
    assert_eq!(old["result"]["status"], "superseded");
}

#[test]
fn invoke_protocol_bounds_the_complete_serialized_response() {
    let root = temporary_directory("invoke-output-bound");
    let init = run(&root, ["--project", "init"]);
    assert_eq!(init.status.code(), Some(0));

    for index in 0..5 {
        let body = format!("bounded{}", "x".repeat(65_536 - "bounded".len()));
        let request = serde_json::json!({
            "version": 1,
            "title": format!("Bounded response {index}"),
            "kind": "fact",
            "access": "agent",
            "body": body,
            "sources": [{
                "kind": "document",
                "reference": "ROADMAP.md#cli-contract",
                "actor": "human"
            }]
        });
        let proposal = run_json(&root, ["--project", "invoke", "propose"], &request.to_string());
        assert_eq!(
            proposal.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&proposal.stderr)
        );
        let envelope: serde_json::Value = serde_json::from_slice(&proposal.stdout).expect("proposal envelope");
        let id = envelope["result"]["record_id"].as_str().expect("candidate id");
        let approve = run(&root, ["--project", "approve", id]);
        assert_eq!(approve.status.code(), Some(0));
    }

    let output = run_json(
        &root,
        ["--project", "invoke", "context"],
        r#"{"version":1,"query":"bounded response","limit":5,"budget":10}"#,
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.len() < 1024);
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).expect("bounded error envelope");
    assert_eq!(envelope["error"]["code"], "output_too_large");

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn sbuf_exposes_version_help_and_commands() {
    let root = temporary_directory("public-name");
    let version = run(&root, ["--version"]);
    assert_eq!(version.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&version.stdout).contains("0.1.0"));

    let help = run(&root, ["--help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&help.stdout).contains("Usage: sbuf"),
        "{}",
        String::from_utf8_lossy(&help.stdout)
    );

    let status = run(&root, ["--project", "status"]);
    assert_eq!(status.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&status.stdout).contains("State: not initialized"));
    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn skill_install_is_offline_idempotent_and_requires_force_for_conflicts() {
    let root = temporary_directory("skill-install");
    let skills = root.join(".agents").join("skills");
    let skills_argument = skills.to_string_lossy().into_owned();
    let destination = skills.join("stormbuffer-global-memory").join("SKILL.md");

    let first = run(&root, ["--global", "skill", "install", "--directory", &skills_argument]);
    assert_eq!(
        first.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(String::from_utf8_lossy(&first.stdout).contains(&destination.display().to_string()));
    let installed = fs::read(&destination).expect("read installed skill");
    let installed_text = String::from_utf8_lossy(&installed);
    assert!(installed_text.contains("name: stormbuffer-global-memory"));
    assert!(installed_text.contains("sbuf --global invoke search"));
    assert!(installed_text.contains("stormbuffer-mcp --stdio --global"));
    assert!(installed_text.contains("Global retrieval stays within the global store"));
    assert!(!installed_text.contains("--project"));

    let second = run(&root, ["skill", "install", "--directory", &skills_argument]);
    assert_eq!(second.status.code(), Some(0));
    assert_eq!(fs::read(&destination).expect("read reinstalled skill"), installed);

    fs::write(&destination, "locally customized\n").expect("write conflicting skill");
    let conflict = run(&root, ["skill", "install", "--directory", &skills_argument]);
    assert_eq!(conflict.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("--force"));
    assert_eq!(
        fs::read_to_string(&destination).expect("read conflict"),
        "locally customized\n"
    );

    let replacement = run(&root, ["skill", "install", "--directory", &skills_argument, "--force"]);
    assert_eq!(replacement.status.code(), Some(0));
    assert_eq!(fs::read(&destination).expect("read replacement"), installed);

    let project_skills = root.join("project-agent-skills");
    let project_argument = project_skills.to_string_lossy().into_owned();
    let project = run(
        &root,
        ["--project", "skill", "install", "--directory", &project_argument],
    );
    assert_eq!(project.status.code(), Some(0));
    let project_skill =
        fs::read_to_string(project_skills.join("stormbuffer-memory").join("SKILL.md")).expect("read project skill");
    assert!(project_skill.contains("name: stormbuffer-memory"));
    assert!(project_skill.contains("sbuf --project invoke search"));
    assert!(project_skill.contains("Project retrieval can also return global records"));
    assert!(project_skill.contains("possible_overlap"));
    assert!(!project_skill.contains("conflicts_with"));
    assert!(!project_skill.contains("--global"));

    let help = run(&root, ["skill", "install", "--help"]);
    assert_eq!(help.status.code(), Some(0));
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("selected scope's memory skill"));
    assert!(help.contains("--directory <DIRECTORY>"));
    assert!(help.contains("--force"));
    assert!(help.contains("--global"));

    fs::remove_dir_all(root).expect("remove test directory");
}

#[test]
fn color_modes_no_color_and_json_output_follow_the_contract() {
    let root = temporary_directory("color");
    let init = run(&root, ["--project", "init"]);
    assert_eq!(init.status.code(), Some(0));
    assert!(!init.stdout.contains(&0x1b));

    let auto = run(&root, ["--project", "--color", "auto", "status"]);
    assert_eq!(auto.status.code(), Some(0));
    assert!(!auto.stdout.contains(&0x1b));

    let never = run(&root, ["--project", "--color", "never", "status"]);
    assert_eq!(never.status.code(), Some(0));
    assert!(!never.stdout.contains(&0x1b));

    let always = run(&root, ["--project", "--color", "always", "status"]);
    assert_eq!(always.status.code(), Some(0));
    let always_stdout = String::from_utf8_lossy(&always.stdout);
    assert!(
        always_stdout.contains("\u{1b}[1m\u{1b}[36mScope\u{1b}[39m\u{1b}[0m: project"),
        "{always_stdout}"
    );
    assert!(
        always_stdout.contains("\u{1b}[1m\u{1b}[32minitialized\u{1b}[39m\u{1b}[0m"),
        "{always_stdout}"
    );
    assert!(
        always_stdout.contains("\u{1b}[4m\u{1b}[96m"),
        "root path should have its own underlined color: {always_stdout}"
    );

    let mut no_color_command = Command::new(binary());
    no_color_command
        .current_dir(&root)
        .args(["--project", "--color", "auto", "status"])
        .env("NO_COLOR", "1");
    let no_color = no_color_command.output().expect("run NO_COLOR status");
    assert_eq!(no_color.status.code(), Some(0));
    assert!(!no_color.stdout.contains(&0x1b));

    let mut forced_no_color_command = Command::new(binary());
    forced_no_color_command
        .current_dir(&root)
        .args(["--project", "--color", "always", "status"])
        .env("NO_COLOR", "1");
    let forced_no_color = forced_no_color_command.output().expect("run forced NO_COLOR status");
    assert_eq!(forced_no_color.status.code(), Some(0));
    assert!(!forced_no_color.stdout.contains(&0x1b));

    let colored_error = run(&root, ["--project", "--color", "always", "mcp"]);
    assert_eq!(colored_error.status.code(), Some(1));
    assert!(colored_error.stderr.contains(&0x1b));

    let mut no_color_error_command = Command::new(binary());
    no_color_error_command
        .current_dir(&root)
        .args(["--project", "--color", "always", "mcp"])
        .env("NO_COLOR", "1");
    let no_color_error = no_color_error_command.output().expect("run forced NO_COLOR error");
    assert_eq!(no_color_error.status.code(), Some(1));
    assert!(!no_color_error.stderr.contains(&0x1b));

    let json = run(&root, ["--project", "--color", "always", "status", "--json"]);
    assert_eq!(json.status.code(), Some(0));
    assert!(!json.stdout.contains(&0x1b));
    assert!(String::from_utf8_lossy(&json.stdout).starts_with('{'));

    fs::remove_dir_all(root).expect("remove test directory");
}
