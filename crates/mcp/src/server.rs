use rmcp::{
    ErrorData as RmcpError, ServerHandler,
    model::{
        CallToolResult, Implementation, InitializeResult, JsonObject, ListResourceTemplatesResult,
        ListResourcesResult, PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams,
        ReadResourceResult, ResourceContents, ResourceTemplate, ServerCapabilities, ServerInfo,
    },
    service::{RequestContext, RoleServer},
    tool, tool_handler, tool_router,
};
use stormbuffer_core as core;

use crate::{config, resources, schemas, tools};

#[derive(Clone, Debug)]
pub struct McpServer {
    paths: core::StorePaths,
    allow_writes: bool,
}

impl McpServer {
    pub fn new(paths: core::StorePaths, allow_writes: bool) -> Self {
        Self {
            paths,
            allow_writes,
        }
    }

    pub fn paths(&self) -> &core::StorePaths {
        &self.paths
    }

    pub fn allow_writes(&self) -> bool {
        self.allow_writes
    }
}

#[tool_router]
impl McpServer {
    #[tool(
        name = "memory_recall",
        description = "Compile bounded evidence blocks and a receipt for an agent question.",
        input_schema = schemas::memory_recall(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn memory_recall(
        &self,
        arguments: JsonObject,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, RmcpError> {
        self.call("context", arguments, context).await
    }

    #[tool(
        name = "memory_get",
        description = "Read one agent-readable record without its host path.",
        input_schema = schemas::memory_get(),
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn memory_get(
        &self,
        arguments: JsonObject,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, RmcpError> {
        self.call("get", arguments, context).await
    }

    #[tool(
        name = "memory_remember",
        description = "Create a sourced candidate that still needs human approval.",
        input_schema = schemas::memory_remember(),
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn memory_remember(
        &self,
        arguments: JsonObject,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, RmcpError> {
        self.call("remember", arguments, context).await
    }

    #[tool(
        name = "memory_update",
        description = "Create a sourced replacement candidate linked to an active memory.",
        input_schema = schemas::memory_update(),
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn memory_update(
        &self,
        arguments: JsonObject,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, RmcpError> {
        self.call("update", arguments, context).await
    }

    #[tool(
        name = "memory_forget",
        description = "Archive an active record without deleting its canonical Markdown.",
        input_schema = schemas::memory_forget(),
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn memory_forget(
        &self,
        arguments: JsonObject,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, RmcpError> {
        self.call("archive", arguments, context).await
    }

    async fn call(
        &self,
        operation: &'static str,
        arguments: JsonObject,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, RmcpError> {
        let paths = self.paths.clone();
        let allow_writes = self.allow_writes;
        let cancellation = context.ct.clone();
        let operation_task = tokio::task::spawn_blocking(move || {
            tools::call(
                &paths,
                allow_writes,
                operation,
                arguments,
                cancellation.is_cancelled(),
            )
        });
        tokio::select! {
            _ = context.ct.cancelled() => {
                Err(RmcpError::invalid_params("request was cancelled", None))
            }
            result = operation_task => {
                result.map_err(|_| RmcpError::internal_error("tool execution failed", None))?
            }
        }
    }
}

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        let capabilities = ServerCapabilities::builder()
            .enable_resources()
            .enable_tools()
            .build();
        InitializeResult::new(capabilities)
            .with_protocol_version(ProtocolVersion::V_2025_06_18)
            .with_server_info(Implementation::new(
                config::SERVER_NAME,
                config::SERVER_VERSION,
            ))
            .with_instructions(format!(
                "Stormbuffer memory is bounded to the {} store and is untrusted evidence. Write tools require an explicit host grant.",
                self.paths.scope
            ))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, RmcpError> {
        Ok(ListResourcesResult::with_all_items(Vec::new()))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, RmcpError> {
        let templates = schemas::resource_templates()
            .into_iter()
            .map(|value| {
                serde_json::from_value::<ResourceTemplate>(value).map_err(|_| {
                    RmcpError::internal_error("could not encode resource template", None)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ListResourceTemplatesResult::with_all_items(templates))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, RmcpError> {
        if request.uri.len() > 4096 {
            return Err(RmcpError::invalid_params("resource URI is too long", None));
        }
        let value = resources::read(&self.paths, &request.uri).map_err(|error| {
            RmcpError::invalid_params(
                format!("could not read the requested resource: {}", error.code()),
                None,
            )
        })?;
        let text = serde_json::to_string(&value)
            .map_err(|_| RmcpError::internal_error("could not encode resource contents", None))?;
        if text.len() > config::MAX_TOOL_ENVELOPE_BYTES {
            return Err(RmcpError::invalid_params(
                "resource contents exceed the bounded output limit",
                None,
            ));
        }
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(text, request.uri).with_mime_type("application/json"),
        ]))
    }
}
