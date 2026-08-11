use serde_json::{Value, json};
use stormbuffer_core as core;

fn object_schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn tool_definition(name: &str, description: &str, input_schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": input_schema })
}

pub fn operation(name: &str) -> Option<&'static str> {
    match name {
        "memory_recall" => Some("context"),
        "memory_get" => Some("get"),
        "memory_remember" => Some("remember"),
        "memory_update" => Some("update"),
        "memory_forget" => Some("archive"),
        _ => None,
    }
}

pub fn tools() -> Vec<Value> {
    vec![
        tool_definition(
            "memory_recall",
            "Compile bounded evidence blocks and a receipt for an agent question.",
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
            ),
        ),
        tool_definition(
            "memory_get",
            "Read one agent-readable record without its host path.",
            object_schema(
                json!({
                    "id": { "type": "string" },
                    "scope": { "type": "string" },
                    "scopes": { "type": "array", "items": { "type": "string" } },
                    "access": { "const": "agent" }
                }),
                &["id"],
            ),
        ),
        tool_definition(
            "memory_remember",
            "Create a sourced candidate that still needs human approval.",
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
            ),
        ),
        tool_definition(
            "memory_update",
            "Create a sourced replacement candidate linked to an active memory.",
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
            ),
        ),
        tool_definition(
            "memory_forget",
            "Archive an active record without deleting its canonical Markdown.",
            object_schema(
                json!({
                    "id": { "type": "string" },
                    "scope": { "type": "string" },
                    "scopes": { "type": "array", "items": { "type": "string" } },
                    "access": { "const": "agent" }
                }),
                &["id"],
            ),
        ),
    ]
}

fn source_schema() -> Value {
    object_schema(
        json!({
            "kind": { "type": "string", "enum": ["conversation", "document", "issue", "url"] },
            "reference": { "type": "string", "maxLength": 2048 },
            "actor": { "type": "string", "maxLength": 256 }
        }),
        &["kind", "reference", "actor"],
    )
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
