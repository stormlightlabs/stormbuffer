use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result as AnyhowResult, bail};

use clap::FromArgMatches;
use owo_colors::OwoColorize;
use serde_json::{Map, Value, json};
use stormbuffer_core::{self as core, ProposalActor, StoreInitMode, StoreScope};

mod command;

pub use command::{
    AddArgs, Cli, CliCommand, ColorMode, ContextArgs, EditArgs, ForgetArgs, IdArgs, InitArgs,
    InvokeArgs, ListArgs, McpArgs, PathArgs, SearchArgs, StatusArgs, SupersedeArgs, WatchArgs,
    WriteStubArgs, command_name,
};

pub const FAILURE: i32 = 1;

pub fn run() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(false)
        .try_init();

    let code = run_with_args(std::env::args_os());
    std::process::exit(code);
}

pub fn run_with_args<I>(args: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    let invoked_name = invoked_name(args.first());
    let parsed = match parse(args, &invoked_name) {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code();
            let _ = error.print();
            return code;
        }
    };

    let machine = matches!(&parsed.command, CliCommand::Status(arguments) if arguments.json)
        || matches!(&parsed.command, CliCommand::Search(arguments) if arguments.json)
        || matches!(&parsed.command, CliCommand::Context(_))
        || matches!(&parsed.command, CliCommand::Evaluate)
        || matches!(&parsed.command, CliCommand::Invoke(_));
    let output = Output::new(parsed.color.clone(), machine);
    run_command(parsed, output)
}

fn parse(args: Vec<OsString>, invoked_name: &str) -> Result<Cli, clap::Error> {
    let matches = command_name(invoked_name).try_get_matches_from(args)?;
    Cli::from_arg_matches(&matches)
}

fn run_command(cli: Cli, output: Output) -> i32 {
    let scope = if cli.project {
        StoreScope::Project
    } else {
        StoreScope::Global
    };

    match cli.command {
        CliCommand::Init(arguments) => run_init(scope, arguments.shared, &output),
        CliCommand::Root => run_root(scope, &output),
        CliCommand::Status(arguments) => run_status(scope, arguments.json, &output),
        CliCommand::Add(arguments) => run_add(scope, arguments, &output),
        CliCommand::Edit(arguments) => run_edit(scope, arguments, &output),
        CliCommand::Show(arguments) => run_show(scope, arguments, &output),
        CliCommand::List(arguments) => run_list(scope, arguments, &output),
        CliCommand::Supersede(arguments) => run_supersede(scope, arguments, &output),
        CliCommand::Archive(arguments) => run_archive(scope, arguments, &output),
        CliCommand::Restore(arguments) => run_restore(scope, arguments, &output),
        CliCommand::Forget(arguments) => run_forget(scope, arguments, &output),
        CliCommand::Evaluate => run_evaluate(&output),
        CliCommand::Mcp(arguments) => {
            if !arguments.stdio {
                output.error("mcp currently requires --stdio; the adapter is not implemented yet");
                FAILURE
            } else {
                stub("mcp", &output)
            }
        }
        CliCommand::Invoke(arguments) => run_invoke(scope, arguments, &output),
        CliCommand::Search(arguments) => run_search(scope, arguments, &output),
        CliCommand::Context(arguments) => run_context(scope, arguments, &output),
        CliCommand::Sync => run_sync(scope, &output),
        CliCommand::Watch(arguments) => run_watch(scope, arguments, &output),
        CliCommand::Reindex => run_reindex(scope, &output),
        CliCommand::Doctor => run_doctor(scope, &output),
        CliCommand::Propose(arguments) => run_propose(scope, arguments, &output),
        CliCommand::Approve(arguments) => run_approve(scope, arguments, &output),
        CliCommand::Reject(arguments) => run_reject(scope, arguments, &output),
        CliCommand::Gc | CliCommand::Export(_) | CliCommand::Import(_) => {
            stub(command_as_str(&cli.command), &output)
        }
    }
}

const INVOKE_VERSION: u64 = 1;
const MAX_INVOKE_INPUT: usize = 256 * 1024;
const MAX_INVOKE_OUTPUT: usize = 256 * 1024;
const MAX_INVOKE_QUERY: usize = 2048;
const MAX_INVOKE_OUTPUT_BODY: usize = 64 * 1024;
const MAX_INVOKE_LIMIT: usize = 100;
const MAX_INVOKE_BUDGET: usize = 4096;

struct InvokeFailure {
    code: &'static str,
    message: String,
}

impl InvokeFailure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

fn run_invoke(scope: StoreScope, arguments: InvokeArgs, output: &Output) -> i32 {
    let mut input = Vec::new();
    let read_result = io::stdin()
        .take((MAX_INVOKE_INPUT + 1) as u64)
        .read_to_end(&mut input);
    let result = match read_result {
        Ok(_) if input.len() <= MAX_INVOKE_INPUT => {
            invoke_operation(scope, &arguments.operation, &input)
        }
        Ok(_) => Err(InvokeFailure::new(
            "input_too_large",
            "request exceeds the bounded input limit",
        )),
        Err(_) => Err(InvokeFailure::new(
            "invalid_request",
            "could not read the JSON request",
        )),
    };
    let mut response = match result {
        Ok(value) => json!({
            "version": INVOKE_VERSION,
            "operation": arguments.operation,
            "ok": true,
            "result": value,
        }),
        Err(error) => json!({
            "version": INVOKE_VERSION,
            "operation": arguments.operation,
            "ok": false,
            "error": {
                "code": error.code,
                "message": error.message,
            },
        }),
    };
    let mut encoded = serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"version":1,"operation":"invoke","ok":false,"error":{"code":"internal_error","message":"could not encode protocol response"}}"#.to_owned()
    });
    if encoded.len().saturating_add(1) > MAX_INVOKE_OUTPUT {
        response = json!({
            "version": INVOKE_VERSION,
            "operation": arguments.operation,
            "ok": false,
            "error": {
                "code": "output_too_large",
                "message": "response exceeds the bounded protocol output",
            },
        });
        encoded = serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"version":1,"operation":"invoke","ok":false,"error":{"code":"internal_error","message":"could not encode protocol response"}}"#.to_owned()
        });
    }
    output.line(&encoded);
    if response.get("ok") == Some(&Value::Bool(true)) {
        0
    } else {
        FAILURE
    }
}

