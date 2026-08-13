use serde_json::{Map, Value, json};

pub const INVOKE_VERSION: u64 = 1;
pub const MAX_INVOKE_INPUT: usize = 256 * 1024;
pub const MAX_INVOKE_OUTPUT: usize = 256 * 1024;
pub const MAX_INVOKE_QUERY: usize = 2048;
pub const MAX_INVOKE_OUTPUT_BODY: usize = 64 * 1024;
pub const MAX_INVOKE_LIMIT: usize = 100;
pub const MAX_INVOKE_BUDGET: usize = 4096;

#[derive(Debug)]
pub struct InvokeFailure {
    code: &'static str,
    message: String,
}

impl InvokeFailure {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub fn invoke_envelope(operation: &str, result: Result<Value, InvokeFailure>) -> Value {
    match result {
        Ok(result) => json!({
            "version": INVOKE_VERSION,
            "operation": operation,
            "ok": true,
            "result": result
        }),
        Err(error) => json!({
            "version": INVOKE_VERSION,
            "operation": operation,
            "ok": false,
            "error": { "code": error.code, "message": error.message }
        }),
    }
}

pub fn invoke_request(
    paths: &crate::StorePaths,
    operation: &str,
    input: &[u8],
) -> Result<Value, InvokeFailure> {
    invoke_request_with_embedder(paths, operation, input, None)
}

pub fn invoke_request_with_embedder(
    paths: &crate::StorePaths,
    operation: &str,
    input: &[u8],
    embedder: Option<&dyn crate::Embedder>,
) -> Result<Value, InvokeFailure> {
    let value: Value = serde_json::from_slice(input)
        .map_err(|_| InvokeFailure::new("invalid_json", "stdin must contain one JSON object"))?;
    let map = value
        .as_object()
        .ok_or_else(|| InvokeFailure::new("invalid_request", "request must be a JSON object"))?;
    invoke_operation_with_embedder(paths, operation, map, embedder)
}

pub fn invoke_operation(
    paths: &crate::StorePaths,
    operation: &str,
    map: &Map<String, Value>,
) -> Result<Value, InvokeFailure> {
    invoke_operation_with_embedder(paths, operation, map, None)
}

pub fn invoke_operation_with_embedder(
    paths: &crate::StorePaths,
    operation: &str,
    map: &Map<String, Value>,
    embedder: Option<&dyn crate::Embedder>,
) -> Result<Value, InvokeFailure> {
    invoke_operation_with_semantic_status(
        paths,
        operation,
        map,
        embedder,
        embedder
            .is_none()
            .then_some(crate::SemanticFallbackReason::IntentionallyUnavailable),
    )
}

pub fn invoke_operation_with_semantic_status(
    paths: &crate::StorePaths,
    operation: &str,
    map: &Map<String, Value>,
    embedder: Option<&dyn crate::Embedder>,
    unavailable_reason: Option<crate::SemanticFallbackReason>,
) -> Result<Value, InvokeFailure> {
    request_map(map, operation)?;
    match operation {
        "search" => invoke_search(paths, map, embedder, unavailable_reason),
        "context" => invoke_context(paths, map, embedder, unavailable_reason),
        "get" => invoke_get(paths, map),
        "remember" => invoke_remember(paths, map),
        "update" => invoke_update(paths, map),
        "propose" => invoke_propose(paths, map),
        "supersede" => invoke_supersede(paths, map),
        "archive" => invoke_archive(paths, map),
        _ => Err(InvokeFailure::new(
            "unknown_operation",
            "operation is not supported by protocol version 1",
        )),
    }
}

fn request_map(map: &Map<String, Value>, operation: &str) -> Result<(), InvokeFailure> {
    if let Some(version) = map.get("version").and_then(Value::as_u64) {
        if version != INVOKE_VERSION {
            return Err(InvokeFailure::new(
                "unsupported_version",
                "request version is not supported",
            ));
        }
    } else {
        return Err(InvokeFailure::new(
            "invalid_request",
            "request must include integer version 1",
        ));
    }
    if let Some(request_operation) = map.get("operation") {
        if request_operation.as_str() != Some(operation) {
            return Err(InvokeFailure::new(
                "invalid_request",
                "request operation does not match the command operation",
            ));
        }
    }
    if !matches!(
        operation,
        "search" | "context" | "get" | "remember" | "update" | "propose" | "supersede" | "archive"
    ) {
        return Err(InvokeFailure::new(
            "unknown_operation",
            "operation is not supported by protocol version 1",
        ));
    }
    Ok(())
}

fn ensure_keys(map: &Map<String, Value>, allowed: &[&str]) -> Result<(), InvokeFailure> {
    for key in map.keys() {
        if key == "version" || key == "operation" || allowed.contains(&key.as_str()) {
            continue;
        }
        if key == "path"
            || key.ends_with("_path")
            || key.contains("file")
            || key == "root"
            || key == "store"
        {
            return Err(InvokeFailure::new(
                "path_denied",
                "filesystem paths are not accepted by the invocation protocol",
            ));
        }
        return Err(InvokeFailure::new(
            "invalid_request",
            "request contains an unknown field",
        ));
    }
    Ok(())
}

fn required_string<'a>(map: &'a Map<String, Value>, key: &str) -> Result<&'a str, InvokeFailure> {
    map.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= MAX_INVOKE_OUTPUT_BODY)
        .ok_or_else(|| {
            InvokeFailure::new(
                "invalid_request",
                format!("field `{key}` must be a bounded string"),
            )
        })
}

