use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use stormbuffer_core::{
    AdvisoryRelationProjection, INDEX_SCHEMA_VERSION, SearchOptions, StoreInitMode, StorePaths, StoreScope,
    context_store, doctor_store, index_path, initialize_store, inspect_store, invoke_request, reindex_store,
    repair_store, replace_advisory_relation_projection, search_store, search_stores, sync_store,
};

struct TempStore {
    root: PathBuf,
}

static NEXT_TEMP_STORE: AtomicU64 = AtomicU64::new(0);

impl TempStore {
    fn new() -> Self {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
        for attempt in 0..100 {
            let counter = NEXT_TEMP_STORE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "stormbuffer-index-{}-{suffix}-{counter}-{attempt}",
                std::process::id(),
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Self { root },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create temporary store root: {error}"),
            }
        }
        panic!("could not find a unique temporary store root")
    }

    fn paths(&self) -> StorePaths {
        StorePaths {
            scope: StoreScope::Global,
            root: self.root.clone(),
            records: self.root.join("records"),
            cache: self.root.join("cache"),
        }
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_record(
    paths: &StorePaths, filename: &str, id: &str, title: &str, scope: &str, alias: &str, body: &str,
) -> PathBuf {
    let markdown = format!(
        r#"+++
format_version = 1
id = "{id}"
title = "{title}"
kind = "fact"
scope = "{scope}"
status = "active"
access = "human"
created_at = "2026-08-05T20:09:00-05:00"
updated_at = "2026-08-05T20:09:00-05:00"
tags = ["testing"]
aliases = ["{alias}"]
supersedes = []

[[sources]]
kind = "document"
reference = "test.md"
actor = "tester"
+++

{body}
"#
    );
    let path = paths.records.join(filename);
    fs::write(&path, markdown).expect("write canonical record");
    path
}

#[test]
fn sync_is_incremental_and_search_returns_attributable_results() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid/fact.md");
    let destination = paths.records.join("memory.md");
    fs::copy(fixture, &destination).expect("copy canonical record");

    let first = sync_store(&paths).expect("first sync");
    assert_eq!(first.indexed, 1);
    let second = sync_store(&paths).expect("incremental sync");
    assert_eq!(second.indexed, 0);
    assert_eq!(second.skipped, 1);

    let results = search_store(&paths, "portable source", SearchOptions::default()).expect("search");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].lexical_match_reason, "phrase");
    assert_eq!(results[0].sources[0].reference, "ROADMAP.md#canonical-records");

    let context = context_store(
        &paths,
        "portable",
        stormbuffer_core::ContextOptions { budget: 3, search: SearchOptions::default() },
    )
    .expect("context");
    assert!(context.receipt.used_tokens <= 3);
    assert_eq!(context.receipt.index_version, INDEX_SCHEMA_VERSION);
}

#[test]
fn doctor_explains_how_to_enable_semantic_retrieval() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    sync_store(&paths).expect("sync");

    let report = doctor_store(&paths).expect("diagnose index");
    assert!(!report.semantic_model_ready);
    let semantic = report
        .issues
        .iter()
        .find(|issue| issue.message.contains("semantic retrieval"))
        .expect("semantic readiness warning");

    assert_eq!(semantic.severity, "warning");
    assert!(semantic.message.contains("lexical matching"));
    assert_eq!(
        semantic.repair,
        "run `sbuf init` while online to download and verify the local model"
    );
}

#[test]
fn sync_removes_deleted_records_and_reindex_switches_projection() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid/fact.md");
    let destination = paths.records.join("memory.md");
    fs::copy(source, &destination).expect("copy canonical record");
    let report = sync_store(&paths).expect("sync");
    assert_eq!(report.indexed, 1, "invalid: {:?}", report.invalid_files);
    fs::remove_file(destination).expect("delete canonical record");
    let report = sync_store(&paths).expect("remove stale projection");
    assert_eq!(report.removed, 1);
    assert!(
        search_store(&paths, "portable", SearchOptions::default())
            .expect("search empty projection")
            .is_empty()
    );
    let report = reindex_store(&paths).expect("rebuild projection");
    assert!(index_path(&paths).is_file());
    assert_eq!(
        report.semantic.as_ref().map(|value| value.status.as_str()),
        Some("unavailable")
    );
}

