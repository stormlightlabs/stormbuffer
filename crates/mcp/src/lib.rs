mod config;
mod resources;
mod schemas;
mod server;
mod tools;
mod transport;

pub use config::McpConfig;
pub use server::McpServer;
pub use transport::{run_stdio, run_stdio_with_config};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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
        let result = tools::call(&paths(), false, "archive", JsonObject::new(), false).unwrap();
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content.as_ref().unwrap()["error"]["code"],
            "permission_denied"
        );

        let error = tools::call(&paths(), true, "context", JsonObject::new(), true).unwrap_err();
        assert!(error.to_string().contains("cancelled"));
    }

    #[test]
    fn malformed_invocation_is_rejected_by_core() {
        let error = core::invoke_request(&paths(), "search", b"not-json").unwrap_err();
        assert_eq!(error.code(), "invalid_json");
    }
}