fn bounded_number(
    map: &Map<String, Value>,
    key: &str,
    default: usize,
    maximum: usize,
) -> Result<usize, InvokeFailure> {
    let Some(value) = map.get(key) else {
        return Ok(default);
    };
    let number = value
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(|| {
            InvokeFailure::new(
                "invalid_request",
                format!("field `{key}` must be an integer"),
            )
        })?;
    Ok(number.clamp(1, maximum))
}

fn invocation_access(map: &Map<String, Value>) -> Result<Vec<crate::Access>, InvokeFailure> {
    let caller = map
        .get("access")
        .and_then(Value::as_str)
        .unwrap_or("agent")
        .parse::<crate::Access>()
        .map_err(|_| InvokeFailure::new("invalid_request", "access must be `agent` or `human`"))?;
    if caller == crate::Access::Human {
        return Err(InvokeFailure::new(
            "permission_denied",
            "the invoke protocol is limited to agent-readable records",
        ));
    }
    Ok(vec![crate::Access::Agent])
}

fn invocation_scope(
    paths: &crate::StorePaths,
    map: &Map<String, Value>,
) -> Result<Vec<String>, InvokeFailure> {
    let defaults = crate::SearchOptions::for_store(paths)
        .allowed_scopes
        .unwrap_or_default();
    let requested = match (map.get("scope"), map.get("scopes")) {
        (Some(_), Some(_)) => {
            return Err(InvokeFailure::new(
                "invalid_request",
                "use either scope or scopes, not both",
            ));
        }
        (Some(value), None) => vec![
            value
                .as_str()
                .ok_or_else(|| InvokeFailure::new("invalid_request", "scope must be a string"))?
                .to_owned(),
        ],
        (None, Some(value)) => value
            .as_array()
            .ok_or_else(|| InvokeFailure::new("invalid_request", "scopes must be an array"))?
            .iter()
            .map(|value| {
                value.as_str().map(str::to_owned).ok_or_else(|| {
                    InvokeFailure::new("invalid_request", "scopes must contain strings")
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        (None, None) => defaults.clone(),
    };
    if requested.is_empty() || requested.len() > 16 {
        return Err(InvokeFailure::new(
            "invalid_request",
            "scope filter is out of bounds",
        ));
    }
    for scope in &requested {
        if scope.parse::<crate::Scope>().is_err()
            || !defaults.iter().any(|allowed| allowed == scope)
        {
            return Err(InvokeFailure::new(
                "scope_denied",
                "requested scope is outside the selected store boundary",
            ));
        }
    }
    Ok(requested)
}

fn invocation_stores(
    paths: &crate::StorePaths,
    allowed_scopes: &[String],
    embedder: Option<&dyn crate::Embedder>,
    unavailable_reason: Option<crate::SemanticFallbackReason>,
) -> Result<
    (
        Vec<crate::StorePaths>,
        Option<crate::SemanticFallbackReason>,
    ),
    InvokeFailure,
> {
    let cwd = std::env::current_dir()
        .map_err(|_| InvokeFailure::new("internal_error", "could not resolve selected stores"))?;
    let mut stores =
        crate::retrieval_stores(paths, &cwd).map_err(|error| map_core_error(&error))?;
    stores.retain(|store| {
        store.scope != crate::StoreScope::Global
            || allowed_scopes.iter().any(|scope| scope == "global")
    });
    let mut fallback = if embedder.is_none() {
        Some(unavailable_reason.unwrap_or(crate::SemanticFallbackReason::IntentionallyUnavailable))
    } else {
        None
    };
    for store in &stores {
        let report = match crate::sync_store(store) {
            Ok(report) => report,
            Err(crate::Error::IndexBusy) => {
                fallback = Some(crate::SemanticFallbackReason::VectorProjectionBusy);
                continue;
            }
            Err(error) => return Err(map_core_error(&error)),
        };
        if !report.is_complete() {
            return Err(InvokeFailure::new(
                "invalid_record",
                "one or more canonical records are invalid",
            ));
        }
        if let Some(embedder) = embedder {
            if fallback.is_some() {
                continue;
            }
            if let Err(error) = crate::rebuild_vector_index(store, embedder) {
                fallback = match error {
                    crate::Error::Embedding { .. } => {
                        Some(crate::SemanticFallbackReason::EmbeddingExecutionFailed)
                    }
                    crate::Error::IndexBusy => {
                        Some(crate::SemanticFallbackReason::VectorProjectionBusy)
                    }
                    crate::Error::IndexUnavailable { .. } => {
                        Some(crate::SemanticFallbackReason::VectorProjectionUnavailable)
                    }
                    _ => return Err(map_core_error(&error)),
                };
            }
        }
    }
    Ok((stores, fallback))
}

fn invoke_search(
    paths: &crate::StorePaths,
    map: &Map<String, Value>,
    embedder: Option<&dyn crate::Embedder>,
    unavailable_reason: Option<crate::SemanticFallbackReason>,
) -> Result<Value, InvokeFailure> {
    ensure_keys(map, &["query", "limit", "scope", "scopes", "access"])?;
    let query = required_string(map, "query")?;
    if query.len() > MAX_INVOKE_QUERY {
        return Err(InvokeFailure::new("invalid_request", "query is too long"));
    }
    let limit = bounded_number(map, "limit", 20, MAX_INVOKE_LIMIT)?;
    let scopes = invocation_scope(paths, map)?;
    let access = invocation_access(map)?;
    let (stores, fallback) = invocation_stores(paths, &scopes, embedder, unavailable_reason)?;
    let mut options = crate::SearchOptions::for_store(paths);
    options.limit = limit;
    options.allowed_scopes = Some(scopes);
    options.allowed_access = Some(access);
    let results = if let Some(embedder) = embedder.filter(|_| fallback.is_none()) {
        options.mode = crate::RetrievalMode::Hybrid;
        match crate::search_stores_with_embedder(&stores, query, options.clone(), embedder) {
            Ok(results) => Ok(results),
            Err(crate::Error::Embedding { .. }) => {
                options.mode = crate::RetrievalMode::Lexical;
                crate::search_stores(&stores, query, options)
            }
            Err(error) => Err(error),
        }
    } else {
        options.mode = crate::RetrievalMode::Lexical;
        crate::search_stores(&stores, query, options)
    }
    .map_err(|error| map_core_error(&error))?;
    Ok(Value::Array(
        results.iter().map(invoke_search_result).collect(),
    ))
}

fn invoke_context(
    paths: &crate::StorePaths,
    map: &Map<String, Value>,
    embedder: Option<&dyn crate::Embedder>,
    unavailable_reason: Option<crate::SemanticFallbackReason>,
) -> Result<Value, InvokeFailure> {
    ensure_keys(
        map,
        &["query", "limit", "budget", "scope", "scopes", "access"],
    )?;
    let query = required_string(map, "query")?;
    if query.len() > MAX_INVOKE_QUERY {
        return Err(InvokeFailure::new("invalid_request", "query is too long"));
    }
    let limit = bounded_number(map, "limit", 20, MAX_INVOKE_LIMIT)?;
    let budget = bounded_number(map, "budget", 512, MAX_INVOKE_BUDGET)?;
    let scopes = invocation_scope(paths, map)?;
    let access = invocation_access(map)?;
    let (stores, mut fallback) = invocation_stores(paths, &scopes, embedder, unavailable_reason)?;
    let mut search = crate::SearchOptions::for_store(paths);
    search.limit = limit;
    search.allowed_scopes = Some(scopes);
    search.allowed_access = Some(access);
    let options = crate::ContextOptions { budget, search };
    let mut result = if let Some(embedder) = embedder.filter(|_| fallback.is_none()) {
        let mut options = options;
        options.search.mode = crate::RetrievalMode::Hybrid;
        match crate::context_stores_with_embedder(&stores, query, options.clone(), embedder) {
            Ok(result) => Ok(result),
            Err(crate::Error::Embedding { .. }) => {
                fallback = Some(crate::SemanticFallbackReason::EmbeddingExecutionFailed);
                let mut options = options;
                options.search.mode = crate::RetrievalMode::Lexical;
                crate::context_stores(&stores, query, options)
            }
            Err(error) => Err(error),
        }
    } else {
        let mut options = options;
        options.search.mode = crate::RetrievalMode::Lexical;
        crate::context_stores(&stores, query, options)
    }
    .map_err(|error| map_core_error(&error))?;
    result.receipt.semantic_fallback = fallback;
    serde_json::to_value(result)
        .map_err(|_| InvokeFailure::new("internal_error", "could not encode context result"))
}

fn invoke_get(paths: &crate::StorePaths, map: &Map<String, Value>) -> Result<Value, InvokeFailure> {
    ensure_keys(map, &["id", "scope", "scopes", "access"])?;
    let id = parse_protocol_id(required_string(map, "id")?)?;
    let scopes = invocation_scope(paths, map)?;
    let access = invocation_access(map)?;
    let (stores, _) = invocation_stores(paths, &scopes, None, None)?;
    let mut denied = None;
    for store in stores {
        match crate::RecordRepository::new(store).find_allowed(id, &scopes, &access) {
            Ok(stored) => {
                if stored.record().body.len() > MAX_INVOKE_OUTPUT_BODY {
                    return Err(InvokeFailure::new(
                        "output_too_large",
                        "record exceeds the bounded protocol output",
                    ));
                }
                return Ok(invoke_record(stored.record()));
            }
            Err(
                error @ crate::Error::Repository {
                    source: crate::RepositoryError::ScopeDenied { .. },
                },
            )
            | Err(
                error @ crate::Error::Repository {
                    source: crate::RepositoryError::AccessDenied { .. },
                },
            ) => {
                denied = Some(map_core_error(&error));
            }
            Err(crate::Error::Repository {
                source: crate::RepositoryError::NotFound { .. },
            }) => {}
            Err(error) => return Err(map_core_error(&error)),
        }
    }
    denied.map_or_else(
        || Err(InvokeFailure::new("not_found", "record was not found")),
        Err,
    )
}

fn invoke_remember(
    paths: &crate::StorePaths,
    map: &Map<String, Value>,
) -> Result<Value, InvokeFailure> {
    ensure_keys(
        map,
        &[
            "actor", "approved", "title", "kind", "scope", "tags", "aliases", "source", "body",
        ],
    )?;
    ensure_agent_write(map)?;
    let mut record_map = Map::new();
    for key in ["title", "kind", "scope", "tags", "aliases", "body"] {
        if let Some(value) = map.get(key) {
            record_map.insert(key.to_owned(), value.clone());
        }
    }
    record_map.insert("access".to_owned(), Value::String("agent".to_owned()));
    record_map.insert("status".to_owned(), Value::String("candidate".to_owned()));
    record_map.insert("sources".to_owned(), protocol_source_array(map)?);
    let record = parse_protocol_record(
        &Value::Object(record_map),
        None,
        &default_protocol_scope(paths)?,
    )?;
    reject_secret_candidate(&record)?;
    let result = crate::RecordRepository::new(paths.clone())
        .propose(record, crate::ProposalActor::Agent)
        .map_err(|error| map_core_error(&error))?;
    serde_json::to_value(result)
        .map_err(|_| InvokeFailure::new("internal_error", "could not encode remember result"))
}

fn invoke_update(
    paths: &crate::StorePaths,
    map: &Map<String, Value>,
) -> Result<Value, InvokeFailure> {
    ensure_keys(
        map,
        &[
            "actor", "approved", "id", "scope", "scopes", "title", "kind", "tags", "aliases",
            "source", "body",
        ],
    )?;
    ensure_agent_write(map)?;
    let target_id = parse_protocol_id(required_string(map, "id")?)?;
    let scopes = invocation_scope(paths, map)?;
    let repository = crate::RecordRepository::new(paths.clone());
    let old = repository
        .find_allowed(target_id, &scopes, &[crate::Access::Agent])
        .map_err(|error| map_core_error(&error))?;
    let mut replacement_map = Map::new();
    for key in ["title", "kind", "tags", "aliases", "body"] {
        if let Some(value) = map.get(key) {
            replacement_map.insert(key.to_owned(), value.clone());
        }
    }
    replacement_map.insert(
        "id".to_owned(),
        Value::String(crate::RecordId::new_v7().to_string()),
    );
    let now = crate::Timestamp::now_utc().to_string();
    replacement_map.insert("created_at".to_owned(), Value::String(now.clone()));
    replacement_map.insert("updated_at".to_owned(), Value::String(now));
    replacement_map.insert("access".to_owned(), Value::String("agent".to_owned()));
    replacement_map.insert("status".to_owned(), Value::String("candidate".to_owned()));
    replacement_map.insert("sources".to_owned(), protocol_source_array(map)?);
    let replacement = parse_protocol_record(
        &Value::Object(replacement_map),
        Some(old.record()),
        old.record().scope.as_str(),
    )?;
    reject_secret_candidate(&replacement)?;
    let result = repository
        .propose_update(target_id, replacement)
        .map_err(|error| map_core_error(&error))?;
    serde_json::to_value(result)
        .map_err(|_| InvokeFailure::new("internal_error", "could not encode update result"))
}

fn reject_secret_candidate(record: &crate::Record) -> Result<(), InvokeFailure> {
    if crate::secret_guard::contains_likely_secret(record) {
        return Err(InvokeFailure::new(
            "secret_detected",
            "candidate contains credential-like material; remove it before retrying",
        ));
    }
    Ok(())
}

fn ensure_agent_write(map: &Map<String, Value>) -> Result<(), InvokeFailure> {
    let actor = map.get("actor").and_then(Value::as_str).unwrap_or("agent");
    if actor != "agent" || map.contains_key("approved") {
        return Err(InvokeFailure::new(
            "permission_denied",
            "agent writes always require explicit human approval",
        ));
    }
    Ok(())
}

fn protocol_source_array(map: &Map<String, Value>) -> Result<Value, InvokeFailure> {
    match map.get("source") {
        Some(source) if source.is_object() => Ok(Value::Array(vec![source.clone()])),
        Some(_) => Err(InvokeFailure::new(
            "invalid_request",
            "source must be an object",
        )),
        None => Ok(Value::Array(Vec::new())),
    }
}

fn invoke_propose(
    paths: &crate::StorePaths,
    map: &Map<String, Value>,
) -> Result<Value, InvokeFailure> {
    ensure_keys(
        map,
        &[
            "actor",
            "approved",
            "record",
            "id",
            "title",
            "kind",
            "scope",
            "status",
            "access",
            "created_at",
            "updated_at",
            "tags",
            "aliases",
            "supersedes",
            "sources",
            "body",
        ],
    )?;
    let actor = map
        .get("actor")
        .and_then(Value::as_str)
        .unwrap_or("agent")
        .parse::<crate::ProposalActor>()
        .map_err(|_| InvokeFailure::new("invalid_request", "actor must be `agent` or `human`"))?;
    if actor == crate::ProposalActor::Human || map.contains_key("approved") {
        return Err(InvokeFailure::new(
            "permission_denied",
            "invoke proposals always require explicit human approval",
        ));
    }
    let scope = default_protocol_scope(paths)?;
    let record_value = if let Some(value) = map.get("record") {
        value.clone()
    } else {
        Value::Object(
            map.iter()
                .filter(|(key, _)| {
                    !matches!(key.as_str(), "version" | "operation" | "actor" | "approved")
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        )
    };
    let record = parse_protocol_record(&record_value, None, &scope)?;
    ensure_agent_record(&record)?;
    let result = crate::RecordRepository::new(paths.clone())
        .propose(record, crate::ProposalActor::Agent)
        .map_err(|error| map_core_error(&error))?;
    serde_json::to_value(result)
        .map_err(|_| InvokeFailure::new("internal_error", "could not encode proposal result"))
}

fn invoke_supersede(
    paths: &crate::StorePaths,
    map: &Map<String, Value>,
) -> Result<Value, InvokeFailure> {
    ensure_keys(
        map,
        &[
            "id",
            "access",
            "replacement",
            "title",
            "kind",
            "scope",
            "status",
            "created_at",
            "updated_at",
            "tags",
            "aliases",
            "supersedes",
            "sources",
            "body",
        ],
    )?;
    let target_id = parse_protocol_id(required_string(map, "id")?)?;
    let scopes = invocation_scope(paths, map)?;
    let access = invocation_access(map)?;
    let repository = crate::RecordRepository::new(paths.clone());
    let old = repository
        .find_allowed(target_id, &scopes, &access)
        .map_err(|error| map_core_error(&error))?;
    let default_scope = old.record().scope.to_string();
    let replacement_value = if let Some(value) = map.get("replacement") {
        value.clone()
    } else {
        Value::Object(
            map.iter()
                .filter(|(key, _)| {
                    !matches!(key.as_str(), "version" | "operation" | "id" | "access")
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        )
    };
    let mut replacement =
        parse_protocol_record(&replacement_value, Some(old.record()), &default_scope)?;
    ensure_agent_record(&replacement)?;
    if replacement.id == target_id {
        replacement.id = crate::RecordId::new_v7();
    }
    if !replacement.supersedes.contains(&target_id) {
        replacement.supersedes.push(target_id);
    }
    let stored = repository
        .supersede(target_id, replacement)
        .map_err(|error| map_core_error(&error))?;
    Ok(
        json!({ "outcome": "accepted", "id": stored.record().id.to_string(), "status": stored.record().status.to_string() }),
    )
}

fn ensure_agent_record(record: &crate::Record) -> Result<(), InvokeFailure> {
    if record.access == crate::Access::Agent {
        Ok(())
    } else {
        Err(InvokeFailure::new(
            "permission_denied",
            "agent invocations may only write agent-accessible records",
        ))
    }
}

fn invoke_archive(
    paths: &crate::StorePaths,
    map: &Map<String, Value>,
) -> Result<Value, InvokeFailure> {
    ensure_keys(map, &["id", "scope", "scopes", "access"])?;
    let id = parse_protocol_id(required_string(map, "id")?)?;
    let scopes = invocation_scope(paths, map)?;
    let access = invocation_access(map)?;
    let repository = crate::RecordRepository::new(paths.clone());
    repository
        .find_allowed(id, &scopes, &access)
        .map_err(|error| map_core_error(&error))?;
    let stored = repository
        .archive(id)
        .map_err(|error| map_core_error(&error))?;
    Ok(
        json!({ "outcome": "accepted", "id": stored.record().id.to_string(), "status": stored.record().status.to_string() }),
    )
}

fn parse_protocol_id(value: &str) -> Result<crate::RecordId, InvokeFailure> {
    value
        .parse()
        .map_err(|_| InvokeFailure::new("invalid_request", "id must be a valid record identifier"))
}

fn default_protocol_scope(paths: &crate::StorePaths) -> Result<String, InvokeFailure> {
    crate::record_scope(paths)
        .map(|scope| scope.as_str().to_owned())
        .map_err(|error| map_core_error(&error))
}

const PROTOCOL_RECORD_FIELDS: &[&str] = &[
    "id",
    "title",
    "kind",
    "scope",
    "status",
    "access",
    "created_at",
    "updated_at",
    "tags",
    "aliases",
    "supersedes",
    "sources",
    "body",
];

fn parse_protocol_record(
    value: &Value,
    base: Option<&crate::Record>,
    default_scope: &str,
) -> Result<crate::Record, InvokeFailure> {
    let map = value
        .as_object()
        .ok_or_else(|| InvokeFailure::new("invalid_request", "record must be a JSON object"))?;
    ensure_keys(map, PROTOCOL_RECORD_FIELDS)?;
    let now = crate::Timestamp::now_utc();
    let id = match map.get("id").and_then(Value::as_str) {
        Some(value) => parse_protocol_id(value)?,
        None => base
            .map(|record| record.id)
            .unwrap_or_else(crate::RecordId::new_v7),
    };
    let title = protocol_string(map, "title", base.map(|record| record.title.as_str()))?;
    let kind = protocol_owned_string(map, "kind", base.map(|record| record.kind.to_string()))?
        .parse()
        .map_err(|_| InvokeFailure::new("invalid_request", "kind is invalid"))?;
    let scope = protocol_string(map, "scope", Some(default_scope))?
        .parse()
        .map_err(|_| InvokeFailure::new("invalid_request", "scope is invalid"))?;
    let status = protocol_owned_string(
        map,
        "status",
        base.map(|record| record.status.to_string())
            .or_else(|| Some("active".to_owned())),
    )?
    .parse()
    .map_err(|_| InvokeFailure::new("invalid_request", "status is invalid"))?;
    let access =
        protocol_owned_string(map, "access", base.map(|record| record.access.to_string()))?
            .parse()
            .map_err(|_| InvokeFailure::new("invalid_request", "access is invalid"))?;
    let created_at =
        protocol_timestamp(map, "created_at", base.map(|record| record.created_at), now)?;
    let updated_at = protocol_timestamp(
        map,
        "updated_at",
        base.map(|record| record.updated_at),
        created_at,
    )?;
    let tags = protocol_strings(map, "tags", base.map(|record| record.tags.as_slice()))?;
    let aliases = protocol_strings(map, "aliases", base.map(|record| record.aliases.as_slice()))?;
    let supersedes = protocol_ids(
        map,
        "supersedes",
        base.map(|record| record.supersedes.as_slice()),
    )?;
    let sources = protocol_sources(map, base.map(|record| record.sources.as_slice()))?;
    let body = protocol_string(map, "body", base.map(|record| record.body.as_str()))?;
    Ok(crate::Record {
        id,
        title,
        kind,
        scope,
        status,
        access,
        created_at,
        updated_at,
        tags,
        aliases,
        supersedes,
        sources,
        body,
    })
}

fn protocol_string(
    map: &Map<String, Value>,
    key: &str,
    default: Option<&str>,
) -> Result<String, InvokeFailure> {
    match map.get(key) {
        Some(value) => value
            .as_str()
            .filter(|value| !value.is_empty() && value.len() <= MAX_INVOKE_OUTPUT_BODY)
            .map(str::to_owned)
            .ok_or_else(|| {
                InvokeFailure::new(
                    "invalid_request",
                    format!("field `{key}` must be a bounded string"),
                )
            }),
        None => default.map(str::to_owned).ok_or_else(|| {
            InvokeFailure::new("invalid_request", format!("field `{key}` is required"))
        }),
    }
}

fn protocol_owned_string(
    map: &Map<String, Value>,
    key: &str,
    default: Option<String>,
) -> Result<String, InvokeFailure> {
    match map.get(key) {
        Some(value) => value
            .as_str()
            .filter(|value| !value.is_empty() && value.len() <= MAX_INVOKE_OUTPUT_BODY)
            .map(str::to_owned)
            .ok_or_else(|| {
                InvokeFailure::new(
                    "invalid_request",
                    format!("field `{key}` must be a bounded string"),
                )
            }),
        None => default.ok_or_else(|| {
            InvokeFailure::new("invalid_request", format!("field `{key}` is required"))
        }),
    }
}

fn protocol_timestamp(
    map: &Map<String, Value>,
    key: &str,
    default: Option<crate::Timestamp>,
    fallback: crate::Timestamp,
) -> Result<crate::Timestamp, InvokeFailure> {
    match map.get(key) {
        Some(value) => value
            .as_str()
            .ok_or_else(|| {
                InvokeFailure::new(
                    "invalid_request",
                    format!("field `{key}` must be a timestamp"),
                )
            })?
            .parse()
            .map_err(|_| {
                InvokeFailure::new("invalid_request", format!("field `{key}` is not RFC 3339"))
            }),
        None => Ok(default.unwrap_or(fallback)),
    }
}

fn protocol_strings(
    map: &Map<String, Value>,
    key: &str,
    default: Option<&[String]>,
) -> Result<Vec<String>, InvokeFailure> {
    let Some(value) = map.get(key) else {
        return Ok(default.map_or_else(Vec::new, ToOwned::to_owned));
    };
    let values = value.as_array().ok_or_else(|| {
        InvokeFailure::new("invalid_request", format!("field `{key}` must be an array"))
    })?;
    if values.len() > 128 {
        return Err(InvokeFailure::new(
            "invalid_request",
            "record collection is too large",
        ));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty() && value.len() <= 512)
                .map(str::to_owned)
                .ok_or_else(|| {
                    InvokeFailure::new(
                        "invalid_request",
                        "record collection contains an invalid string",
                    )
                })
        })
        .collect()
}

fn protocol_ids(
    map: &Map<String, Value>,
    key: &str,
    default: Option<&[crate::RecordId]>,
) -> Result<Vec<crate::RecordId>, InvokeFailure> {
    let Some(value) = map.get(key) else {
        return Ok(default.map_or_else(Vec::new, ToOwned::to_owned));
    };
    let values = value.as_array().ok_or_else(|| {
        InvokeFailure::new("invalid_request", format!("field `{key}` must be an array"))
    })?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| {
                    InvokeFailure::new(
                        "invalid_request",
                        "record id collection contains an invalid value",
                    )
                })
                .and_then(parse_protocol_id)
        })
        .collect()
}

fn protocol_sources(
    map: &Map<String, Value>,
    default: Option<&[crate::Source]>,
) -> Result<Vec<crate::Source>, InvokeFailure> {
    let Some(value) = map.get("sources") else {
        return Ok(default.map_or_else(Vec::new, ToOwned::to_owned));
    };
    let values = value
        .as_array()
        .ok_or_else(|| InvokeFailure::new("invalid_request", "sources must be an array"))?;
    if values.len() > 32 {
        return Err(InvokeFailure::new("invalid_request", "too many sources"));
    }
    values
        .iter()
        .map(|value| {
            let source = value
                .as_object()
                .ok_or_else(|| InvokeFailure::new("invalid_request", "source must be an object"))?;
            ensure_keys(
                source,
                &[
                    "kind",
                    "reference",
                    "actor",
                    "observed_at",
                    "revision",
                    "content_hash",
                ],
            )?;
            let kind = source
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| InvokeFailure::new("invalid_request", "source kind is required"))?
                .parse()
                .map_err(|_| InvokeFailure::new("invalid_request", "source kind is invalid"))?;
            let reference = source
                .get("reference")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty() && value.len() <= 2048)
                .ok_or_else(|| {
                    InvokeFailure::new("invalid_request", "source reference is invalid")
                })?;
            let actor = source
                .get("actor")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty() && value.len() <= 256)
                .ok_or_else(|| InvokeFailure::new("invalid_request", "source actor is invalid"))?;
            let observed_at = source
                .get("observed_at")
                .map(|value| {
                    value
                        .as_str()
                        .ok_or_else(|| {
                            InvokeFailure::new("invalid_request", "source observed_at is invalid")
                        })?
                        .parse()
                        .map_err(|_| {
                            InvokeFailure::new(
                                "invalid_request",
                                "source observed_at must be an RFC 3339 timestamp",
                            )
                        })
                })
                .transpose()?;
            let revision = protocol_optional_source_string(source, "revision", 2048)?;
            let content_hash = protocol_optional_source_string(source, "content_hash", 256)?;
            Ok(crate::Source {
                kind,
                reference: reference.to_owned(),
                actor: actor.to_owned(),
                observed_at,
                revision,
                content_hash,
            })
        })
        .collect()
}

