use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use stormbuffer_core::{
    RecordRepository, StoreInitMode, StorePaths, StoreScope, initialize_store, invoke_request,
    parse_markdown,
};

struct TempStore {
    root: PathBuf,
}

impl TempStore {
    fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stormbuffer-secret-guard-test-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create temporary root");
        Self { root }
    }

    fn paths(&self) -> StorePaths {
        StorePaths {
            scope: StoreScope::Global,
            records: self.root.join("records"),
            cache: self.root.join("cache"),
            root: self.root.clone(),
        }
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn request(paths: &StorePaths, operation: &str, value: serde_json::Value) -> serde_json::Value {
    let bytes = serde_json::to_vec(&value).expect("encode request");
    invoke_request(paths, operation, &bytes).expect("request succeeds")
}

fn record_count(paths: &StorePaths) -> usize {
    fs::read_dir(&paths.records)
        .expect("read records")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "md")
        })
        .count()
}

#[test]
fn agent_remember_and_update_reject_secrets_without_writing_them() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize store");
    let secret = "ghp_0123456789abcdefghijklmnop";
    let remember = json!({
        "version": 1,
        "title": "Unsafe candidate",
        "kind": "fact",
        "body": format!("credential: {secret}"),
        "source": {"kind": "conversation", "reference": "test", "actor": "agent"}
    });

    let error = invoke_request(
        &paths,
        "remember",
        &serde_json::to_vec(&remember).expect("encode remember"),
    )
    .expect_err("secret is rejected");
    assert_eq!(error.code(), "secret_detected");
    assert!(!error.message().contains(secret));
    assert_eq!(record_count(&paths), 0);

    let remembered = request(
        &paths,
        "remember",
        json!({
            "version": 1,
            "title": "Safe candidate",
            "kind": "fact",
            "body": "ordinary body",
            "source": {"kind": "conversation", "reference": "test", "actor": "agent"}
        }),
    );
    let id = remembered["record_id"].as_str().expect("record ID");
    let before = record_count(&paths);
    let update = json!({
        "version": 1,
        "id": id,
        "body": format!("Authorization: Bearer {secret}"),
        "source": {"kind": "conversation", "reference": "test", "actor": "agent"}
    });
    let error = invoke_request(
        &paths,
        "update",
        &serde_json::to_vec(&update).expect("encode update"),
    )
    .expect_err("secret update is rejected");
    assert_eq!(error.code(), "secret_detected");
    assert!(!error.message().contains(secret));
    assert_eq!(record_count(&paths), before);
}

#[test]
fn direct_human_repository_writes_remain_outside_the_agent_guard() {
    let store = TempStore::new();
    let paths = store.paths();
    initialize_store(&paths, StoreInitMode::Default).expect("initialize store");
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/valid/fact.md");
    let markdown = fs::read_to_string(&fixture_path).expect("read fixture");
    let mut record = parse_markdown(&fixture_path, &markdown).expect("parse fixture");
    record.body = "Authorization: Bearer human-maintained-example".to_owned();

    RecordRepository::new(paths)
        .add(record)
        .expect("direct repository write remains available");
}
