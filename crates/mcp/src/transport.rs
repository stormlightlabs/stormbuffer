use rmcp::ServiceExt;
use std::error::Error;
use std::sync::Arc;
use stormbuffer_core as core;

use crate::{config::McpConfig, server::McpServer};

pub fn run_stdio() -> Result<(), Box<dyn Error + Send + Sync>> {
    run_stdio_with_config(McpConfig::default())
}

pub fn run_stdio_with_config(config: McpConfig) -> Result<(), Box<dyn Error + Send + Sync>> {
    let cwd = std::env::current_dir()?;
    let paths = core::resolve_store(config.scope, &cwd)?;
    let use_default_embedder =
        !cfg!(debug_assertions) || std::env::var_os("STORMBUFFER_TEST_MODE").is_none();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let server =
            if cfg!(debug_assertions) && std::env::var_os("STORMBUFFER_TEST_EMBEDDER").is_some() {
                McpServer::with_embedder(
                    paths,
                    config.allow_writes,
                    Arc::new(core::DeterministicEmbedder::new("mcp-stdio-test-v1", 24)?),
                )
            } else if use_default_embedder {
                McpServer::with_default_embedder(paths, config.allow_writes)
            } else {
                McpServer::new(paths, config.allow_writes)
            };
        let running = server.serve(rmcp::transport::stdio()).await?;
        running.waiting().await?;
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    })
}