fn protocol_optional_source_string(
    source: &Map<String, Value>,
    key: &str,
    max_length: usize,
) -> Result<Option<String>, InvokeFailure> {
    source
        .get(key)
        .map(|value| {
            value
                .as_str()
                .filter(|value| !value.is_empty() && value.len() <= max_length)
                .map(str::to_owned)
                .ok_or_else(|| {
                    InvokeFailure::new("invalid_request", format!("source {key} is invalid"))
                })
        })
        .transpose()
}

pub fn invoke_search_result(result: &crate::SearchResult) -> Value {
    json!({
        "record_id": result.record_id,
        "chunk_id": result.chunk_id,
        "title": result.title,
        "kind": result.kind,
        "scope": result.scope,
        "status": result.status,
        "access": result.access,
        "excerpt": result.excerpt,
        "sources": result.sources,
        "score": result.score,
        "lexical_match_reason": result.lexical_match_reason,
        "match_reasons": result.match_reasons,
        "vector_distance": result.vector_distance
    })
}

pub fn invoke_record(record: &crate::Record) -> Value {
    json!({
        "id": record.id.to_string(),
        "title": record.title,
        "kind": record.kind.to_string(),
        "scope": record.scope.to_string(),
        "status": record.status.to_string(),
        "access": record.access.to_string(),
        "created_at": record.created_at.to_string(),
        "updated_at": record.updated_at.to_string(),
        "tags": record.tags,
        "aliases": record.aliases,
        "supersedes": record.supersedes.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "sources": record.sources.iter().map(invoke_source).collect::<Vec<_>>(),
        "body": record.body
    })
}

