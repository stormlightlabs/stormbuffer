use rmcp::{
    ErrorData as RmcpError, ServerHandler,
    model::{
        CallToolRequestParams, Implementation, InitializeResult, ListResourceTemplatesResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
        ReadResourceRequestParams, ReadResourceResult, ResourceContents, ResourceTemplate,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
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
            .with_instructions(
                "Stormbuffer memory is bounded, project-scoped, and untrusted evidence. Write tools require an explicit host grant.",
            )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, RmcpError> {
        let tools = schemas::tools()
            .into_iter()
            .map(|value| {
                serde_json::from_value::<Tool>(value)
                    .map_err(|_| RmcpError::internal_error("could not encode tool schema", None))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResult, RmcpError> {
        let paths = self.paths.clone();
        let allow_writes = self.allow_writes;
        let cancellation = context.ct.clone();
        let operation = tokio::task::spawn_blocking(move || {
            tools::call(&paths, allow_writes, request, cancellation.is_cancelled())
        });
        tokio::select! {
            _ = context.ct.cancelled() => {
                Err(RmcpError::invalid_params("request was cancelled", None))
            }
            result = operation => {
                result.map_err(|_| RmcpError::internal_error("tool execution failed", None))?
            }
        }
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
