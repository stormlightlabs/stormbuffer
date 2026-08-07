use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result as AnyhowResult, bail};

use clap::{CommandFactory, FromArgMatches};
use owo_colors::OwoColorize;
use serde_json::Value;
use stormbuffer_core::{self as core, ProposalActor, StoreInitMode, StoreScope};

mod command;

use command::{
    AddArgs, Cli, CliCommand, ColorMode, ContextArgs, EditArgs, ForgetArgs, GcArgs, IdArgs,
    ImportArgs, InvokeArgs, ListArgs, McpArgs, PathArgs, SearchArgs, SupersedeArgs, WatchArgs,
    WriteStubArgs,
};

const FAILURE: i32 = 1;

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

fn run_with_args<I>(args: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<OsString> = args.into_iter().collect();
    let parsed = match parse(args) {
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

fn parse(args: Vec<OsString>) -> Result<Cli, clap::Error> {
    let matches = Cli::command().try_get_matches_from(args)?;
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
        CliCommand::Mcp(arguments) => run_mcp(scope, arguments, &output),
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
        CliCommand::Gc(arguments) => run_gc(scope, arguments, &output),
        CliCommand::Export(arguments) => run_export(scope, arguments, &output),
        CliCommand::Import(arguments) => run_import(scope, arguments, &output),
    }
}

fn run_mcp(scope: StoreScope, arguments: McpArgs, output: &Output) -> i32 {
    if !arguments.stdio {
        output.error("mcp requires --stdio");
        return FAILURE;
    }
    let executable = std::env::var_os("STORMBUFFER_MCP_BIN")
        .unwrap_or_else(|| OsString::from("stormbuffer-mcp"));
    let mut command = Command::new(executable);
    command.arg("--stdio");
    if scope == StoreScope::Project {
        command.arg("--project");
    }
    if arguments.allow_writes {
        command.arg("--allow-writes");
    }
    match command
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
    {
        Ok(status) => status.code().unwrap_or(FAILURE),
        Err(error) => {
            output.error(&format!("could not start stormbuffer-mcp: {error}"));
            FAILURE
        }
    }
}

fn run_invoke(scope: StoreScope, arguments: InvokeArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(_) => {
            output.line(
                &serde_json::to_string(&core::invoke_envelope(
                    &arguments.operation,
                    Err(core::InvokeFailure::new(
                        "internal_error",
                        "could not resolve the selected store",
                    )),
                ))
                .unwrap_or_else(|_| {
                    "{\"version\":1,\"operation\":\"invoke\",\"ok\":false}".to_owned()
                }),
            );
            return FAILURE;
        }
    };
    let mut input = Vec::new();
    let result = match io::stdin()
        .take((core::MAX_INVOKE_INPUT + 1) as u64)
        .read_to_end(&mut input)
    {
        Ok(_) if input.len() <= core::MAX_INVOKE_INPUT => {
            core::invoke_request(&paths, &arguments.operation, &input)
        }
        Ok(_) => Err(core::InvokeFailure::new(
            "input_too_large",
            "request exceeds the bounded input limit",
        )),
        Err(_) => Err(core::InvokeFailure::new(
            "invalid_request",
            "could not read the JSON request",
        )),
    };
    let response = core::invoke_envelope(&arguments.operation, result);
    let mut encoded = serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"version":1,"operation":"invoke","ok":false,"error":{"code":"internal_error","message":"could not encode protocol response"}}"#.to_owned()
    });
    let response = if encoded.len().saturating_add(1) > core::MAX_INVOKE_OUTPUT {
        let bounded = core::invoke_envelope(
            &arguments.operation,
            Err(core::InvokeFailure::new(
                "output_too_large",
                "response exceeds the bounded protocol output",
            )),
        );
        encoded = serde_json::to_string(&bounded)
            .unwrap_or_else(|_| r#"{"version":1,"operation":"invoke","ok":false}"#.to_owned());
        bounded
    } else {
        response
    };
    output.line(&encoded);
    if response.get("ok") == Some(&Value::Bool(true)) {
        0
    } else {
        FAILURE
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
    for (index, result) in results.iter().enumerate() {
        if index > 0 {
            output.line("");
        }
        let source = result
            .sources
            .first()
            .map(|source| source.reference.as_str())
            .unwrap_or("");
        output.line(&format!(
            "{}\n  ID: {}\n  Kind: {}  Scope: {}\n  {}\n  Source: {}\n  Path: {}\n  Score: {:.4} ({})",
            human_text(&result.title),
            result.record_id,
            result.kind,
            result.scope,
            human_text(&result.excerpt),
            human_text(source),
            human_text(&result.path),
            result.score,
            human_text(&result.lexical_match_reason),
        ));
    }
    0
}

fn human_text(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| match character {
            '\n' | '\r' | '\t' | '\u{2028}' | '\u{2029}' => Some(' '),
            '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}' => None,
            character if character.is_control() => None,
            character => Some(character),
        })
        .collect()
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

fn run_export(scope: StoreScope, arguments: PathArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let bundle = match core::export_store(&paths).context("could not export canonical records") {
        Ok(bundle) => bundle,
        Err(error) => return report_error(error, output),
    };
    let encoded = match core::encode_export(&bundle) {
        Ok(encoded) => encoded,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    match arguments.path.as_deref() {
        None | Some("-") => {
            output.raw(encoded.as_bytes());
        }
        Some(path) => match core::write_export_archive(&paths, Path::new(path), encoded.as_bytes())
        {
            Ok(()) => output.line(&format!(
                "Exported {} records to {path}",
                bundle.records.len()
            )),
            Err(error) => {
                return report_error(
                    anyhow::Error::new(error).context("could not write export archive"),
                    output,
                );
            }
        },
    }
    0
}

fn run_import(scope: StoreScope, arguments: ImportArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let contents = if arguments.path == "-" {
        match read_import_archive(io::stdin()) {
            Ok(contents) => contents,
            Err(error) => {
                return report_error(
                    error.context("could not read import archive from stdin"),
                    output,
                );
            }
        }
    } else {
        match fs::File::open(&arguments.path)
            .map_err(anyhow::Error::new)
            .and_then(read_import_archive)
        {
            Ok(contents) => contents,
            Err(error) => {
                return report_error(error.context("could not read import archive"), output);
            }
        }
    };
    let bundle = match core::decode_export(&contents) {
        Ok(bundle) => bundle,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    let options = match import_options(&arguments) {
        Ok(options) => options,
        Err(error) => return report_error(error, output),
    };
    match core::import_store(&paths, &bundle, &options)
        .context("could not import canonical records")
    {
        Ok(report) => {
            output.line(&format!(
                "Imported: {}\nSkipped: {}\nOverwritten: {}\nRemapped: {}",
                report.imported, report.skipped, report.overwritten, report.remapped
            ));
            0
        }
        Err(error) => report_error(error, output),
    }
}

fn import_options(arguments: &ImportArgs) -> AnyhowResult<core::ImportOptions> {
    Ok(core::ImportOptions {
        id_collision: arguments
            .on_id
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(anyhow::Error::msg)?,
        scope_collision: arguments
            .on_scope
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(anyhow::Error::msg)?,
        existing_record: arguments
            .on_existing
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(anyhow::Error::msg)?,
    })
}

fn run_gc(scope: StoreScope, arguments: GcArgs, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let report = match core::gc_store(
        &paths,
        core::GcOptions {
            dry_run: arguments.dry_run,
        },
    )
    .context("could not collect disposable data")
    {
        Ok(report) => report,
        Err(error) => return report_error(error, output),
    };
    let action = if report.dry_run {
        "Reclaimable"
    } else {
        "Reclaimed"
    };
    output.line(&format!(
        "{action}: {} files, {} bytes",
        if report.dry_run {
            report.candidates.len()
        } else {
            report.removed
        },
        report.reclaimed_bytes
    ));
    for entry in report.candidates {
        output.line(&format!("{}\t{} bytes", entry.path, entry.bytes));
    }
    0
}

fn read_import_archive(reader: impl Read) -> AnyhowResult<String> {
    let limit = u64::try_from(core::MAX_EXPORT_ARCHIVE_BYTES).expect("archive limit fits in u64");
    let mut contents = String::new();
    reader
        .take(limit + 1)
        .read_to_string(&mut contents)
        .context("could not read the archive")?;
    if contents.len() > core::MAX_EXPORT_ARCHIVE_BYTES {
        bail!(
            "import archive exceeds the {} byte limit",
            core::MAX_EXPORT_ARCHIVE_BYTES
        );
    }
    Ok(contents)
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
            if report.is_complete() { 0 } else { FAILURE }
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
            if report.is_complete() { 0 } else { FAILURE }
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
            let mut complete = report.is_complete();
            output.line(&format!("Reindexed: {}", report.indexed));
            report_invalid_files(&report.invalid_files, output);
            if let Some(ref error) = model_error {
                output.error(&format!("semantic index unavailable: {error}"));
                complete = false;
            }
            if let Some(semantic) = report.semantic {
                if semantic.status == "unavailable" && model_error.is_none() {
                    output.error(&format!(
                        "semantic index unavailable: {}",
                        semantic
                            .message
                            .unwrap_or_else(|| "configure a verified model".to_owned())
                    ));
                    complete = false;
                } else if let Some(version) = semantic.model_version {
                    output.line(&format!("Semantic index: {} ({version})", semantic.status));
                }
            }
            if complete { 0 } else { FAILURE }
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
            report.is_complete()
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
    fn help_uses_the_public_binary_name() {
        let usage = Cli::command().render_help().to_string();
        assert!(usage.contains("Usage: sbuf"), "{usage}");
        assert!(usage.contains("init"));
        assert!(usage.contains("mcp"));
    }

    #[test]
    fn json_escape_handles_paths_safely() {
        assert_eq!(json_escape("a\"b\\c\n"), "a\\\"b\\\\c\\n");
    }
}
