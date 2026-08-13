use rmcp::{
    ErrorData as RmcpError,
    model::{CallToolResult, JsonObject},
};
use serde_json::{Value, json};
use stormbuffer_core as core;

use crate::config::{MAX_TOOL_ENVELOPE_BYTES, McpWritePolicy};

pub fn call(
    paths: &core::StorePaths,
    write_policy: McpWritePolicy,
    operation: &str,
    mut arguments: JsonObject,
    cancelled: bool,
    embedder: Option<&dyn core::Embedder>,
    unavailable_reason: Option<core::SemanticFallbackReason>,
) -> Result<CallToolResult, RmcpError> {
    if cancelled {
        return Err(RmcpError::invalid_params("request was cancelled", None));
    }
    let write = matches!(operation, "remember" | "update" | "archive");
    let envelope = if write && !write_policy.allows(operation) {
        let message = match write_policy {
            McpWritePolicy::ReadOnly => {
                "MCP write tools are disabled; restart with an explicit host write grant"
            }
            McpWritePolicy::CandidateOnly => {
                "MCP candidate-write mode permits only remember and update"
            }
            McpWritePolicy::All => unreachable!("all supported MCP writes are permitted"),
        };
        core::invoke_envelope(
            operation,
            Err(core::InvokeFailure::new("permission_denied", message)),
        )
    } else {
        arguments.insert("version".to_owned(), json!(core::INVOKE_VERSION));
        arguments.insert("operation".to_owned(), Value::String(operation.to_owned()));
        core::invoke_envelope(
            operation,
            core::invoke_operation_with_semantic_status(
                paths,
                operation,
                &arguments,
                embedder,
                unavailable_reason,
            ),
        )
    };
    let encoded = serde_json::to_string(&envelope)
        .map_err(|_| RmcpError::internal_error("could not encode tool result", None))?;
    if encoded.len() > MAX_TOOL_ENVELOPE_BYTES {
        return Ok(error_result(
            operation,
            "response exceeds the bounded MCP tool output",
        ));
    }
    let result = if envelope.get("ok") == Some(&Value::Bool(true)) {
        CallToolResult::structured(envelope)
    } else {
        CallToolResult::structured_error(envelope)
    };
    if serde_json::to_vec(&result)
        .map_err(|_| RmcpError::internal_error("could not encode tool result", None))?
        .len()
        > MAX_TOOL_ENVELOPE_BYTES * 2
    {
        return Ok(error_result(
            operation,
            "response exceeds the bounded MCP tool output",
        ));
    }
    Ok(result)
}

fn error_result(operation: &str, message: &str) -> CallToolResult {
    CallToolResult::structured_error(core::invoke_envelope(
        operation,
        Err(core::InvokeFailure::new("output_too_large", message)),
    ))
}
