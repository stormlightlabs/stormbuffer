use rmcp::{
    ErrorData as RmcpError,
    model::{CallToolResult, JsonObject},
};
use serde_json::{Value, json};
use stormbuffer_core as core;

use crate::config::MAX_TOOL_ENVELOPE_BYTES;

pub fn call(
    paths: &core::StorePaths,
    allow_writes: bool,
    operation: &str,
    mut arguments: JsonObject,
    cancelled: bool,
    embedder: Option<&dyn core::Embedder>,
) -> Result<CallToolResult, RmcpError> {
    if cancelled {
        return Err(RmcpError::invalid_params("request was cancelled", None));
    }
    let write = matches!(operation, "remember" | "update" | "archive");
    let envelope = if write && !allow_writes {
        core::invoke_envelope(
            operation,
            Err(core::InvokeFailure::new(
                "permission_denied",
                "MCP write tools are disabled; restart with an explicit host write grant",
            )),
        )
    } else {
        arguments.insert("version".to_owned(), json!(core::INVOKE_VERSION));
        arguments.insert("operation".to_owned(), Value::String(operation.to_owned()));
        core::invoke_envelope(
            operation,
            core::invoke_operation_with_embedder(paths, operation, &arguments, embedder),
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
