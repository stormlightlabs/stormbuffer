use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use stormbuffer_core::{PlatformDirs, StoreInitMode, StoreScope};

fn temporary_project() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("stormbuffer-mcp-{suffix}"));
    fs::create_dir_all(&root).expect("create temporary project");
    let paths = stormbuffer_core::resolve_store_with_dirs(
        StoreScope::Project,
        &root,
        &PlatformDirs::new(root.join("data"), root.join("cache")),
    )
    .expect("resolve temporary store");
    stormbuffer_core::initialize_store(&paths, StoreInitMode::Default)
        .expect("initialize temporary store");
    root
}

fn request(id: u64, method: &str, params: Value) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    }))
    .unwrap()
}

#[test]
fn stdio_lists_surface_and_closes_cleanly_at_eof() {
    let project = temporary_project();
    let mut child = Command::new(env!("CARGO_BIN_EXE_stormbuffer-mcp"))
        .arg("--stdio")
        .arg("--project")
        .current_dir(project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn stormbuffer-mcp");
    let input = [
        request(
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "stormbuffer-test", "version": "0.1.0"}
            }),
        ),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string(),
        request(2, "tools/list", json!({})),
        request(3, "resources/templates/list", json!({})),
        json!({
            "jsonrpc":"2.0",
            "method":"notifications/cancelled",
            "params":{"requestId":999}
        })
        .to_string(),
    ]
    .join("\n");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(format!("{input}\n").as_bytes())
        .expect("write MCP requests");

    let output = child.wait_with_output().expect("wait for MCP shutdown");
    assert!(
        output.status.success(),
        "MCP stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .expect("MCP stdout is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("MCP response is JSON"))
        .collect();
    assert_eq!(responses.len(), 3);
    assert_eq!(
        responses[0]["result"]["serverInfo"]["name"],
        "stormbuffer-mcp"
    );
    assert_eq!(responses[1]["result"]["tools"].as_array().unwrap().len(), 6);
    assert_eq!(
        responses[2]["result"]["resourceTemplates"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}