fn invoke_operation(
    scope: StoreScope,
    operation: &str,
    input: &[u8],
) -> Result<Value, InvokeFailure> {
    let value: Value = serde_json::from_slice(input)
        .map_err(|_| InvokeFailure::new("invalid_json", "stdin must contain one JSON object"))?;
    let map = request_map(&value, operation)?;
    let paths = resolve(scope).map_err(|_| {
        InvokeFailure::new("internal_error", "could not resolve the selected store")
    })?;
    match operation {
        "search" => invoke_search(&paths, map),
        "context" => invoke_context(&paths, map),
        "get" => invoke_get(&paths, map),
        "propose" => invoke_propose(&paths, map),
        "supersede" => invoke_supersede(&paths, map),
        "archive" => invoke_archive(&paths, map),
        _ => Err(InvokeFailure::new(
            "unknown_operation",
            "operation is not supported by protocol version 1",
        )),
    }
}

fn request_map<'a>(
    value: &'a Value,
    operation: &str,
) -> Result<&'a Map<String, Value>, InvokeFailure> {
    let map = value
        .as_object()
        .ok_or_else(|| InvokeFailure::new("invalid_request", "request must be a JSON object"))?;
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
        "search" | "context" | "get" | "propose" | "supersede" | "archive"
    ) {
        return Err(InvokeFailure::new(
            "unknown_operation",
            "operation is not supported by protocol version 1",
        ));
    }
    Ok(map)
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
    let number = value.as_u64().ok_or_else(|| {
        InvokeFailure::new(
            "invalid_request",
            format!("field `{key}` must be an integer"),
        )
    })?;
    Ok((number as usize).clamp(1, maximum))
}

fn invocation_access(
    map: &Map<String, Value>,
) -> Result<(Vec<core::Access>, core::Access), InvokeFailure> {
    let caller = map
        .get("access")
        .and_then(Value::as_str)
        .unwrap_or("agent")
        .parse::<core::Access>()
        .map_err(|_| InvokeFailure::new("invalid_request", "access must be `agent` or `human`"))?;
    if caller == core::Access::Human {
        return Err(InvokeFailure::new(
            "permission_denied",
            "the invoke protocol is limited to agent-readable records",
        ));
    }
    Ok((vec![core::Access::Agent], core::Access::Agent))
}

fn invocation_scope(
    paths: &core::StorePaths,
    map: &Map<String, Value>,
) -> Result<Vec<String>, InvokeFailure> {
    let defaults = core::SearchOptions::for_store(paths)
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
        if scope.parse::<core::Scope>().is_err() || !defaults.iter().any(|allowed| allowed == scope)
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
    paths: &core::StorePaths,
    allowed_scopes: &[String],
) -> Result<Vec<core::StorePaths>, InvokeFailure> {
    let mut stores = vec![paths.clone()];
    if paths.scope == StoreScope::Project {
        let global = resolve(StoreScope::Global).map_err(|_| {
            InvokeFailure::new("internal_error", "could not resolve the global store")
        })?;
        if global.root.join("store.toml").is_file()
            && allowed_scopes.iter().any(|scope| scope == "global")
        {
            stores.push(global);
        }
    }
    for store in &stores {
        core::sync_store(store).map_err(|error| map_core_error(&error))?;
    }
    Ok(stores)
}

fn invoke_search(
    paths: &core::StorePaths,
    map: &Map<String, Value>,
) -> Result<Value, InvokeFailure> {
    ensure_keys(map, &["query", "limit", "scope", "scopes", "access"])?;
    let query = required_string(map, "query")?;
    if query.len() > MAX_INVOKE_QUERY {
        return Err(InvokeFailure::new("invalid_request", "query is too long"));
    }
    let limit = bounded_number(map, "limit", 20, MAX_INVOKE_LIMIT)?;
    let scopes = invocation_scope(paths, map)?;
    let (access, _) = invocation_access(map)?;
    let stores = invocation_stores(paths, &scopes)?;
    let mut options = core::SearchOptions::for_store(paths);
    options.limit = limit;
    options.allowed_scopes = Some(scopes);
    options.allowed_access = Some(access);
    options.mode = core::RetrievalMode::Lexical;
    let results =
        core::search_stores(&stores, query, options).map_err(|error| map_core_error(&error))?;
    Ok(Value::Array(
        results.iter().map(invoke_search_result).collect(),
    ))
}

fn invoke_context(
    paths: &core::StorePaths,
    map: &Map<String, Value>,
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
    let (access, _) = invocation_access(map)?;
    let stores = invocation_stores(paths, &scopes)?;
    let mut search = core::SearchOptions::for_store(paths);
    search.limit = limit;
    search.allowed_scopes = Some(scopes);
    search.allowed_access = Some(access);
    search.mode = core::RetrievalMode::Lexical;
    let result = core::context_stores(&stores, query, core::ContextOptions { budget, search })
        .map_err(|error| map_core_error(&error))?;
    serde_json::to_value(result)
        .map_err(|_| InvokeFailure::new("internal_error", "could not encode context result"))
}

