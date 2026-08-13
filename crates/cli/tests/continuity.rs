use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_project() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let root = env::temp_dir().join(format!("stormbuffer-continuity-{suffix}"));
    fs::create_dir_all(&root).expect("create temporary project");
    root
}

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sbuf"))
}

fn run(root: &Path, arguments: &[&str], input: Option<&str>) -> Output {
    let mut command = Command::new(binary());
    command
        .current_dir(root)
        .args(arguments)
        .env("HOME", root.join("home"))
        .env("USERPROFILE", root.join("home"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("STORMBUFFER_TEST_MODE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("run sbuf");
    if let Some(input) = input {
        child
            .stdin
            .take()
            .expect("open stdin")
            .write_all(input.as_bytes())
            .expect("write request");
    }
    child.wait_with_output().expect("collect sbuf output")
}

fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn project_checkpoint_supports_a_later_session() {
    let root = temporary_project();
    success(run(&root, &["--project", "init"], None));

    const BODY: &str = "## Completed work\nThe manifest parser accepts the settled field names.\n\n\
## Unresolved state\nValidation for duplicate manifest entries is unresolved.\n\n\
## Settled decisions\nKeep duplicate detection in the shared parser.\n\n\
## Next action\nAdd the duplicate-entry case at the parser boundary.\n\n\
## References\nROADMAP.md#project-scoped-continuity";
    let request = serde_json::json!({
        "version": 1,
        "title": "Manifest parser handoff",
        "kind": "checkpoint",
        "body": BODY,
        "source": {
            "kind": "document",
            "reference": "ROADMAP.md#project-scoped-continuity",
            "actor": "human"
        }
    });
    let proposal = success(run(
        &root,
        &["--project", "invoke", "remember"],
        Some(&request.to_string()),
    ));
    let proposal: serde_json::Value = serde_json::from_slice(&proposal).expect("parse proposal");
    assert_eq!(proposal["result"]["outcome"], "requires_approval");
    let record_id = proposal["result"]["record_id"]
        .as_str()
        .expect("checkpoint ID");
    success(run(&root, &["--project", "approve", record_id], None));

    let recall_request = serde_json::json!({
        "version": 1,
        "query": "manifest parser unresolved validation",
        "budget": 512
    });
    let recalled = success(run(
        &root,
        &["--project", "invoke", "context"],
        Some(&recall_request.to_string()),
    ));
    let recalled: serde_json::Value =
        serde_json::from_slice(&recalled).expect("parse recalled context");
    let blocks = recalled["result"]["blocks"]
        .as_array()
        .expect("context blocks");
    let checkpoint_blocks: Vec<_> = blocks
        .iter()
        .filter(|block| block["record_id"] == record_id)
        .collect();
    assert!(!checkpoint_blocks.is_empty());
    assert!(checkpoint_blocks.iter().all(|block| {
        block["kind"] == "checkpoint"
            && block["scope"]
                .as_str()
                .is_some_and(|scope| scope.starts_with("project:"))
            && block["sources"] == serde_json::json!([request["source"].clone()])
    }));
    let text = checkpoint_blocks
        .iter()
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("Validation for duplicate manifest entries is unresolved."));
    assert!(text.contains("Add the duplicate-entry case at the parser boundary."));

    let get_request = serde_json::json!({"version": 1, "id": record_id});
    let resumed = success(run(
        &root,
        &["--project", "invoke", "get"],
        Some(&get_request.to_string()),
    ));
    let resumed: serde_json::Value =
        serde_json::from_slice(&resumed).expect("parse resumed checkpoint");
    assert_eq!(resumed["result"]["body"], BODY);
    assert_eq!(resumed["result"]["sources"][0], request["source"]);
}
