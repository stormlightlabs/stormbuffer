mod config;
mod resources;
mod schemas;
mod server;
mod tools;
mod transport;

pub use config::{McpConfig, McpWritePolicy};
pub use server::McpServer;
pub use transport::{run_stdio, run_stdio_with_config};

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rmcp::model::JsonObject;
    use stormbuffer_core as core;

    use super::*;

    fn paths() -> core::StorePaths {
        core::resolve_store_with_dirs(
            core::StoreScope::Global,
            &std::env::current_dir().unwrap(),
            &core::PlatformDirs::new(
                PathBuf::from("target/test-mcp-data"),
                PathBuf::from("target/test-mcp-cache"),
            ),
        )
        .unwrap()
    }

    #[test]
    fn schemas_and_resources_are_stable() {
        let tools = vec![
            McpServer::memory_recall_tool_attr(),
            McpServer::memory_get_tool_attr(),
            McpServer::memory_remember_tool_attr(),
            McpServer::memory_update_tool_attr(),
            McpServer::memory_forget_tool_attr(),
        ];
        assert_eq!(tools.len(), 5);
        assert_eq!(schemas::resource_templates().len(), 3);

        let annotations = |name: &str| {
            let annotations = tools
                .iter()
                .find(|tool| tool.name == name)
                .unwrap()
                .annotations
                .as_ref()
                .unwrap();
            (
                annotations.read_only_hint.unwrap(),
                annotations.destructive_hint.unwrap(),
                annotations.idempotent_hint.unwrap(),
                annotations.open_world_hint.unwrap(),
            )
        };
        for name in ["memory_recall", "memory_get"] {
            assert_eq!(annotations(name), (true, false, false, false));
        }
        for name in ["memory_remember", "memory_update"] {
            assert_eq!(annotations(name), (false, false, false, false));
        }
        assert_eq!(annotations("memory_forget"), (false, true, false, false));
    }

    #[test]
    fn writes_are_denied_and_cancellation_is_observed() {
        assert!(!McpWritePolicy::ReadOnly.allows("remember"));
        assert!(McpWritePolicy::CandidateOnly.allows("remember"));
        assert!(McpWritePolicy::CandidateOnly.allows("update"));
        assert!(!McpWritePolicy::CandidateOnly.allows("archive"));
        assert!(McpWritePolicy::All.allows("archive"));

        let result = tools::call(
            &paths(),
            McpWritePolicy::ReadOnly,
            "archive",
            JsonObject::new(),
            false,
            None,
            None,
        )
        .unwrap();
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content.as_ref().unwrap()["error"]["code"],
            "permission_denied"
        );

        let error = tools::call(
            &paths(),
            McpWritePolicy::All,
            "context",
            JsonObject::new(),
            true,
            None,
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
    }

    #[test]
    fn malformed_invocation_is_rejected_by_core() {
        let error = core::invoke_request(&paths(), "search", b"not-json").unwrap_err();
        assert_eq!(error.code(), "invalid_json");
    }

    #[test]
    fn agent_write_secret_errors_are_sanitized_at_the_mcp_boundary() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stormbuffer-mcp-secret-{suffix}"));
        let paths = core::StorePaths {
            scope: core::StoreScope::Global,
            records: root.join("records"),
            cache: root.join("cache"),
            root: root.clone(),
        };
        core::initialize_store(&paths, core::StoreInitMode::Default).expect("initialize store");
        let secret = "ghp_0123456789abcdefghijklmnop";
        let arguments = serde_json::from_value(serde_json::json!({
            "title": "Unsafe MCP candidate",
            "kind": "fact",
            "body": format!("credential: {secret}"),
            "source": {"kind": "conversation", "reference": "mcp-test", "actor": "agent"}
        }))
        .expect("tool arguments");

        let result = McpServer::new(paths.clone(), McpWritePolicy::CandidateOnly)
            .call_sync("remember", arguments, false)
            .expect("structured rejection");
        let structured = result.structured_content.expect("structured result");
        assert_eq!(structured["error"]["code"], "secret_detected");
        assert!(!structured.to_string().contains(secret));
        assert_eq!(
            fs::read_dir(&paths.records)
                .expect("read records")
                .filter_map(Result::ok)
                .count(),
            0
        );

        fs::remove_dir_all(root).expect("remove temporary store");
    }

    #[test]
    fn recall_uses_the_supplied_embedder_for_hybrid_retrieval() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stormbuffer-mcp-hybrid-{suffix}"));
        let paths = core::StorePaths {
            scope: core::StoreScope::Global,
            records: root.join("records"),
            cache: root.join("cache"),
            root: root.clone(),
        };
        core::initialize_store(&paths, core::StoreInitMode::Default).expect("initialize store");
        let remembered = core::invoke_request(
            &paths,
            "remember",
            br#"{"version":1,"title":"Hybrid MCP memory","kind":"fact","body":"A pulsar powers the MCP fixture.","source":{"kind":"document","reference":"mcp-test","actor":"test"}}"#,
        )
        .expect("remember fixture");
        let id = remembered["record_id"]
            .as_str()
            .expect("record ID")
            .parse()
            .expect("valid record ID");
        core::RecordRepository::new(paths.clone())
            .approve(id)
            .expect("approve fixture");
        let embedder = core::DeterministicEmbedder::new("mcp-semantic-v1", 24)
            .expect("deterministic embedder");
        let arguments =
            serde_json::from_value(serde_json::json!({"query":"pulsar MCP","budget":128}))
                .expect("tool arguments");

        let server =
            McpServer::with_embedder(paths.clone(), McpWritePolicy::ReadOnly, Arc::new(embedder));
        let result = server
            .call_sync("context", arguments, false)
            .expect("recall result");
        let receipt = &result
            .structured_content
            .as_ref()
            .expect("structured result")["result"]["receipt"];
        assert_eq!(receipt["retrieval_mode"], "hybrid");
        assert_eq!(receipt["embedding_model"], "stormbuffer/deterministic");
        assert_eq!(receipt["embedding_version"], "mcp-semantic-v1");
        assert!(receipt["semantic_fallback"].is_null());
        let blocks = result
            .structured_content
            .as_ref()
            .expect("structured result")["result"]["blocks"]
            .as_array()
            .expect("context blocks");
        assert_eq!(blocks[0]["record_id"], id.to_string());
        let reasons = blocks[0]["ranking_reasons"]
            .as_array()
            .expect("ranking evidence");
        assert!(reasons.iter().any(|reason| {
            reason
                .as_str()
                .is_some_and(|reason| reason.starts_with("lexical:"))
        }));
        assert!(reasons.iter().any(|reason| {
            reason
                .as_str()
                .is_some_and(|reason| reason.starts_with("vector:"))
        }));

        fs::remove_dir_all(root).expect("remove temporary store");
    }
}