fn invoke_get(paths: &core::StorePaths, map: &Map<String, Value>) -> Result<Value, InvokeFailure> {
    ensure_keys(map, &["id", "scope", "scopes", "access"])?;
    let id = parse_protocol_id(required_string(map, "id")?)?;
    let scopes = invocation_scope(paths, map)?;
    let (access, _) = invocation_access(map)?;
    let stores = invocation_stores(paths, &scopes)?;
    let mut denied = None;
    for store in stores {
        match core::RecordRepository::new(store).find_allowed(id, &scopes, &access) {
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
                error @ core::Error::Repository {
                    source: core::RepositoryError::ScopeDenied { .. },
                },
            )
            | Err(
                error @ core::Error::Repository {
                    source: core::RepositoryError::AccessDenied { .. },
                },
            ) => {
                denied = Some(map_core_error(&error));
            }
            Err(core::Error::Repository {
                source: core::RepositoryError::NotFound { .. },
            }) => {}
            Err(error) => return Err(map_core_error(&error)),
        }
    }
    denied.map_or_else(
        || Err(InvokeFailure::new("not_found", "record was not found")),
        Err,
    )
}

fn invoke_propose(
    paths: &core::StorePaths,
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
        .parse::<ProposalActor>()
        .map_err(|_| InvokeFailure::new("invalid_request", "actor must be `agent` or `human`"))?;
    if actor == ProposalActor::Human || map.contains_key("approved") {
        return Err(InvokeFailure::new(
            "permission_denied",
            "invoke proposals always require explicit human approval",
        ));
    }
    let scope = default_protocol_scope(paths);
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
    let result = core::RecordRepository::new(paths.clone())
        .propose(record, ProposalActor::Agent)
        .map_err(|error| map_core_error(&error))?;
    serde_json::to_value(result)
        .map_err(|_| InvokeFailure::new("internal_error", "could not encode proposal result"))
}

fn invoke_supersede(
    paths: &core::StorePaths,
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
    let (access, _) = invocation_access(map)?;
    let repository = core::RecordRepository::new(paths.clone());
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
    if replacement.id == target_id {
        replacement.id = core::RecordId::new_v7();
    }
    if !replacement.supersedes.contains(&target_id) {
        replacement.supersedes.push(target_id);
    }
    let stored = repository
        .supersede(target_id, replacement)
        .map_err(|error| map_core_error(&error))?;
    Ok(json!({
        "outcome": "accepted",
        "id": stored.record().id.to_string(),
        "status": stored.record().status.to_string(),
    }))
}

fn invoke_archive(
    paths: &core::StorePaths,
    map: &Map<String, Value>,
) -> Result<Value, InvokeFailure> {
    ensure_keys(map, &["id", "scope", "scopes", "access"])?;
    let id = parse_protocol_id(required_string(map, "id")?)?;
    let scopes = invocation_scope(paths, map)?;
    let (access, _) = invocation_access(map)?;
    let repository = core::RecordRepository::new(paths.clone());
    repository
        .find_allowed(id, &scopes, &access)
        .map_err(|error| map_core_error(&error))?;
    let stored = repository
        .archive(id)
        .map_err(|error| map_core_error(&error))?;
    Ok(json!({
        "outcome": "accepted",
        "id": stored.record().id.to_string(),
        "status": stored.record().status.to_string(),
    }))
}

fn parse_protocol_id(value: &str) -> Result<core::RecordId, InvokeFailure> {
    value
        .parse()
        .map_err(|_| InvokeFailure::new("invalid_request", "id must be a valid record identifier"))
}

fn default_protocol_scope(paths: &core::StorePaths) -> String {
    match paths.scope {
        StoreScope::Global => "global".to_owned(),
        StoreScope::Project => format!("project:{}", project_scope_name(paths)),
    }
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
    base: Option<&core::Record>,
    default_scope: &str,
) -> Result<core::Record, InvokeFailure> {
    let map = value
        .as_object()
        .ok_or_else(|| InvokeFailure::new("invalid_request", "record must be a JSON object"))?;
    ensure_keys(map, PROTOCOL_RECORD_FIELDS)?;
    let now = core::Timestamp::now_utc();
    let id = match map.get("id").and_then(Value::as_str) {
        Some(value) => parse_protocol_id(value)?,
        None => base
            .map(|record| record.id)
            .unwrap_or_else(core::RecordId::new_v7),
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
    Ok(core::Record {
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
    default: Option<core::Timestamp>,
    fallback: core::Timestamp,
) -> Result<core::Timestamp, InvokeFailure> {
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
    default: Option<&[core::RecordId]>,
) -> Result<Vec<core::RecordId>, InvokeFailure> {
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
    default: Option<&[core::Source]>,
) -> Result<Vec<core::Source>, InvokeFailure> {
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
            ensure_keys(source, &["kind", "reference", "actor"])?;
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
            Ok(core::Source {
                kind,
                reference: reference.to_owned(),
                actor: actor.to_owned(),
            })
        })
        .collect()
}

fn invoke_search_result(result: &core::SearchResult) -> Value {
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
        "vector_distance": result.vector_distance,
    })
}

fn invoke_record(record: &core::Record) -> Value {
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
        "sources": record.sources.iter().map(|source| json!({
            "kind": source.kind.to_string(),
            "reference": source.reference,
            "actor": source.actor,
        })).collect::<Vec<_>>(),
        "body": record.body,
    })
}

