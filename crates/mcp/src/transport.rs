use std::error::Error;

use rmcp::ServiceExt;
use stormbuffer_core as core;

use crate::{config::McpConfig, server::McpServer};

pub fn run_stdio() -> Result<(), Box<dyn Error + Send + Sync>> {
    run_stdio_with_config(McpConfig::default())
}

pub fn run_stdio_with_config(config: McpConfig) -> Result<(), Box<dyn Error + Send + Sync>> {
    let cwd = std::env::current_dir()?;
    let paths = core::resolve_store(config.scope, &cwd)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let running = McpServer::new(paths, config.allow_writes)
            .serve(rmcp::transport::stdio())
            .await?;
        running.waiting().await?;
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    })
}