fn invoke_source(source: &crate::Source) -> Value {
    let mut value = Map::from_iter([
        ("kind".to_owned(), Value::String(source.kind.to_string())),
        (
            "reference".to_owned(),
            Value::String(source.reference.clone()),
        ),
        ("actor".to_owned(), Value::String(source.actor.clone())),
    ]);
    if let Some(observed_at) = source.observed_at {
        value.insert(
            "observed_at".to_owned(),
            Value::String(observed_at.to_string()),
        );
    }
    if let Some(revision) = &source.revision {
        value.insert("revision".to_owned(), Value::String(revision.clone()));
    }
    if let Some(content_hash) = &source.content_hash {
        value.insert(
            "content_hash".to_owned(),
            Value::String(content_hash.clone()),
        );
    }
    Value::Object(value)
}

fn map_core_error(error: &crate::Error) -> InvokeFailure {
    match error {
        crate::Error::Repository { source } => match source {
            crate::RepositoryError::StoreNotInitialized { .. } => {
                InvokeFailure::new("not_initialized", "the selected store is not initialized")
            }
            crate::RepositoryError::NotFound { .. } => {
                InvokeFailure::new("not_found", "record was not found")
            }
            crate::RepositoryError::ScopeDenied { .. } => {
                InvokeFailure::new("scope_denied", "record is outside the selected scope")
            }
            crate::RepositoryError::AccessDenied { .. } => InvokeFailure::new(
                "access_denied",
                "record is not available to this access class",
            ),
            crate::RepositoryError::MustBeCandidate { .. }
            | crate::RepositoryError::MustBeActive { .. }
            | crate::RepositoryError::MustBeArchived { .. }
            | crate::RepositoryError::MissingSupersededLink { .. } => InvokeFailure::new(
                "invalid_state",
                "record lifecycle state does not permit this operation",
            ),
            crate::RepositoryError::ConcurrentModification { .. }
            | crate::RepositoryError::MutationBusy { .. }
            | crate::RepositoryError::RecoveryConflict { .. } => InvokeFailure::new(
                "conflict",
                "record changed or is busy; retry after inspection",
            ),
            _ => InvokeFailure::new("internal_error", "the store operation failed"),
        },
        crate::Error::InvalidRecord { .. } => {
            InvokeFailure::new("invalid_record", "record validation failed")
        }
        crate::Error::InvalidInput { .. } => {
            InvokeFailure::new("invalid_request", "request input is invalid")
        }
        crate::Error::IndexUnavailable { .. } => InvokeFailure::new(
            "internal_error",
            "the SQLite projection could not be opened; check that its directory is writable, then reindex the selected store",
        ),
        crate::Error::IndexBusy => InvokeFailure::new(
            "conflict",
            "the SQLite projection is busy; retry the operation",
        ),
        crate::Error::Io { .. }
        | crate::Error::InvalidWorkingDirectory
        | crate::Error::MissingHomeDirectory
        | crate::Error::SharedStoreRequiresProject
        | crate::Error::InvalidStoreConfiguration { .. }
        | crate::Error::Index { .. }
        | crate::Error::Embedding { .. }
        | crate::Error::Backup { .. } => {
            InvokeFailure::new("internal_error", "the store operation failed")
        }
    }
}

