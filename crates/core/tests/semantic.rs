use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use stormbuffer_core::{
    Access, DeterministicEmbedder, Embedder, Embedding, Error, RetrievalMode, SearchOptions,
    StoreInitMode, StorePaths, StoreScope, index_path, initialize_store, rebuild_vector_index,
    search_stores_with_embedder, sync_store,
};

struct TempStore {
    root: PathBuf,
}
impl TempStore {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stormbuffer-semantic-{stamp}"));
        fs::create_dir_all(&root).expect("create temp store");
        Self { root }
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
    rebuild_vector_index(&paths, &embedder).expect("vector backfill");

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
