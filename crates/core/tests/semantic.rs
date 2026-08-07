use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use stormbuffer_core::{
    Access, ContextOptions, DeterministicEmbedder, Embedder, Embedding, Error, RetrievalMode,
    SearchOptions, StoreInitMode, StorePaths, StoreScope, context_stores_with_embedder, index_path,
    initialize_store, rebuild_vector_index, reindex_store_with_embedder,
    search_stores_with_embedder, sync_store,
};

struct TempStore {
    root: PathBuf,
}

static NEXT_TEMP_STORE: AtomicU64 = AtomicU64::new(0);

impl TempStore {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        for attempt in 0..100 {
            let counter = NEXT_TEMP_STORE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "stormbuffer-semantic-{}-{stamp}-{counter}-{attempt}",
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

fn write_record(paths: &StorePaths, id: &str, scope: &str, kind: &str, status: &str, body: &str) {
    let markdown = format!(
        r#"+++
format_version = 1
id = "{id}"
title = "{kind} memory"
kind = "{kind}"
scope = "{scope}"
status = "{status}"
access = "human"
created_at = "2026-08-05T20:09:00Z"
updated_at = "2026-08-05T20:09:00Z"
tags = ["semantic"]
aliases = []
supersedes = []

[[sources]]
kind = "document"
reference = "semantic-test"
actor = "test"
+++

{body}
"#
    );
    fs::write(paths.records.join(format!("{id}.md")), markdown).expect("write record");
}

#[test]
fn vector_backfill_records_metadata_and_applies_scope_kind_and_active_filters() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    write_record(
        &paths,
        "01989af2-4305-7b19-88b1-e8ae4ea9a201",
        "project:alpha",
        "fact",
        "active",
        "alpha deployment procedure",
    );
    write_record(
        &paths,
        "01989af2-4305-7b19-88b1-e8ae4ea9a202",
        "project:beta",
        "fact",
        "active",
        "beta deployment procedure",
    );
    write_record(
        &paths,
        "01989af2-4305-7b19-88b1-e8ae4ea9a203",
        "project:alpha",
        "procedure",
        "archived",
        "archived deployment procedure",
    );
    sync_store(&paths).expect("sync");
    let embedder = DeterministicEmbedder::new("semantic-v1", 24).expect("embedder");
    let first_metadata = rebuild_vector_index(&paths, &embedder).expect("vector backfill");
    let first_table = first_metadata.table_name.clone();
    let second_metadata = rebuild_vector_index(&paths, &embedder).expect("reuse vector index");
    assert_eq!(first_metadata.index_id, second_metadata.index_id);

    let mut options = SearchOptions::for_store(&paths);
    options.mode = RetrievalMode::Semantic;
    options.allowed_scopes = Some(vec!["project:alpha".to_owned()]);
    options.allowed_kinds = Some(vec!["fact".to_owned()]);
    let results = search_stores_with_embedder(
        &[paths.clone()],
        "deployment procedure",
        options.clone(),
        &embedder,
    )
    .expect("semantic search");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].scope, "project:alpha");
    assert_eq!(results[0].status, "active");
    assert_eq!(results[0].kind, "fact");
    assert!(
        results[0]
            .match_reasons
            .iter()
            .any(|reason| reason.starts_with("vector:"))
    );
    let mut context_search = options.clone();
    context_search.allowed_scopes =
        Some(vec!["project:alpha".to_owned(), "project:beta".to_owned()]);
    context_search.allowed_kinds = None;
    context_search.limit = 1;
    let context = context_stores_with_embedder(
        &[paths.clone()],
        "deployment procedure",
        ContextOptions {
            budget: 100,
            search: context_search,
        },
        &embedder,
    )
    .expect("limited semantic context");
    assert_eq!(context.blocks.len(), 1);
    options.allowed_access = Some(vec![Access::Agent]);
    assert!(
        search_stores_with_embedder(&[paths.clone()], "deployment procedure", options, &embedder,)
            .expect("inaccessible search")
            .is_empty()
    );

    let connection = rusqlite::Connection::open(index_path(&paths)).expect("open index");
    let metadata: (String, i64) = connection
        .query_row(
            "SELECT model_version, dimension FROM vector_indexes WHERE active = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("vector metadata");
    assert_eq!(metadata, ("semantic-v1".to_owned(), 24));
    let vector_indexes: i64 = connection
        .query_row("SELECT count(*) FROM vector_indexes", [], |row| row.get(0))
        .expect("vector index count");
    assert_eq!(vector_indexes, 1);

    let replacement = DeterministicEmbedder::new("semantic-v2", 24).expect("replacement");
    rebuild_vector_index(&paths, &replacement).expect("replace vector index");
    let old_table_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [&first_table],
            |row| row.get(0),
        )
        .expect("old vector table count");
    assert_eq!(old_table_count, 0);
    let vector_indexes: i64 = connection
        .query_row("SELECT count(*) FROM vector_indexes", [], |row| row.get(0))
        .expect("vector index count after replacement");
    assert_eq!(vector_indexes, 1);
}

#[test]
fn reindex_reports_and_rebuilds_semantic_state_when_model_is_supplied() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    write_record(
        &paths,
        "01989af2-4305-7b19-88b1-e8ae4ea9a206",
        "global",
        "fact",
        "active",
        "reindex semantic state",
    );
    let embedder = DeterministicEmbedder::new("semantic-v1", 24).expect("embedder");
    let report = reindex_store_with_embedder(&paths, Some(&embedder)).expect("reindex");
    let semantic = report.semantic.expect("semantic report");
    assert_eq!(semantic.status, "rebuilt");
    assert_eq!(semantic.model_version.as_deref(), Some("semantic-v1"));
}