#[test]
fn sync_rejects_invalid_store_metadata_and_foreign_record_scopes() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    write_record(
        &paths,
        "foreign.md",
        "01989af2-4305-7b19-88b1-e8ae4ea9a08b",
        "Foreign scope",
        "project:01989af2-4305-7b19-88b1-e8ae4ea9a099",
        "scope boundary",
        "This record must not enter the global projection.",
    );
    let report = sync_store(&paths).expect("sync reports foreign scope");
    assert_eq!(report.invalid_files.len(), 1);
    assert!(
        search_store(&paths, "scope boundary", SearchOptions::default())
            .expect("search valid store")
            .is_empty()
    );

    fs::write(paths.root.join("store.toml"), "not valid metadata").expect("corrupt metadata");
    assert!(sync_store(&paths).is_err());
    assert!(search_store(&paths, "scope boundary", SearchOptions::default()).is_err());
}

#[test]
fn search_matches_filename_commands_aliases_and_unicode() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    write_record(
        &paths,
        "deploy-runbook.md",
        "01989af2-4305-7b19-88b1-e8ae4ea9a03b",
        "Serve crème brûlée",
        "global",
        "production ritual",
        "Run `cargo test --workspace` before serving crème brûlée.",
    );
    let report = sync_store(&paths).expect("sync");
    assert_eq!(report.indexed, 1, "invalid: {:?}", report.invalid_files);

    for query in ["production ritual", "crème brûlée", "cargo test --workspace"] {
        assert_eq!(
            search_store(&paths, query, SearchOptions::default())
                .expect("search")
                .len(),
            1,
            "query {query:?}"
        );
    }
    let filename = search_store(&paths, "deploy-runbook.md", SearchOptions::default()).expect("filename search");
    assert_eq!(filename[0].lexical_match_reason, "exact_filename");
}

#[test]
fn project_search_includes_global_and_rebuild_preserves_manual_edits() {
    let store = TempStore::new();
    let global = store.paths();
    let project_root = store.root.join("demo-project/.sbuf");
    let project = StorePaths {
        scope: StoreScope::Project,
        records: project_root.join("records"),
        cache: store.root.join("project-cache"),
        root: project_root,
    };
    initialize_store(&global, StoreInitMode::Default).expect("initialize global");
    initialize_store(&project, StoreInitMode::Default).expect("initialize project");
    let project_scope = stormbuffer_core::record_scope(&project)
        .expect("project scope")
        .as_str()
        .to_owned();
    write_record(
        &global,
        "global.md",
        "01989af2-4305-7b19-88b1-e8ae4ea9a04b",
        "Global recovery",
        "global",
        "shared recovery",
        "A recoverable index keeps canonical memory safe.",
    );
    let mut project_file = write_record(
        &project,
        "project.md",
        "01989af2-4305-7b19-88b1-e8ae4ea9a05b",
        "Project recovery",
        &project_scope,
        "local recovery",
        "A recoverable index starts with the project record.",
    );
    let global_report = sync_store(&global).expect("sync global");
    assert_eq!(global_report.indexed, 1, "invalid: {:?}", global_report.invalid_files);
    let project_report = sync_store(&project).expect("sync project");
    assert_eq!(project_report.indexed, 1, "invalid: {:?}", project_report.invalid_files);

    let options = SearchOptions::for_store(&project);
    let results = search_stores(&[project.clone(), global.clone()], "recoverable index", options.clone())
        .expect("search both scopes");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].scope, project_scope);
    assert_eq!(results[1].scope, "global");

    let moved = project.records.join("moved-project.md");
    fs::rename(&project_file, &moved).expect("move canonical record");
    project_file = moved;
    let move_report = sync_store(&project).expect("sync moved record");
    assert_eq!(move_report.indexed, 1);
    assert_eq!(move_report.invalid_files.len(), 0);

    let edited = fs::read_to_string(&project_file)
        .expect("read project record")
        .replace("recoverable index", "manually edited index");
    fs::write(&project_file, edited).expect("edit canonical record");
    let report = sync_store(&project).expect("sync manual edit");
    assert_eq!(report.indexed, 1);
    assert_eq!(
        search_store(&project, "manually edited", options.clone())
            .expect("search manual edit")
            .len(),
        1
    );

    let canonical_before = fs::read(&project_file).expect("read canonical bytes");
    let before = search_store(&project, "manually edited", options.clone()).expect("search before rebuild");
    reindex_store(&project).expect("rebuild projection");
    let after = search_store(&project, "manually edited", options).expect("search after rebuild");
    assert_eq!(
        before.iter().map(|result| &result.title).collect::<Vec<_>>(),
        after.iter().map(|result| &result.title).collect::<Vec<_>>()
    );
    assert_eq!(
        fs::read(&project_file).expect("read canonical after rebuild"),
        canonical_before
    );

    let records_away = project.root.join("records-away");
    fs::rename(&project.records, &records_away).expect("make canonical scan fail");
    assert!(reindex_store(&project).is_err());
    fs::rename(&records_away, &project.records).expect("restore canonical records");
    assert_eq!(
        search_store(&project, "manually edited", SearchOptions::for_store(&project))
            .expect("existing index remains usable")
            .len(),
        1
    );

    fs::write(index_path(&project), "corrupt projection").expect("corrupt disposable index");
    let diagnosis = doctor_store(&project).expect("diagnose index");
    assert!(diagnosis.failures > 0);
    assert!(diagnosis.warnings > 0);
    assert!(diagnosis.issues.iter().all(|issue| !issue.repair.is_empty()));
    reindex_store(&project).expect("replace corrupt projection");
    assert_eq!(
        search_store(&project, "manually edited", SearchOptions::for_store(&project))
            .expect("search repaired index")
            .len(),
        1
    );
    assert_eq!(
        fs::read(&project_file).expect("read canonical after repair"),
        canonical_before
    );

    let invalid = project.records.join("invalid.md");
    fs::write(&invalid, "not valid frontmatter").expect("write invalid record");
    let invalid_bytes = fs::read(&invalid).expect("read invalid bytes");
    let report = sync_store(&project).expect("report invalid record");
    assert_eq!(report.invalid_files.len(), 1);
    assert_eq!(fs::read(invalid).expect("read invalid after sync"), invalid_bytes);
}

