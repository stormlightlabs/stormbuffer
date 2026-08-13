use std::time::Duration;

use anyhow::{Context, Result as AnyhowResult};
use stormbuffer_core::{self as core, StoreScope};

use crate::command::{ContextArgs, GcArgs, SearchArgs, WatchArgs};
use crate::echo::Echo;
use crate::{FAILURE, report_error, resolve};

pub(super) fn run_evaluate(output: &Echo) -> i32 {
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

pub(super) fn run_search(scope: StoreScope, arguments: SearchArgs, output: &Echo) -> i32 {
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
            "{}\n  {}: {}\n  {}: {}  {}: {}\n  {}\n  {}: {}\n  {}: {}\n  {}: {:.4} ({})",
            output.success(&human_text(&result.title)),
            output.label("ID"),
            result.record_id,
            output.label("Kind"),
            result.kind,
            output.label("Scope"),
            result.scope,
            human_text(&result.excerpt),
            output.label("Source"),
            human_text(source),
            output.label("Path"),
            output.path(human_text(&result.path)),
            output.label("Score"),
            result.score,
            human_text(&result.lexical_match_reason),
        ));
    }
    0
}

pub(super) fn run_context(scope: StoreScope, arguments: ContextArgs, output: &Echo) -> i32 {
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

pub(super) fn run_gc(scope: StoreScope, arguments: GcArgs, output: &Echo) -> i32 {
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
    output.field(
        action,
        format!(
            "{} files, {} bytes",
            if report.dry_run {
                report.candidates.len()
            } else {
                report.removed
            },
            report.reclaimed_bytes
        ),
    );
    for entry in report.candidates {
        output.line(&format!(
            "{}\t{} bytes",
            output.path(entry.path),
            entry.bytes
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

pub(super) fn run_sync(scope: StoreScope, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    match core::sync_store(&paths) {
        Ok(report) => {
            output.field("Indexed", report.indexed);
            output.field("Skipped", report.skipped);
            output.field("Removed", report.removed);
            output.field("Invalid", report.invalid_files.len());
            report_invalid_files(&report.invalid_files, output);
            if report.is_complete() { 0 } else { FAILURE }
        }
        Err(error) => report_error(anyhow::Error::new(error), output),
    }
}

pub(super) fn run_watch(scope: StoreScope, arguments: WatchArgs, output: &Echo) -> i32 {
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
            output.field("Watch cycles", report.cycles);
            report_invalid_files(&report.invalid_files, output);
            if report.is_complete() { 0 } else { FAILURE }
        }
        Err(error) => report_error(anyhow::Error::new(error), output),
    }
}

pub(super) fn run_reindex(scope: StoreScope, output: &Echo) -> i32 {
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
            output.field("Reindexed", report.indexed);
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
                    output.field("Semantic index", format!("{} ({version})", semantic.status));
                }
            }
            if complete { 0 } else { FAILURE }
        }
        Err(error) => report_error(anyhow::Error::new(error), output),
    }
}

pub(super) fn run_doctor(
    scope: StoreScope,
    arguments: crate::command::DoctorArgs,
    output: &Echo,
) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let (report, repaired) = match if arguments.repair {
        core::repair_store(&paths).map(|repair| (repair.diagnosis, repair.repaired))
    } else {
        core::doctor_store(&paths).map(|report| (report, Vec::new()))
    } {
        Ok(result) => result,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    output.field("Store", scope);
    output.field("Index", output.path(&report.index_path));
    let semantic_state = if report.semantic_model_ready {
        output.success("ready")
    } else {
        output.warning("lexical fallback")
    };
    output.field("Semantic retrieval", semantic_state);
    for action in repaired {
        output.line(&format!("{} {action}", output.success("Repaired:")));
    }
    if report.issues.is_empty() {
        output.field("Status", output.success("healthy"));
        return 0;
    }
    let status = format!(
        "{} failure(s), {} warning(s)",
        report.failures, report.warnings
    );
    let status = if report.failures == 0 {
        output.warning(&status)
    } else {
        output.failure(&status)
    };
    output.field("Status", status);
    output.line("");
    for issue in &report.issues {
        let severity = match issue.severity.as_str() {
            "failure" => output.failure("failure"),
            _ => output.warning("warning"),
        };
        output.line(&format!("{severity}: {}", issue.message));
        output.line(&format!("  {}: {}", output.label("Repair"), issue.repair));
    }
    if report.failures == 0 { 0 } else { FAILURE }
}

fn reconcile(paths: &core::StorePaths, output: &Echo) -> bool {
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
    _scope: StoreScope,
    paths: core::StorePaths,
    output: &Echo,
    embedder: Option<&dyn core::Embedder>,
) -> Option<Vec<core::StorePaths>> {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            report_error(anyhow::Error::new(error), output);
            return None;
        }
    };
    let stores = match core::retrieval_stores(&paths, &cwd) {
        Ok(stores) => stores,
        Err(error) => {
            report_error(anyhow::Error::new(error), output);
            return None;
        }
    };
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

pub(super) fn configured_embedder() -> AnyhowResult<Option<Box<dyn core::Embedder>>> {
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

pub(super) fn semantic_model_enabled() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("STORMBUFFER_TEST_MODE").is_none()
}

pub(super) fn report_invalid_files(files: &[core::SyncInvalidFile], output: &Echo) {
    for file in files {
        output.error(&format!(
            "invalid canonical record {}: {}",
            file.path, file.error
        ));
    }
}
