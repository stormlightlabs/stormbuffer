use std::fs;
use std::io::{BufRead, BufReader, Write};
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

fn exchange(stdin: &mut impl Write, stdout: &mut impl BufRead, message: &str) -> Value {
    writeln!(stdin, "{message}").expect("write MCP request");
    stdin.flush().expect("flush MCP request");
    let mut response = String::new();
    stdout.read_line(&mut response).expect("read MCP response");
    serde_json::from_str(&response).expect("MCP response is JSON")
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
    let mut tool_names = responses[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    tool_names.sort_unstable();
    assert_eq!(
        tool_names,
        [
            "memory_forget",
            "memory_get",
            "memory_recall",
            "memory_remember",
            "memory_update",
        ]
    );
    assert_eq!(
        responses[2]["result"]["resourceTemplates"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn stdio_exercises_each_documented_memory_tool() {
    let project = temporary_project();
    let paths = stormbuffer_core::resolve_store_with_dirs(
        StoreScope::Project,
        &project,
        &PlatformDirs::new(project.join("data"), project.join("cache")),
    )
    .expect("resolve project store");
    let remembered = stormbuffer_core::invoke_request(
        &paths,
        "remember",
        br#"{"version":1,"title":"MCP fixture","kind":"fact","body":"A recalled MCP fixture.","source":{"kind":"document","reference":"docs/reference/mcp","actor":"human"}}"#,
    )
    .expect("remember fixture");
    let id = remembered["record_id"].as_str().expect("fixture id");
    stormbuffer_core::RecordRepository::new(paths)
        .approve(id.parse().expect("parse fixture id"))
        .expect("approve fixture");

    let mut child = Command::new(env!("CARGO_BIN_EXE_stormbuffer-mcp"))
        .arg("--stdio")
        .arg("--project")
        .arg("--allow-writes")
        .current_dir(project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn writable stormbuffer-mcp");
    let source = json!({
        "kind": "document",
        "reference": "docs/reference/mcp",
        "actor": "human"
    });
    let mut stdin = child.stdin.take().expect("child stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("child stdout"));
    let initialize = exchange(
        &mut stdin,
        &mut stdout,
        &request(
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "stormbuffer-test", "version": "0.1.0"}
            }),
        ),
    );
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .expect("write initialized notification");
    stdin.flush().expect("flush initialized notification");

    let calls = [
        request(
            2,
            "tools/call",
            json!({"name":"memory_recall","arguments":{"query":"MCP fixture","budget":128}}),
        ),
        request(
            3,
            "tools/call",
            json!({"name":"memory_get","arguments":{"id":id}}),
        ),
        request(
            4,
            "tools/call",
            json!({"name":"memory_remember","arguments":{"title":"New MCP memory","kind":"fact","body":"A sourced candidate.","source":source.clone()}}),
        ),
        request(
            5,
            "tools/call",
            json!({"name":"memory_update","arguments":{"id":id,"body":"An updated MCP fixture.","source":source}}),
        ),
        request(
            6,
            "tools/call",
            json!({"name":"memory_forget","arguments":{"id":id}}),
        ),
    ];
    let responses = calls
        .iter()
        .map(|call| exchange(&mut stdin, &mut stdout, call))
        .collect::<Vec<_>>();
    drop(stdin);
    drop(stdout);

    let output = child.wait_with_output().expect("wait for MCP shutdown");
    assert!(
        output.status.success(),
        "MCP stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "stormbuffer-mcp"
    );
    assert_eq!(responses.len(), 5);
    for (response, operation) in responses
        .iter()
        .zip(["context", "get", "remember", "update", "archive"])
    {
        let content = &response["result"]["structuredContent"];
        assert_eq!(content["operation"], operation);
        assert_eq!(content["ok"], true, "{content}");
    }
}
