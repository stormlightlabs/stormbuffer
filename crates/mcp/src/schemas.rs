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
        "stormbuffer_search" => Some("search"),
        "stormbuffer_context" => Some("context"),
        "stormbuffer_get" => Some("get"),
        "stormbuffer_propose" => Some("propose"),
        "stormbuffer_supersede" => Some("supersede"),
        "stormbuffer_archive" => Some("archive"),
        _ => None,
    }
}

pub fn tools() -> Vec<Value> {
    vec![
        tool_definition(
            "stormbuffer_search",
            "Search bounded, agent-readable active memory.",
            object_schema(
                json!({
                    "query": { "type": "string", "maxLength": core::MAX_INVOKE_QUERY },
                    "limit": { "type": "integer", "minimum": 1, "maximum": core::MAX_INVOKE_LIMIT },
                    "scope": { "type": "string" },
                    "scopes": { "type": "array", "items": { "type": "string" } },
                    "access": { "const": "agent" }
                }),
                &["query"],
            ),
        ),
        tool_definition(
            "stormbuffer_context",
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
            "stormbuffer_get",
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
            "stormbuffer_propose",
            "Create a sourced candidate that still needs human approval.",
            object_schema(
                json!({
                    "record": { "type": "object" },
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "kind": { "type": "string", "enum": ["fact", "decision", "procedure", "checkpoint"] },
                    "scope": { "type": "string" },
                    "access": { "const": "agent" },
                    "body": { "type": "string", "maxLength": core::MAX_INVOKE_OUTPUT_BODY },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "aliases": { "type": "array", "items": { "type": "string" } },
                    "supersedes": { "type": "array", "items": { "type": "string" } },
                    "sources": { "type": "array", "items": { "type": "object" } }
                }),
                &[],
            ),
        ),
        tool_definition(
            "stormbuffer_supersede",
            "Create an active replacement and retain the superseded record.",
            object_schema(
                json!({
                    "id": { "type": "string" },
                    "replacement": { "type": "object" },
                    "title": { "type": "string" },
                    "kind": { "type": "string" },
                    "body": { "type": "string", "maxLength": core::MAX_INVOKE_OUTPUT_BODY },
                    "scope": { "type": "string" },
                    "access": { "const": "agent" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "aliases": { "type": "array", "items": { "type": "string" } },
                    "supersedes": { "type": "array", "items": { "type": "string" } },
                    "sources": { "type": "array", "items": { "type": "object" } }
                }),
                &["id"],
            ),
        ),
        tool_definition(
            "stormbuffer_archive",
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