#[test]
fn semantic_search_rejects_vectors_when_canonical_content_changes() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    write_record(
        &paths,
        "01989af2-4305-7b19-88b1-e8ae4ea9a205",
        "global",
        "fact",
        "active",
        "fresh canonical content",
    );
    sync_store(&paths).expect("sync");
    let embedder = DeterministicEmbedder::new("semantic-v1", 24).expect("embedder");
    rebuild_vector_index(&paths, &embedder).expect("vector backfill");
    let path = paths
        .records
        .join("01989af2-4305-7b19-88b1-e8ae4ea9a205.md");
    let changed = fs::read_to_string(&path)
        .expect("read record")
        .replace("fresh canonical content", "changed canonical content");
    fs::write(path, changed).expect("change canonical record");

    let mut options = SearchOptions::for_store(&paths);
    options.mode = RetrievalMode::Semantic;
    let error = search_stores_with_embedder(&[paths], "fresh canonical", options, &embedder)
        .expect_err("stale vectors must not escape");
    assert!(error.to_string().contains("semantic index is stale"));
}

#[test]
fn rebuilding_vectors_synchronizes_changed_canonical_content() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    write_record(
        &paths,
        "01989af2-4305-7b19-88b1-e8ae4ea9a205",
        "global",
        "fact",
        "active",
        "original canonical content",
    );
    sync_store(&paths).expect("sync");
    let embedder = DeterministicEmbedder::new("semantic-v1", 24).expect("embedder");
    rebuild_vector_index(&paths, &embedder).expect("vector backfill");
    let path = paths
        .records
        .join("01989af2-4305-7b19-88b1-e8ae4ea9a205.md");
    let changed = fs::read_to_string(&path)
        .expect("read record")
        .replace("original canonical content", "changed canonical content");
    fs::write(path, changed).expect("change canonical record");

    rebuild_vector_index(&paths, &embedder).expect("rebuild changed canonical content");

    let mut options = SearchOptions::for_store(&paths);
    options.mode = RetrievalMode::Semantic;
    let results =
        search_stores_with_embedder(&[paths], "changed canonical content", options, &embedder)
            .expect("search rebuilt vectors");
    assert_eq!(results.len(), 1);
}

#[test]
fn rebuilding_vectors_rejects_invalid_canonical_records() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    let id = "01989af2-4305-7b19-88b1-e8ae4ea9a207";
    write_record(
        &paths,
        id,
        "global",
        "fact",
        "active",
        "valid canonical content",
    );
    let embedder = DeterministicEmbedder::new("semantic-v1", 24).expect("embedder");
    rebuild_vector_index(&paths, &embedder).expect("initial vector backfill");
    fs::write(paths.records.join(format!("{id}.md")), "invalid record").expect("corrupt record");

    let error = rebuild_vector_index(&paths, &embedder)
        .expect_err("invalid canonical records must prevent a vector rebuild");
    assert!(matches!(error, Error::InvalidRecord { .. }));
}

struct FilteredEmbedder;
impl Embedder for FilteredEmbedder {
    fn model_version(&self) -> &str {
        "filtered-v1"
    }

    fn model_checksum(&self) -> &str {
        "filtered-checksum"
    }

    fn dimension(&self) -> usize {
        1
    }

    fn embed(&self, text: &str) -> stormbuffer_core::Result<Embedding> {
        let value = if text.contains("rare-target") {
            2.0
        } else {
            0.0
        };
        Embedding::new(vec![value])
    }
}

#[test]
fn filtered_vector_search_adapts_beyond_the_initial_candidate_window() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    for index in 0..=1000 {
        let id = format!("01989af2-4305-7b19-88b1-e8ae4ea9{index:04x}");
        write_record(&paths, &id, "global", "fact", "active", "common candidate");
    }
    let target_id = "01989af2-4305-7b19-88b1-e8ae4ea9ffff";
    write_record(
        &paths,
        target_id,
        "global",
        "procedure",
        "active",
        "rare-target",
    );
    sync_store(&paths).expect("sync");
    let embedder = FilteredEmbedder;
    rebuild_vector_index(&paths, &embedder).expect("vector backfill");

    let mut options = SearchOptions::for_store(&paths);
    options.mode = RetrievalMode::Semantic;
    options.limit = 1;
    options.allowed_kinds = Some(vec!["procedure".to_owned()]);
    let results = search_stores_with_embedder(&[paths], "common query", options, &embedder)
        .expect("filtered semantic search");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].record_id, target_id);
}

struct FailingEmbedder;
impl Embedder for FailingEmbedder {
    fn model_version(&self) -> &str {
        "semantic-v2"
    }
    fn model_checksum(&self) -> &str {
        "different"
    }
    fn dimension(&self) -> usize {
        24
    }
    fn embed(&self, _text: &str) -> stormbuffer_core::Result<Embedding> {
        Err(Error::InvalidInput {
            message: "fixture failure".to_owned(),
        })
    }
}

#[test]
fn failed_model_backfill_keeps_the_previous_active_index_available() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize");
    write_record(
        &paths,
        "01989af2-4305-7b19-88b1-e8ae4ea9a204",
        "global",
        "fact",
        "active",
        "recoverable semantic index",
    );
    sync_store(&paths).expect("sync");
    let embedder = DeterministicEmbedder::new("semantic-v1", 24).expect("embedder");
    rebuild_vector_index(&paths, &embedder).expect("first vector backfill");
    assert!(rebuild_vector_index(&paths, &FailingEmbedder).is_err());

    let mut options = SearchOptions::for_store(&paths);
    options.mode = RetrievalMode::Semantic;
    let results = search_stores_with_embedder(&[paths], "recoverable semantic", options, &embedder)
        .expect("old vector search");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "fact memory");
}
