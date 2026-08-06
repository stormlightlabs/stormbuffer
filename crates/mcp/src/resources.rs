use serde_json::{Map, Value, json};
use stormbuffer_core as core;

pub fn read(paths: &core::StorePaths, uri: &str) -> Result<Value, core::InvokeFailure> {
    let Some(rest) = uri.strip_prefix("stormbuffer://") else {
        return Err(core::InvokeFailure::new(
            "invalid_request",
            "resource URI is not a Stormbuffer URI",
        ));
    };
    if let Some(id) = rest.strip_prefix("record/") {
        return record(paths, id, false);
    }
    if let Some(id) = rest.strip_prefix("candidate/") {
        return record(paths, id, true);
    }
    if let Some(scope) = rest
        .strip_prefix("scope/")
        .and_then(|value| value.strip_suffix("/records"))
    {
        return core::invoke_scope_records(paths, scope);
    }
    Err(core::InvokeFailure::new(
        "not_found",
        "resource URI was not found",
    ))
}

fn record(
    paths: &core::StorePaths,
    id: &str,
    candidate: bool,
) -> Result<Value, core::InvokeFailure> {
    if id.is_empty() || id.contains('/') {
        return Err(core::InvokeFailure::new(
            "invalid_request",
            "resource identifier is invalid",
        ));
    }
    let request = Map::from_iter([
        ("version".to_owned(), json!(core::INVOKE_VERSION)),
        ("operation".to_owned(), Value::String("get".to_owned())),
        ("id".to_owned(), Value::String(id.to_owned())),
    ]);
    let value = core::invoke_operation(paths, "get", &request)?;
    if candidate && value.get("status").and_then(Value::as_str) != Some("candidate") {
        return Err(core::InvokeFailure::new(
            "not_found",
            "candidate was not found",
        ));
    }
    Ok(value)
}