fn map_core_error(error: &core::Error) -> InvokeFailure {
    match error {
        core::Error::Repository { source } => match source {
            core::RepositoryError::StoreNotInitialized { .. } => {
                InvokeFailure::new("not_initialized", "the selected store is not initialized")
            }
            core::RepositoryError::NotFound { .. } => {
                InvokeFailure::new("not_found", "record was not found")
            }
            core::RepositoryError::ScopeDenied { .. } => {
                InvokeFailure::new("scope_denied", "record is outside the selected scope")
            }
            core::RepositoryError::AccessDenied { .. } => InvokeFailure::new(
                "access_denied",
                "record is not available to this access class",
            ),
            core::RepositoryError::MustBeCandidate { .. }
            | core::RepositoryError::MustBeActive { .. }
            | core::RepositoryError::MustBeArchived { .. }
            | core::RepositoryError::MissingSupersededLink { .. } => InvokeFailure::new(
                "invalid_state",
                "record lifecycle state does not permit this operation",
            ),
            core::RepositoryError::ConcurrentModification { .. }
            | core::RepositoryError::MutationBusy { .. }
            | core::RepositoryError::RecoveryConflict { .. } => InvokeFailure::new(
                "conflict",
                "record changed or is busy; retry after inspection",
            ),
            _ => InvokeFailure::new("internal_error", "the store operation failed"),
        },
        core::Error::InvalidRecord { .. } => {
            InvokeFailure::new("invalid_record", "record validation failed")
        }
        core::Error::InvalidInput { .. } => {
            InvokeFailure::new("invalid_request", "request input is invalid")
        }
        core::Error::Io { .. }
        | core::Error::InvalidWorkingDirectory
        | core::Error::MissingHomeDirectory
        | core::Error::SharedStoreRequiresProject
        | core::Error::InvalidStoreConfiguration { .. }
        | core::Error::Index { .. }
        | core::Error::Embedding { .. } => {
            InvokeFailure::new("internal_error", "the store operation failed")
        }
    }
}

fn run_evaluate(output: &Output) -> i32 {
    match core::run_evaluation() {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(value) => {
                output.line(&value);
                if report.passed { 0 } else { FAILURE }
            }
            Err(error) => report_error(anyhow::Error::new(error), output),
        },
        Err(error) => report_error(
            anyhow::Error::new(error).context("could not run retrieval evaluation"),
            output,
        ),
    }
}

fn run_search(scope: StoreScope, arguments: SearchArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let embedder = match configured_embedder() {
        Ok(embedder) => embedder,
        Err(error) => return report_error(error, output),
    };
    let mut options = core::SearchOptions::for_store(&paths);
    let stores = match prepare_retrieval_stores(scope, paths, output, embedder.as_deref()) {
        Some(stores) => stores,
        None => return FAILURE,
    };
    options.limit = arguments.limit;
    options.include_inactive = arguments.all;
    let results = match match embedder.as_deref() {
        Some(embedder) => {
            core::search_stores_with_embedder(&stores, &arguments.query, options, embedder)
        }
        None => core::search_stores(&stores, &arguments.query, options),
    } {
        Ok(results) => results,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    if arguments.json {
        return match serde_json::to_string_pretty(&results) {
            Ok(value) => {
                output.line(&value);
                0
            }
            Err(error) => report_error(anyhow::Error::new(error), output),
        };
    }
    for result in results {
        let source = result
            .sources
            .first()
            .map(|source| source.reference.as_str())
            .unwrap_or("");
        output.line(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            result.record_id,
            result.title,
            result.kind,
            result.scope,
            result.excerpt.replace('\n', " "),
            source,
            result.path,
            result.score,
            result.lexical_match_reason,
        ));
    }
    0
}