#[test]
fn renamed_project_keeps_its_scope_when_reindexed_locally() {
    let store = TempStore::new();
    let original_directory = store.root.join("before");
    let original_root = original_directory.join(".sbuf");
    let original = StorePaths {
        scope: StoreScope::Project,
        records: original_root.join("records"),
        cache: original_root.join("cache"),
        root: original_root,
    };
    initialize_store(&original, StoreInitMode::Default).expect("initialize project");
    let project_scope = stormbuffer_core::record_scope(&original)
        .expect("project scope")
        .as_str()
        .to_owned();
    write_record(
        &original,
        "identity.md",
        "01989af2-4305-7b19-88b1-e8ae4ea9a06b",
        "Stable project identity",
        &project_scope,
        "renamed project",
        "Renaming a project does not change its memory scope.",
    );
    sync_store(&original).expect("sync original project");

    let renamed_directory = store.root.join("after");
    fs::rename(&original_directory, &renamed_directory).expect("rename project directory");
    let renamed_root = renamed_directory.join(".sbuf");
    let renamed = StorePaths {
        scope: StoreScope::Local,
        records: renamed_root.join("records"),
        cache: renamed_root.join("cache"),
        root: renamed_root,
    };

    assert_eq!(
        stormbuffer_core::record_scope(&renamed)
            .expect("renamed project scope")
            .as_str(),
        project_scope
    );
    reindex_store(&renamed).expect("reindex renamed project");
    let results =
        search_store(&renamed, "renamed project", SearchOptions::for_store(&renamed)).expect("search renamed project");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, project_scope);
}

#[test]
fn invoke_search_reports_an_unopenable_sqlite_projection() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    fs::create_dir(index_path(&paths)).expect("block projection path");

    let error = invoke_request(&paths, "search", br#"{"version":1,"query":"projection"}"#)
        .expect_err("search should fail when the projection cannot be opened");

    assert_eq!(error.code(), "internal_error");
    assert_eq!(
        error.message(),
        "the SQLite projection could not be opened; check that its directory is writable, then reindex the selected store"
    );
    let root = paths.root.to_string_lossy();
    assert!(!error.message().contains(root.as_ref()));
    assert!(!error.message().contains("unable to open database file"));
}

