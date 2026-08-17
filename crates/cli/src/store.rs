use anyhow::Context;
use stormbuffer_core::{self as core, StoreInitMode, StoreScope};

use crate::echo::Echo;
use crate::index::{report_invalid_files, semantic_model_enabled};
use crate::{FAILURE, report_error, resolve};

pub(super) fn run_init(scope: StoreScope, shared: bool, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let mode = if shared { StoreInitMode::Shared } else { StoreInitMode::Default };
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
    let action = if created { "Initialized" } else { "Already initialized" };
    let model_ready = scope == StoreScope::Global && semantic_model_enabled();
    if model_ready {
        if let Err(error) = core::ensure_default_model(&paths) {
            return report_error(
                anyhow::Error::new(error)
                    .context("store initialized, but the verified local embedding model is unavailable"),
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
        let project_id = status.project.as_ref().map(|project| project.id.to_string());
        let project_name = status.project.as_ref().map(|project| project.name.as_str());
        let value = serde_json::json!({
            "view": view_name(status.scope),
            "scope": status.scope.as_str(),
            "root": status.root,
            "initialized": status.initialized,
            "visibility": status.visibility.map(|value| value.as_str()),
            "project_id": project_id,
            "project_name": project_name,
            "record_count": status.record_count,
            "lifecycle": {
                "candidate": status.lifecycle.candidate,
                "active": status.lifecycle.active,
                "superseded": status.lifecycle.superseded,
                "archived": status.lifecycle.archived,
            },
            "disk_usage": {
                "canonical_bytes": status.canonical_bytes,
                "disposable_bytes": status.disposable_bytes,
            },
            "index_version": status.index_version,
            "embedding_version": status.embedding_version,
            "last_successful_sync": status.last_successful_sync,
        });
        output.line(&value.to_string());
        return 0;
    }

    let state = if status.initialized { output.success("initialized") } else { output.warning("not initialized") };
    output.field("View", view_name(status.scope));
    output.field("Scope", status.scope);
    output.field("Root", output.path(status.root.display()));
    output.field("State", state);
    if let Some(visibility) = status.visibility {
        output.field("Visibility", visibility);
    }
    if let Some(project) = status.project {
        output.field("Project ID", project.id);
        output.field("Project name", project.name);
    }
    output.field("Records", status.record_count);
    output.field("Candidates", status.lifecycle.candidate);
    output.field("Active", status.lifecycle.active);
    output.field("Superseded", status.lifecycle.superseded);
    output.field("Archived", status.lifecycle.archived);
    output.field("Canonical disk", format!("{} bytes", status.canonical_bytes));
    output.field("Disposable disk", format!("{} bytes", status.disposable_bytes));
    output.field(
        "Index version",
        status
            .index_version
            .map_or_else(|| "unavailable".to_owned(), |value| value.to_string()),
    );
    output.field(
        "Embedding version",
        status.embedding_version.as_deref().unwrap_or("unavailable"),
    );
    output.field("Last sync", status.last_successful_sync.as_deref().unwrap_or("never"));
    0
}

fn view_name(scope: StoreScope) -> &'static str {
    match scope {
        StoreScope::Global => "global",
        StoreScope::Project => "project with applicable global memory",
        StoreScope::Local => "strict local",
    }
}