fn run_context(scope: StoreScope, arguments: ContextArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let embedder = match configured_embedder() {
        Ok(embedder) => embedder,
        Err(error) => return report_error(error, output),
    };
    let mut search = core::SearchOptions::for_store(&paths);
    let stores = match prepare_retrieval_stores(scope, paths, output, embedder.as_deref()) {
        Some(stores) => stores,
        None => return FAILURE,
    };
    search.limit = arguments.limit;
    search.include_inactive = arguments.all;
    let context_options = core::ContextOptions {
        budget: arguments.budget,
        search,
    };
    let result = match match embedder.as_deref() {
        Some(embedder) => {
            core::context_stores_with_embedder(&stores, &arguments.query, context_options, embedder)
        }
        None => core::context_stores(&stores, &arguments.query, context_options),
    } {
        Ok(result) => result,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    match serde_json::to_string_pretty(&result) {
        Ok(value) => {
            output.line(&value);
            0
        }
        Err(error) => report_error(anyhow::Error::new(error), output),
    }
}

fn run_sync(scope: StoreScope, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    match core::sync_store(&paths) {
        Ok(report) => {
            output.line(&format!(
                "Indexed: {}\nSkipped: {}\nRemoved: {}\nInvalid: {}",
                report.indexed,
                report.skipped,
                report.removed,
                report.invalid_files.len()
            ));
            report_invalid_files(&report.invalid_files, output);
            0
        }
        Err(error) => report_error(anyhow::Error::new(error), output),
    }
}

fn run_watch(scope: StoreScope, arguments: WatchArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let options = core::WatchOptions {
        once: arguments.once,
        interval: Duration::from_millis(arguments.interval_ms.max(50)),
    };
    match core::watch_store(&paths, options) {
        Ok(report) => {
            output.line(&format!("Watch cycles: {}", report.cycles));
            report_invalid_files(&report.invalid_files, output);
            0
        }
        Err(error) => report_error(anyhow::Error::new(error), output),
    }
}

fn run_reindex(scope: StoreScope, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let (embedder, model_error) = match configured_embedder() {
        Ok(embedder) => (embedder, None),
        Err(error) => (None, Some(error)),
    };
    match core::reindex_store_with_embedder(&paths, embedder.as_deref()) {
        Ok(report) => {
            output.line(&format!("Reindexed: {}", report.indexed));
            report_invalid_files(&report.invalid_files, output);
            if let Some(ref error) = model_error {
                output.error(&format!("semantic index unavailable: {error}"));
            }
            if let Some(semantic) = report.semantic {
                if semantic.status == "unavailable" && model_error.is_none() {
                    output.error(&format!(
                        "semantic index unavailable: {}",
                        semantic
                            .message
                            .unwrap_or_else(|| "configure a verified model".to_owned())
                    ));
                } else if let Some(version) = semantic.model_version {
                    output.line(&format!("Semantic index: {} ({version})", semantic.status));
                }
            }
            0
        }
        Err(error) => report_error(anyhow::Error::new(error), output),
    }
}

fn run_doctor(scope: StoreScope, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let report = match core::doctor_store(&paths) {
        Ok(report) => report,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    output.line(&format!("Index: {}", report.index_path));
    for issue in &report.issues {
        output.line(&format!(
            "{}: {} (repair: {})",
            issue.severity, issue.message, issue.repair
        ));
    }
    if report.failures == 0 { 0 } else { FAILURE }
}

fn reconcile(paths: &core::StorePaths, output: &Output) -> bool {
    match core::sync_store(paths) {
        Ok(report) => {
            report_invalid_files(&report.invalid_files, output);
            true
        }
        Err(error) => {
            report_error(anyhow::Error::new(error), output);
            false
        }
    }
}

fn prepare_retrieval_stores(
    scope: StoreScope,
    paths: core::StorePaths,
    output: &Output,
    embedder: Option<&dyn core::Embedder>,
) -> Option<Vec<core::StorePaths>> {
    let mut stores = vec![paths];
    if scope == StoreScope::Project {
        let global = match resolve(StoreScope::Global) {
            Ok(paths) => paths,
            Err(error) => {
                report_error(error, output);
                return None;
            }
        };
        if global.root.join("store.toml").is_file() {
            stores.push(global);
        }
    }
    if !stores.iter().all(|paths| reconcile(paths, output)) {
        return None;
    }
    if let Some(embedder) = embedder {
        for store in &stores {
            if let Err(error) = core::rebuild_vector_index(store, embedder) {
                report_error(
                    anyhow::Error::new(error).context("could not build semantic index"),
                    output,
                );
                return None;
            }
        }
    }
    Some(stores)
}

fn configured_embedder() -> AnyhowResult<Option<Box<dyn core::Embedder>>> {
    if !semantic_model_enabled() {
        return Ok(None);
    }
    let global = resolve(StoreScope::Global)?;
    core::ensure_default_model(&global)
        .context("could not acquire the verified local embedding model")?;
    let embedder = core::LocalEmbedder::from_default_cache(&global)
        .context("could not load the verified local embedding model")?;
    Ok(Some(Box::new(embedder)))
}

fn semantic_model_enabled() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("STORMBUFFER_TEST_MODE").is_none()
}

fn report_invalid_files(files: &[core::SyncInvalidFile], output: &Output) {
    for file in files {
        output.error(&format!(
            "invalid canonical record {}: {}",
            file.path, file.error
        ));
    }
}

fn run_init(scope: StoreScope, shared: bool, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let mode = if shared {
        StoreInitMode::Shared
    } else {
        StoreInitMode::Default
    };
    let created = match core::initialize_store(&paths, mode).context("could not initialize store") {
        Ok(created) => created,
        Err(error) => return report_error(error, output),
    };
    let action = if created {
        "Initialized"
    } else {
        "Already initialized"
    };
    if scope == StoreScope::Global && semantic_model_enabled() {
        if let Err(error) = core::ensure_default_model(&paths) {
            return report_error(
                anyhow::Error::new(error).context(
                    "store initialized, but the verified local embedding model is unavailable",
                ),
                output,
            );
        }
    }
    let visibility = if shared {
        "shared"
    } else {
        "private by default"
    };
    output.line(&format!(
        "{} {} store at {} ({visibility})",
        output.success(action),
        scope,
        paths.root.display()
    ));
    0
}

fn run_root(scope: StoreScope, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    output.line(&paths.root.display().to_string());
    0
}

fn run_status(scope: StoreScope, json: bool, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let status = match core::inspect_store(&paths).context("could not inspect store") {
        Ok(status) => status,
        Err(error) => return report_error(error, output),
    };

    if json {
        let root = json_escape(&status.root.display().to_string());
        let visibility = status
            .visibility
            .map(|visibility| format!("\"{visibility}\""))
            .unwrap_or_else(|| "null".to_owned());
        output.line(&format!(
            "{{\"scope\":\"{}\",\"root\":\"{}\",\"initialized\":{},\"visibility\":{},\"record_count\":{}}}",
            status.scope, root, status.initialized, visibility, status.record_count
        ));
        return 0;
    }

    let state = if status.initialized {
        output.success("initialized")
    } else {
        output.warning("not initialized")
    };
    output.line(&format!("Scope: {}", status.scope));
    output.line(&format!("Root: {}", status.root.display()));
    output.line(&format!("State: {state}"));
    if let Some(visibility) = status.visibility {
        output.line(&format!("Visibility: {visibility}"));
    }
    output.line(&format!("Records: {}", status.record_count));
    0
}

fn run_add(scope: StoreScope, arguments: AddArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    if !paths.root.join("store.toml").is_file() {
        output.error("could not add record: store is not initialized");
        return FAILURE;
    }
    let repository = core::RecordRepository::new(paths.clone());
    let draft = match draft_record(
        &paths,
        scope,
        arguments.title,
        arguments.kind,
        arguments.body,
    ) {
        Ok(record) => record,
        Err(error) => return report_error(error, output),
    };
    let markdown = match core::render_markdown(&draft).context("could not prepare the new record") {
        Ok(markdown) => markdown,
        Err(error) => return report_error(error, output),
    };
    let edited = match edit_markdown(&markdown) {
        Ok(markdown) => markdown,
        Err(error) => return report_error(error, output),
    };
    let record = match parse_editor_record(&edited) {
        Ok(record) => record,
        Err(error) => return report_error(error, output),
    };
    if record.status != core::RecordStatus::Active {
        return report_error(
            anyhow::anyhow!("new records must have active status"),
            output,
        );
    }
    match repository.add(record) {
        Ok(stored) => {
            output.line(&stored.record().id.to_string());
            0
        }
        Err(error) => report_error(
            anyhow::Error::new(error).context("could not add record"),
            output,
        ),
    }
}

fn run_propose(scope: StoreScope, arguments: WriteStubArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    if !paths.root.join("store.toml").is_file() {
        output.error("could not propose record: store is not initialized");
        return FAILURE;
    }
    let repository = core::RecordRepository::new(paths.clone());
    let mut draft = match draft_record(
        &paths,
        scope,
        arguments.title,
        arguments.kind,
        arguments.body,
    ) {
        Ok(record) => record,
        Err(error) => return report_error(error, output),
    };
    draft.status = core::RecordStatus::Candidate;
    draft.access = core::Access::Agent;
    draft.sources = vec![core::Source {
        kind: core::SourceKind::Conversation,
        reference: "stormbuffer:cli/propose".to_owned(),
        actor: "agent".to_owned(),
    }];
    let markdown = match core::render_markdown(&draft).context("could not prepare the proposal") {
        Ok(markdown) => markdown,
        Err(error) => return report_error(error, output),
    };
    let edited = match edit_markdown(&markdown) {
        Ok(markdown) => markdown,
        Err(error) => return report_error(error, output),
    };
    let candidate = match parse_editor_record(&edited) {
        Ok(record) => record,
        Err(error) => return report_error(error, output),
    };
    match repository
        .propose(candidate, ProposalActor::Agent)
        .context("could not propose record")
    {
        Ok(result) => {
            output.line(&format!("{}\t{}", result.record_id, result.outcome));
            if let Some(message) = result.message {
                if result.outcome == core::ProposalOutcome::Invalid {
                    output.error(&message);
                    return FAILURE;
                }
            }
            0
        }
        Err(error) => report_error(error, output),
    }
}

fn run_approve(scope: StoreScope, arguments: IdArgs, output: &Output) -> i32 {
    run_candidate_decision(scope, arguments.id, true, output)
}

fn run_reject(scope: StoreScope, arguments: IdArgs, output: &Output) -> i32 {
    run_candidate_decision(scope, arguments.id, false, output)
}

fn run_candidate_decision(
    scope: StoreScope,
    raw_id: String,
    approve: bool,
    output: &Output,
) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths);
    let id = match parse_id(&raw_id) {
        Ok(id) => id,
        Err(error) => return report_error(error, output),
    };
    let result = if approve {
        repository.approve(id)
    } else {
        repository.reject(id)
    };
    match result.context("could not update candidate") {
        Ok(result) => {
            output.line(&format!(
                "{}\t{}\t{}",
                result.record_id,
                result.outcome,
                result.status.unwrap_or_default()
            ));
            0
        }
        Err(error) => report_error(error, output),
    }
}

