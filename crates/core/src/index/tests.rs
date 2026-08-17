use super::chunking::{retrieval_text, split_embedding_text};
use super::*;
use crate::{
    Access, Embedder, Embedding, Record, RecordId, RecordKind, RecordStatus, Scope, Source, SourceKind, Timestamp,
};

struct TokenAwareEmbedder;

impl Embedder for TokenAwareEmbedder {
    fn model_id(&self) -> &str {
        "test/token-aware"
    }

    fn model_version(&self) -> &str {
        "token-aware-v1"
    }

    fn model_checksum(&self) -> &str {
        "token-aware-checksum"
    }

    fn dimension(&self) -> usize {
        1
    }

    fn max_tokens(&self) -> usize {
        256
    }

    fn token_count(&self, text: &str) -> crate::Result<usize> {
        let mut tokens = 2;
        let mut run = 0_usize;
        for character in text.chars() {
            if character.is_alphanumeric() || character == '_' {
                run += 1;
            } else {
                tokens += run.div_ceil(4);
                run = 0;
                if !character.is_whitespace() {
                    tokens += 1;
                }
            }
        }
        Ok(tokens + run.div_ceil(4))
    }

    fn embed(&self, _text: &str) -> crate::Result<Embedding> {
        Embedding::new(vec![1.0])
    }
}

fn record(body: &str) -> Record {
    let now = Timestamp::now_utc();
    Record {
        id: RecordId::new_v7(),
        title: "Chunk test".to_owned(),
        kind: RecordKind::Fact,
        scope: Scope::parse("global").expect("scope"),
        status: RecordStatus::Active,
        access: Access::Human,
        created_at: now,
        updated_at: now,
        tags: vec!["test".to_owned()],
        aliases: vec!["chunking".to_owned()],
        supersedes: Vec::new(),
        sources: vec![Source {
            kind: SourceKind::Document,
            reference: "test.md".to_owned(),
            actor: "tester".to_owned(),
            observed_at: None,
            revision: None,
            content_hash: None,
        }],
        body: body.to_owned(),
    }
}

#[test]
fn chunks_keep_fenced_code_and_lists_together() {
    let chunks = chunk_record(&record(
        "# Heading\n\n- one\n- two\n\n```rust\nlet x = 1;\nlet y = 2;\n```",
    ));
    assert_eq!(chunks.len(), 2);
    assert!(chunks[0].2.contains("- one\n- two"));
    assert!(chunks[1].2.starts_with("```rust"));
    assert_eq!(chunks[0].1.as_deref(), Some("Heading"));
}

#[test]
fn embedding_chunks_bound_tokenizer_heavy_structural_markdown() {
    let identifier = format!("longIdentifierWithoutWhitespace{}", "Abcd".repeat(180));
    let punctuation = "!@#$%^&*()[]{};:,.<>?/\\|+=-_".repeat(8);
    let body = format!(
        "# Payload\n\n```json\n{{\"{identifier}\":\"/Users/example/projects/stormbuffer/deep/path/config.json\",\"command\":\"git rev-parse HEAD && cargo test --workspace --all-features\",\"punctuation\":\"{punctuation}\"}}\n```"
    );
    let record = record(&body);
    let embedder = TokenAwareEmbedder;
    let lexical = chunk_record(&record);
    assert_eq!(lexical.len(), 1);
    assert!(lexical[0].3 < embedder.max_tokens());
    let original_input = retrieval_text(
        &record,
        lexical[0].1.as_deref().unwrap_or_default(),
        "record.md",
        &lexical[0].2,
    );
    assert!(embedder.token_count(&original_input).expect("count tokens") > embedder.max_tokens());

    let chunks = split_embedding_text(&original_input, &embedder).expect("split tokenizer-heavy record");
    assert!(chunks.len() > 1);
    for input in chunks {
        assert!(
            embedder.token_count(&input).expect("count emitted input") <= embedder.max_tokens(),
            "oversized embedding input: {input}"
        );
    }
}

#[test]
fn migration_from_version_one_creates_fts() {
    let path = std::env::temp_dir().join(format!("stormbuffer-migration-{}.sqlite3", std::process::id()));
    let _ = fs::remove_file(&path);
    let connection = Connection::open(&path).expect("open database");
    connection.execute_batch(
            "CREATE TABLE scopes (scope_id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
             CREATE TABLE records (record_id TEXT PRIMARY KEY, scope_id INTEGER NOT NULL REFERENCES scopes(scope_id), path TEXT NOT NULL UNIQUE, title TEXT NOT NULL, kind TEXT NOT NULL, status TEXT NOT NULL, access TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, aliases_json TEXT NOT NULL, tags_json TEXT NOT NULL, content_hash TEXT NOT NULL);
             CREATE TABLE chunks (record_id TEXT NOT NULL REFERENCES records(record_id) ON DELETE CASCADE, chunk_id TEXT NOT NULL UNIQUE, ordinal INTEGER NOT NULL, heading TEXT, text TEXT NOT NULL, retrieval_text TEXT NOT NULL, token_count INTEGER NOT NULL, PRIMARY KEY(record_id, ordinal));
             CREATE TABLE sources (source_id INTEGER PRIMARY KEY, record_id TEXT NOT NULL REFERENCES records(record_id) ON DELETE CASCADE, kind TEXT NOT NULL, reference TEXT NOT NULL, actor TEXT NOT NULL);
             CREATE TABLE index_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             PRAGMA user_version = 1;",
        ).expect("create version one schema");
    drop(connection);
    let index = Index::open_at(&path).expect("migrate version one");
    let version: u32 = index
        .connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("read migrated version");
    assert_eq!(version, INDEX_SCHEMA_VERSION);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(format!("{}-wal", path.display()));
    let _ = fs::remove_file(format!("{}-shm", path.display()));
}
