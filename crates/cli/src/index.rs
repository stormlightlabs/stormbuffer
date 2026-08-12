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
            output.line(&format!("Watch cycles: {}", report.cycles));
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

pub(super) fn run_doctor(scope: StoreScope, output: &Echo) -> i32 {
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
    scope: StoreScope,
    paths: core::StorePaths,
    output: &Echo,
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