#[cfg(unix)]
#[test]
fn global_store_uses_a_temporary_projection_when_its_cache_is_read_only() {
    use std::os::unix::fs::PermissionsExt;

    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    write_record(
        &paths,
        "fallback.md",
        "019fdb80-0000-7000-8000-000000000001",
        "Fallback projection",
        "global",
        "writable fallback",
        "Agents can rebuild a disposable projection.",
    );

    let mut permissions = fs::metadata(&paths.cache).expect("cache metadata").permissions();
    permissions.set_mode(0o555);
    fs::set_permissions(&paths.cache, permissions).expect("make cache read-only");

    let report = sync_store(&paths).expect("sync through fallback projection");
    assert_eq!(report.indexed, 1);
    let doctor = doctor_store(&paths).expect("inspect fallback projection");
    let fallback = PathBuf::from(&doctor.index_path);
    assert!(fallback.starts_with(std::env::temp_dir()));
    assert_ne!(fallback, paths.cache.join("global.sqlite3"));
    assert!(fallback.is_file());
    let status = inspect_store(&paths).expect("inspect fallback status");
    assert_eq!(status.index_version, Some(INDEX_SCHEMA_VERSION));
    assert!(status.last_successful_sync.is_some());
    assert!(status.disposable_bytes >= fs::metadata(&fallback).expect("fallback metadata").len());
    assert_eq!(
        search_store(&paths, "disposable", SearchOptions::for_store(&paths))
            .expect("search fallback projection")
            .len(),
        1
    );

    let mut permissions = fs::metadata(&paths.cache).expect("cache metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&paths.cache, permissions).expect("restore cache permissions");
    fs::remove_dir_all(fallback.parent().expect("fallback directory")).expect("remove fallback projection");
}

#[test]
fn status_reports_lifecycle_disk_and_projection_details() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    for (index, status) in ["candidate", "active", "superseded", "archived"]
        .into_iter()
        .enumerate()
    {
        let path = write_record(
            &paths,
            &format!("{status}.md"),
            &format!("019fdb80-0000-7000-8000-00000000001{index}"),
            &format!("{status} record"),
            "global",
            status,
            "Operational status fixture.",
        );
        if status != "active" {
            let markdown = fs::read_to_string(&path).expect("read fixture");
            fs::write(
                &path,
                markdown.replace("status = \"active\"", &format!("status = \"{status}\"")),
            )
            .expect("set lifecycle status");
        }
    }
    sync_store(&paths).expect("sync");

    let status = inspect_store(&paths).expect("inspect status");
    assert_eq!(status.record_count, 4);
    assert_eq!(status.lifecycle.candidate, 1);
    assert_eq!(status.lifecycle.active, 1);
    assert_eq!(status.lifecycle.superseded, 1);
    assert_eq!(status.lifecycle.archived, 1);
    assert!(status.canonical_bytes > 0);
    assert!(status.disposable_bytes > 0);
    assert_eq!(status.index_version, Some(INDEX_SCHEMA_VERSION));
    assert!(status.last_successful_sync.is_some());
}

#[test]
fn project_status_excludes_the_shared_global_cache() {
    let store = TempStore::new();
    let mut paths = store.paths();
    paths.scope = StoreScope::Project;
    paths.root = store.root.join("project/.sbuf");
    paths.records = paths.root.join("records");
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    sync_store(&paths).expect("sync");

    let before = inspect_store(&paths).expect("inspect project status").disposable_bytes;
    fs::create_dir_all(&paths.cache).expect("create shared cache");
    fs::write(paths.cache.join("global-projection.bin"), vec![0; 4096]).expect("write shared cache fixture");
    let after = inspect_store(&paths)
        .expect("inspect project status with shared cache")
        .disposable_bytes;

    assert_eq!(after, before);
}

#[test]
fn repair_rebuilds_only_disposable_state_and_is_idempotent() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    let record = write_record(
        &paths,
        "repair.md",
        "019fdb80-0000-7000-8000-000000000020",
        "Repair boundary",
        "global",
        "repair boundary",
        "Canonical Markdown must survive projection repair unchanged.",
    );
    sync_store(&paths).expect("sync");
    let marker = paths.root.join("store.toml");
    let marker_before = fs::read(&marker).expect("read marker");
    let record_before = fs::read(&record).expect("read canonical record");
    fs::write(index_path(&paths), b"not a SQLite projection").expect("corrupt projection");
    fs::create_dir_all(paths.root.join("locks")).expect("create locks");
    fs::create_dir_all(paths.root.join("tmp")).expect("create tmp");
    fs::write(paths.root.join("locks/stale.lock"), b"stale").expect("write stale lock");
    fs::write(paths.root.join("locks/supersede.toml"), b"recovery journal").expect("write recovery journal");
    fs::write(paths.root.join("tmp/stale.json"), b"stale").expect("write stale temp");

    let repaired = repair_store(&paths).expect("repair store");
    assert_eq!(repaired.diagnosis.failures, 0);
    assert_eq!(repaired.repaired.len(), 3);
    assert_eq!(fs::read(&marker).expect("read marker after"), marker_before);
    assert_eq!(fs::read(&record).expect("read record after"), record_before);
    assert!(!paths.root.join("locks/stale.lock").exists());
    assert_eq!(
        fs::read(paths.root.join("locks/supersede.toml")).expect("read recovery journal"),
        b"recovery journal"
    );
    assert!(!paths.root.join("tmp/stale.json").exists());
    assert_eq!(
        search_store(&paths, "survive", SearchOptions::for_store(&paths))
            .expect("search repaired projection")
            .len(),
        1
    );

    let repeated = repair_store(&paths).expect("repeat repair");
    assert!(repeated.repaired.is_empty());
    assert_eq!(fs::read(marker).expect("read repeated marker"), marker_before);
    assert_eq!(fs::read(record).expect("read repeated record"), record_before);
}