fn run_edit(scope: StoreScope, arguments: EditArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths);
    let id = match parse_id(&arguments.id) {
        Ok(id) => id,
        Err(error) => return report_error(error, output),
    };
    let current = match repository.find(id).context("could not find record") {
        Ok(record) => record,
        Err(error) => return report_error(error, output),
    };
    let edited = match edit_markdown(current.markdown()) {
        Ok(markdown) => markdown,
        Err(error) => return report_error(error, output),
    };
    let mut replacement = match parse_editor_record(&edited) {
        Ok(record) => record,
        Err(error) => return report_error(error, output),
    };
    replacement.updated_at = core::Timestamp::now_utc();
    match repository
        .replace_if_unchanged(&current, replacement)
        .context("could not save edited record")
    {
        Ok(stored) => {
            output.line(&stored.record().id.to_string());
            0
        }
        Err(error) => report_error(error, output),
    }
}

fn run_show(scope: StoreScope, arguments: IdArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths);
    let id = match parse_id(&arguments.id) {
        Ok(id) => id,
        Err(error) => return report_error(error, output),
    };
    match repository.find(id).context("could not read record") {
        Ok(stored) => {
            output.raw(stored.markdown().as_bytes());
            0
        }
        Err(error) => report_error(error, output),
    }
}

fn run_list(scope: StoreScope, arguments: ListArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths);
    match repository
        .list(arguments.all)
        .context("could not list records")
    {
        Ok(records) => {
            for stored in records {
                let record = stored.record();
                output.line(&format!(
                    "{}\t{}\t{}\t{}\t{}",
                    record.id, record.status, record.kind, record.scope, record.title
                ));
            }
            0
        }
        Err(error) => report_error(error, output),
    }
}

