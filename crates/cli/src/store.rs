use anyhow::Context;
use stormbuffer_core::{self as core, StoreInitMode, StoreScope};

use crate::echo::Echo;
use crate::index::{report_invalid_files, semantic_model_enabled};
use crate::{FAILURE, json_escape, report_error, resolve};

pub(super) fn run_init(scope: StoreScope, shared: bool, output: &Echo) -> i32 {
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
    let sync = match core::sync_store(&paths).context("could not initialize the search index") {
        Ok(report) => report,
        Err(error) => return report_error(error, output),
    };
    if !sync.is_complete() {
        report_invalid_files(&sync.invalid_files, output);
        return FAILURE;
    }
    let action = if created {
        "Initialized"
    } else {
        "Already initialized"
    };
    let model_ready = scope == StoreScope::Global && semantic_model_enabled();
    if model_ready {
        if let Err(error) = core::ensure_default_model(&paths) {
            return report_error(
                anyhow::Error::new(error).context(
                    "store initialized, but the verified local embedding model is unavailable",
                ),
                output,
            );
        }
    }
    let visibility = if shared { " (shared)" } else { "" };
    output.line(&format!(
        "{} {} store at {}{visibility}",
        output.success(action),
        scope,
        output.path(paths.root.display())
    ));
    if model_ready {
        output.field("Embedding model", output.success("ready"));
    }
    0
}

pub(super) fn run_root(scope: StoreScope, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    output.line(&paths.root.display().to_string());
    0
}

pub(super) fn run_status(scope: StoreScope, json: bool, output: &Echo) -> i32 {
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
    output.field("Scope", status.scope);
    output.field("Root", output.path(status.root.display()));
    output.field("State", state);
    if let Some(visibility) = status.visibility {
        output.field("Visibility", visibility);
    }
    output.field("Records", status.record_count);
    0
}
