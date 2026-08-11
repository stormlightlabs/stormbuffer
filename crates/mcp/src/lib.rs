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

    use rmcp::model::CallToolRequestParams;
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
        assert_eq!(schemas::tools().len(), 5);
        assert_eq!(schemas::resource_templates().len(), 3);
        assert_eq!(
            [
                ("memory_recall", "context"),
                ("memory_get", "get"),
                ("memory_remember", "remember"),
                ("memory_update", "update"),
                ("memory_forget", "archive"),
            ]
            .map(|(tool, _)| (tool, schemas::operation(tool))),
            [
                ("memory_recall", Some("context")),
                ("memory_get", Some("get")),
                ("memory_remember", Some("remember")),
                ("memory_update", Some("update")),
                ("memory_forget", Some("archive")),
            ]
        );
        assert_eq!(schemas::operation("unknown"), None);
    }

    #[test]
    fn writes_are_denied_and_cancellation_is_observed() {
        let request = CallToolRequestParams::new("memory_forget");
        let result = tools::call(&paths(), false, request, false).unwrap();
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content.as_ref().unwrap()["error"]["code"],
            "permission_denied"
        );

        let request = CallToolRequestParams::new("memory_recall");
        let error = tools::call(&paths(), true, request, true).unwrap_err();
        assert!(error.to_string().contains("cancelled"));
    }

    #[test]
    fn malformed_invocation_is_rejected_by_core() {
        let error = core::invoke_request(&paths(), "search", b"not-json").unwrap_err();
        assert_eq!(error.code(), "invalid_json");
    }
}