fn run_supersede(scope: StoreScope, arguments: SupersedeArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths.clone());
    let old_id = match parse_id(&arguments.id) {
        Ok(id) => id,
        Err(error) => return report_error(error, output),
    };
    let old = match repository
        .find(old_id)
        .context("could not find record to supersede")
    {
        Ok(record) => record,
        Err(error) => return report_error(error, output),
    };
    let mut draft = old.record().clone();
    draft.id = core::RecordId::new_v7();
    draft.status = core::RecordStatus::Active;
    draft.created_at = core::Timestamp::now_utc();
    draft.updated_at = draft.created_at;
    draft.supersedes = vec![old_id];
    if let Some(title) = arguments.title {
        draft.title = title;
    }
    if let Some(kind) = arguments.kind {
        draft.kind = match kind
            .parse()
            .map_err(anyhow::Error::msg)
            .context("invalid replacement kind")
        {
            Ok(kind) => kind,
            Err(error) => return report_error(error, output),
        };
    }
    if let Some(body) = arguments.body {
        draft.body = body;
    }
    let markdown = match core::render_markdown(&draft).context("could not prepare replacement") {
        Ok(markdown) => markdown,
        Err(error) => return report_error(error, output),
    };
    let edited = match edit_markdown(&markdown) {
        Ok(markdown) => markdown,
        Err(error) => return report_error(error, output),
    };
    let replacement = match parse_editor_record(&edited) {
        Ok(record) => record,
        Err(error) => return report_error(error, output),
    };
    match repository
        .supersede(old_id, replacement)
        .context("could not supersede record")
    {
        Ok(stored) => {
            output.line(&stored.record().id.to_string());
            0
        }
        Err(error) => report_error(error, output),
    }
}

fn run_archive(scope: StoreScope, arguments: IdArgs, output: &Output) -> i32 {
    run_transition(scope, arguments.id, true, output)
}

fn run_restore(scope: StoreScope, arguments: IdArgs, output: &Output) -> i32 {
    run_transition(scope, arguments.id, false, output)
}

fn run_transition(scope: StoreScope, raw_id: String, archive: bool, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths);
    let id = match parse_id(&raw_id) {
        Ok(id) => id,
        Err(error) => return report_error(error, output),
    };
    let result = if archive {
        repository.archive(id)
    } else {
        repository.restore(id)
    };
    match result.context("could not change record lifecycle") {
        Ok(stored) => {
            output.line(&format!(
                "{}\t{}",
                stored.record().id,
                stored.record().status
            ));
            0
        }
        Err(error) => report_error(error, output),
    }
}

fn run_forget(scope: StoreScope, arguments: ForgetArgs, output: &Output) -> i32 {
    if !arguments.destroy {
        output.error("forget requires --destroy for permanent deletion");
        return FAILURE;
    }
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths);
    let id = match parse_id(&arguments.id) {
        Ok(id) => id,
        Err(error) => return report_error(error, output),
    };
    let stored = match repository.find(id).context("could not find record") {
        Ok(stored) => stored,
        Err(error) => return report_error(error, output),
    };
    if !arguments.yes {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() || !io::stderr().is_terminal()
        {
            output.error("noninteractive deletion requires --yes");
            return FAILURE;
        }
        let mut stderr = io::stderr().lock();
        let _ = write!(
            stderr,
            "Permanently delete {} ({})? [y/N] ",
            stored.record().title,
            id
        );
        let _ = stderr.flush();
        let mut answer = String::new();
        if let Err(error) = io::stdin().read_line(&mut answer) {
            return report_error(
                anyhow::Error::new(error).context("could not read confirmation"),
                output,
            );
        }
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            output.error("deletion cancelled");
            return FAILURE;
        }
    }
    match repository
        .forget(id, core::DestructionAcknowledgement::deliberate())
        .context("could not permanently delete record")
    {
        Ok(()) => {
            output.line(&format!("Forgot {id}"));
            0
        }
        Err(error) => report_error(error, output),
    }
}

fn draft_record(
    paths: &core::StorePaths,
    scope: StoreScope,
    title: Option<String>,
    kind: Option<String>,
    body: Option<String>,
) -> AnyhowResult<core::Record> {
    let now = core::Timestamp::now_utc();
    let scope_name = match scope {
        StoreScope::Global => "global".to_owned(),
        StoreScope::Project => format!("project:{}", project_scope_name(paths)),
    };
    Ok(core::Record {
        id: core::RecordId::new_v7(),
        title: title.unwrap_or_else(|| "Untitled memory".to_owned()),
        kind: kind
            .unwrap_or_else(|| "fact".to_owned())
            .parse()
            .map_err(anyhow::Error::msg)
            .context("invalid memory kind")?,
        scope: core::Scope::parse(&scope_name).map_err(anyhow::Error::msg)?,
        status: core::RecordStatus::Active,
        access: core::Access::Human,
        created_at: now,
        updated_at: now,
        tags: Vec::new(),
        aliases: Vec::new(),
        supersedes: Vec::new(),
        sources: vec![core::Source {
            kind: core::SourceKind::Conversation,
            reference: "stormbuffer:cli".to_owned(),
            actor: "human".to_owned(),
        }],
        body: body.unwrap_or_else(|| "Write the memory here.".to_owned()),
    })
}

fn project_scope_name(paths: &core::StorePaths) -> String {
    let name = paths
        .root
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or("local");
    let sanitized: String = name
        .chars()
        .map(|character| {
            if character.is_whitespace() || character == ':' || character.is_control() {
                '-'
            } else {
                character
            }
        })
        .collect();
    if sanitized.is_empty() {
        "local".to_owned()
    } else {
        sanitized
    }
}