pub fn invoke_scope_records(
    paths: &crate::StorePaths,
    scope: &str,
) -> Result<Value, InvokeFailure> {
    if scope.parse::<crate::Scope>().is_err() {
        return Err(InvokeFailure::new(
            "invalid_request",
            "resource scope is invalid",
        ));
    }
    let request = Map::from_iter([(String::from("scope"), Value::String(scope.to_owned()))]);
    let scopes = invocation_scope(paths, &request)?;
    let (stores, _) = invocation_stores(paths, &scopes, None, None)?;
    let mut records = Vec::new();
    for store in stores {
        for stored in crate::RecordRepository::new(store)
            .list(false)
            .map_err(|error| map_core_error(&error))?
        {
            let record = stored.record();
            if record.scope.as_str() == scope && record.access == crate::Access::Agent {
                records.push(invoke_record(record));
            }
        }
    }
    records.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    if records.len() > MAX_INVOKE_LIMIT {
        return Err(InvokeFailure::new(
            "output_too_large",
            "resource contains too many records",
        ));
    }
    Ok(Value::Array(records))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{invoke_record, parse_protocol_record};

    #[test]
    fn source_freshness_metadata_round_trips_through_the_json_protocol() {
        let cases = [
            json!({
                "kind": "document",
                "reference": "notes/design.md",
                "actor": "human",
                "observed_at": "2026-08-05T20:08:00Z",
                "revision": "git:9f2c11a",
                "content_hash": "blake3:4d8f1c"
            }),
            json!({
                "kind": "issue",
                "reference": "SB-706",
                "actor": "agent",
                "revision": "event:12"
            }),
            json!({
                "kind": "conversation",
                "reference": "session:current",
                "actor": "user"
            }),
        ];

        for source in cases {
            let value = json!({
                "title": "Source metadata",
                "kind": "fact",
                "scope": "global",
                "status": "active",
                "access": "agent",
                "sources": [source.clone()],
                "body": "Protocol round trip."
            });
            let record = parse_protocol_record(&value, None, "global");
            let record = record.expect("parse protocol record");
            assert_eq!(invoke_record(&record)["sources"][0], source);
        }
    }
}
