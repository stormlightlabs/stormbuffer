use rmcp::model::JsonObject;
use serde_json::{Value, json};
use stormbuffer_core as core;

fn object_schema(properties: Value, required: &[&str]) -> JsonObject {
    JsonObject::from_iter([
        ("type".to_owned(), json!("object")),
        ("properties".to_owned(), properties),
        ("required".to_owned(), json!(required)),
        ("additionalProperties".to_owned(), json!(false)),
    ])
}

pub fn memory_recall() -> JsonObject {
    object_schema(
        json!({
            "query": { "type": "string", "maxLength": core::MAX_INVOKE_QUERY },
            "limit": { "type": "integer", "minimum": 1, "maximum": core::MAX_INVOKE_LIMIT },
            "budget": { "type": "integer", "minimum": 1, "maximum": core::MAX_INVOKE_BUDGET },
            "scope": { "type": "string" },
            "scopes": { "type": "array", "items": { "type": "string" } },
            "access": { "const": "agent" }
        }),
        &["query"],
    )
}

pub fn memory_get() -> JsonObject {
    object_schema(
        json!({
            "id": { "type": "string" },
            "scope": { "type": "string" },
            "scopes": { "type": "array", "items": { "type": "string" } },
            "access": { "const": "agent" }
        }),
        &["id"],
    )
}

pub fn memory_remember() -> JsonObject {
    object_schema(
        json!({
            "title": { "type": "string" },
            "kind": { "type": "string", "enum": ["fact", "decision", "procedure", "checkpoint"] },
            "scope": { "type": "string" },
            "body": { "type": "string", "maxLength": core::MAX_INVOKE_OUTPUT_BODY },
            "tags": { "type": "array", "items": { "type": "string" } },
            "aliases": { "type": "array", "items": { "type": "string" } },
            "source": source_schema()
        }),
        &["title", "kind", "body", "source"],
    )
}

pub fn memory_update() -> JsonObject {
    object_schema(
        json!({
            "id": { "type": "string" },
            "title": { "type": "string" },
            "kind": { "type": "string" },
            "body": { "type": "string", "maxLength": core::MAX_INVOKE_OUTPUT_BODY },
            "scope": { "type": "string" },
            "scopes": { "type": "array", "items": { "type": "string" } },
            "tags": { "type": "array", "items": { "type": "string" } },
            "aliases": { "type": "array", "items": { "type": "string" } },
            "source": source_schema()
        }),
        &["id", "body", "source"],
    )
}

pub fn memory_forget() -> JsonObject {
    object_schema(
        json!({
            "id": { "type": "string" },
            "scope": { "type": "string" },
            "scopes": { "type": "array", "items": { "type": "string" } },
            "access": { "const": "agent" }
        }),
        &["id"],
    )
}

fn source_schema() -> Value {
    Value::Object(object_schema(
        json!({
            "kind": { "type": "string", "enum": ["conversation", "document", "issue", "url"] },
            "reference": { "type": "string", "maxLength": 2048 },
            "actor": { "type": "string", "maxLength": 256 }
        }),
        &["kind", "reference", "actor"],
    ))
}

pub fn resource_templates() -> Vec<Value> {
    vec![
        json!({
            "uriTemplate": "stormbuffer://record/{id}",
            "name": "record",
            "description": "One agent-readable record as JSON.",
            "mimeType": "application/json"
        }),
        json!({
            "uriTemplate": "stormbuffer://scope/{scope}/records",
            "name": "scope-records",
            "description": "Active agent-readable records in one allowed scope.",
            "mimeType": "application/json"
        }),
        json!({
            "uriTemplate": "stormbuffer://candidate/{id}",
            "name": "candidate",
            "description": "One agent-readable candidate as JSON.",
            "mimeType": "application/json"
        }),
    ]
}