fn parse_id(value: &str) -> AnyhowResult<core::RecordId> {
    value
        .parse()
        .map_err(|error: String| anyhow::Error::msg(error))
}

fn parse_editor_record(markdown: &str) -> AnyhowResult<core::Record> {
    core::parse_markdown(Path::new("<editor>"), markdown)
        .context("editor output is not a valid record")
}

fn edit_markdown(markdown: &str) -> AnyhowResult<String> {
    let path = editor_temp_path()?;
    let mut cleanup = EditorTemp::new(path.clone());
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .context("could not create editor file")?;
    file.write_all(markdown.as_bytes())
        .context("could not write editor file")?;
    file.sync_all().context("could not sync editor file")?;
    drop(file);

    let editor = std::env::var_os("VISUAL")
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var_os("EDITOR").filter(|value| !value.is_empty()))
        .ok_or_else(|| anyhow::anyhow!("set $VISUAL or $EDITOR to edit records"))?;
    let status = Command::new(editor)
        .arg(&path)
        .status()
        .context("could not start the record editor")?;
    if !status.success() {
        bail!("record editor exited unsuccessfully: {status}");
    }
    let edited = fs::read_to_string(&path).context("could not read editor output")?;
    cleanup.disarm();
    fs::remove_file(&path).context("could not remove editor file")?;
    Ok(edited)
}

fn editor_temp_path() -> AnyhowResult<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "stormbuffer-edit-{}-{stamp}.md",
        std::process::id()
    )))
}

struct EditorTemp {
    path: Option<PathBuf>,
}

impl EditorTemp {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for EditorTemp {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

fn resolve(scope: StoreScope) -> AnyhowResult<core::StorePaths> {
    let cwd = std::env::current_dir().context("could not determine current directory")?;
    core::resolve_store(scope, &cwd).context("could not resolve store")
}

fn report_error(error: anyhow::Error, output: &Output) -> i32 {
    output.error(&format!("{error:#}"));
    FAILURE
}

fn stub(name: &str, output: &Output) -> i32 {
    output.error(&format!(
        "{name} is not implemented yet; no data was changed"
    ));
    FAILURE
}

fn command_as_str(command: &CliCommand) -> &'static str {
    match command {
        CliCommand::Init(_) => "init",
        CliCommand::Root => "root",
        CliCommand::Status(_) => "status",
        CliCommand::Add(_) => "add",
        CliCommand::Propose(_) => "propose",
        CliCommand::Approve(_) => "approve",
        CliCommand::Reject(_) => "reject",
        CliCommand::Edit(_) => "edit",
        CliCommand::Show(_) => "show",
        CliCommand::List(_) => "list",
        CliCommand::Search(_) => "search",
        CliCommand::Context(_) => "context",
        CliCommand::Supersede(_) => "supersede",
        CliCommand::Archive(_) => "archive",
        CliCommand::Restore(_) => "restore",
        CliCommand::Forget(_) => "forget",
        CliCommand::Sync => "sync",
        CliCommand::Watch(_) => "watch",
        CliCommand::Reindex => "reindex",
        CliCommand::Gc => "gc",
        CliCommand::Doctor => "doctor",
        CliCommand::Export(_) => "export",
        CliCommand::Import(_) => "import",
        CliCommand::Invoke(_) => "invoke",
        CliCommand::Evaluate => "evaluate",
        CliCommand::Mcp(_) => "mcp",
    }
}

fn invoked_name(argument: Option<&OsString>) -> String {
    argument
        .and_then(|argument| Path::new(argument).file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.strip_suffix(".exe").unwrap_or(name).to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "stormbuffer".to_owned())
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

struct Output {
    colored: bool,
    machine: bool,
}

impl Output {
    fn new(mode: ColorMode, machine: bool) -> Self {
        let colored = !machine
            && match mode {
                ColorMode::Always => true,
                ColorMode::Never => false,
                ColorMode::Auto => {
                    std::env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal()
                }
            };
        Self { colored, machine }
    }

    fn line(&self, message: &str) {
        let _ = writeln!(io::stdout().lock(), "{message}");
    }

    fn raw(&self, bytes: &[u8]) {
        let _ = io::stdout().lock().write_all(bytes);
    }

    fn error(&self, message: &str) {
        let mut stderr = io::stderr().lock();
        let prefix = if self.colored && !self.machine {
            "error".red().bold().to_string()
        } else {
            "error".to_owned()
        };
        let _ = writeln!(stderr, "{prefix}: {message}");
    }

    fn success(&self, message: &str) -> String {
        if self.colored && !self.machine {
            message.green().bold().to_string()
        } else {
            message.to_owned()
        }
    }

    fn warning(&self, message: &str) -> String {
        if self.colored && !self.machine {
            message.yellow().bold().to_string()
        } else {
            message.to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_change_only_the_invoked_usage_name() {
        for name in ["stormbuffer", "stormbuf", "sbuf"] {
            let usage = command_name(name).render_help().to_string();
            assert!(usage.contains(&format!("Usage: {name}")), "{usage}");
            assert!(usage.contains("init"));
            assert!(usage.contains("mcp"));
        }
    }

    #[test]
    fn json_escape_handles_paths_safely() {
        assert_eq!(json_escape("a\"b\\c\n"), "a\\\"b\\\\c\\n");
    }

    #[test]
    fn invoked_name_uses_the_file_name() {
        assert_eq!(
            invoked_name(Some(&OsString::from("/tmp/stormbuf"))),
            "stormbuf"
        );
        assert_eq!(invoked_name(Some(&OsString::from("sbuf.exe"))), "sbuf");
        assert_eq!(invoked_name(Some(&OsString::new())), "stormbuffer");
    }
}