#[cfg(unix)]
#[test]
fn repair_does_not_follow_symlinks_in_disposable_directories() {
    use std::os::unix::fs::symlink;

    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    sync_store(&paths).expect("sync");
    let outside = store.root.join("outside.txt");
    fs::write(&outside, b"outside the repair boundary").expect("write outside fixture");
    fs::create_dir_all(paths.root.join("tmp")).expect("create tmp");
    symlink(&outside, paths.root.join("tmp/outside-link")).expect("create symlink");

    let repair = repair_store(&paths).expect("repair store");

    assert!(repair.repaired.is_empty());
    assert_eq!(
        fs::read(outside).expect("read outside fixture"),
        b"outside the repair boundary"
    );
}

#[test]
fn advisory_relations_are_discarded_when_the_projection_is_rebuilt() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    sync_store(&paths).expect("sync");
    replace_advisory_relation_projection(
        &paths,
        &[AdvisoryRelationProjection {
            left_record_id: "left".to_owned(),
            right_record_id: "right".to_owned(),
            relation: "unknown".to_owned(),
            evidence_json: "[]".to_owned(),
            confidence: "low".to_owned(),
            analyzer_fingerprint: "test-analyzer".to_owned(),
        }],
    )
    .expect("write advisory projection");
    let before: i64 = rusqlite::Connection::open(index_path(&paths))
        .expect("open projection")
        .query_row("SELECT COUNT(*) FROM advisory_relations", [], |row| row.get(0))
        .expect("count advisory rows");
    assert_eq!(before, 1);

    reindex_store(&paths).expect("rebuild projection");
    let after: i64 = rusqlite::Connection::open(index_path(&paths))
        .expect("open rebuilt projection")
        .query_row("SELECT COUNT(*) FROM advisory_relations", [], |row| row.get(0))
        .expect("count rebuilt advisory rows");
    assert_eq!(after, 0);
}

#[test]
fn repair_reports_canonical_failures_for_manual_intervention() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    fs::write(paths.records.join("invalid.md"), b"not a canonical record").expect("write invalid canonical record");
    let invalid_before = fs::read(paths.records.join("invalid.md")).expect("read invalid record");

    let repair = repair_store(&paths).expect("diagnose canonical failure");
    assert!(repair.repaired.is_empty());
    assert!(repair.diagnosis.failures > 0);
    assert!(
        repair
            .diagnosis
            .issues
            .iter()
            .any(|issue| { issue.severity == "failure" && issue.repair.contains("repair the Markdown") })
    );
    assert_eq!(
        fs::read(paths.records.join("invalid.md")).expect("read invalid record after repair"),
        invalid_before
    );
    assert!(!index_path(&paths).exists());
}

#[test]
fn repair_refuses_duplicate_and_out_of_scope_canonical_records() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    let first = write_record(
        &paths,
        "first.md",
        "019fdb80-0000-7000-8000-000000000030",
        "First",
        "global",
        "first",
        "First canonical record.",
    );
    fs::copy(&first, paths.records.join("duplicate.md")).expect("copy duplicate record");
    write_record(
        &paths,
        "wrong-scope.md",
        "019fdb80-0000-7000-8000-000000000031",
        "Wrong scope",
        "project:019fdb80-0000-7000-8000-000000000032",
        "wrong scope",
        "This record belongs to another store.",
    );

    let repair = repair_store(&paths).expect("diagnose canonical invariants");

    assert!(repair.repaired.is_empty());
    assert!(repair.diagnosis.failures >= 2);
    assert!(
        repair
            .diagnosis
            .issues
            .iter()
            .any(|issue| issue.message.contains("duplicates the ID"))
    );
    assert!(
        repair
            .diagnosis
            .issues
            .iter()
            .any(|issue| issue.message.contains("outside the selected store scope"))
    );
    assert!(!index_path(&paths).exists());
}
