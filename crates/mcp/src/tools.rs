use rmcp::{
    ErrorData as RmcpError,
    model::{CallToolRequestParams, CallToolResult},
};
use serde_json::{Value, json};
use stormbuffer_core as core;

use crate::{config::MAX_TOOL_ENVELOPE_BYTES, schemas};

pub fn call(
    paths: &core::StorePaths,
    allow_writes: bool,
    request: CallToolRequestParams,
    cancelled: bool,
) -> Result<CallToolResult, RmcpError> {
    if cancelled {
        return Err(RmcpError::invalid_params("request was cancelled", None));
    }
    let operation = schemas::operation(request.name.as_ref())
        .ok_or_else(|| RmcpError::invalid_params("tool is not supported", None))?;
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
        let mut arguments = request.arguments.unwrap_or_default();
        arguments.insert("version".to_owned(), json!(core::INVOKE_VERSION));
        arguments.insert("operation".to_owned(), Value::String(operation.to_owned()));
        core::invoke_envelope(
            operation,
            core::invoke_operation(paths, operation, &arguments),
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
