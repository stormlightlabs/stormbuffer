use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result as AnyhowResult, bail};
use stormbuffer_core::{self as core, ProposalActor, StoreScope};

use crate::command::{AddArgs, EditArgs, ForgetArgs, IdArgs, ListArgs, SupersedeArgs, WriteStubArgs};
use crate::echo::Echo;
use crate::{FAILURE, report_error, resolve};

pub(super) fn run_add(scope: StoreScope, arguments: AddArgs, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    if !paths.root.join("store.toml").is_file() {
        output.error("could not add record: store is not initialized");
        return FAILURE;
    }
    let repository = core::RecordRepository::new(paths.clone());
    let draft = match draft_record(&paths, scope, arguments.title, arguments.kind, arguments.body) {
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
        return report_error(anyhow::anyhow!("new records must have active status"), output);
    }
    match repository.add(record) {
        Ok(stored) => {
            output.line(&stored.record().id.to_string());
            0
        }
        Err(error) => report_error(anyhow::Error::new(error).context("could not add record"), output),
    }
}

pub(super) fn run_propose(scope: StoreScope, arguments: WriteStubArgs, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    if !paths.root.join("store.toml").is_file() {
        output.error("could not propose record: store is not initialized");
        return FAILURE;
    }
    let repository = core::RecordRepository::new(paths.clone());
    let mut draft = match draft_record(&paths, scope, arguments.title, arguments.kind, arguments.body) {
        Ok(record) => record,
        Err(error) => return report_error(error, output),
    };
    draft.status = core::RecordStatus::Candidate;
    draft.access = core::Access::Agent;
    draft.sources = vec![core::Source {
        kind: core::SourceKind::Conversation,
        reference: "stormbuffer:cli/propose".to_owned(),
        actor: "agent".to_owned(),
        observed_at: None,
        revision: None,
        content_hash: None,
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

pub(super) fn run_approve(scope: StoreScope, arguments: IdArgs, output: &Echo) -> i32 {
    run_candidate_decision(scope, arguments.id, true, output)
}

pub(super) fn run_reject(scope: StoreScope, arguments: IdArgs, output: &Echo) -> i32 {
    run_candidate_decision(scope, arguments.id, false, output)
}

fn run_candidate_decision(scope: StoreScope, raw_id: String, approve: bool, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths);
    let id = match parse_id(&raw_id) {
        Ok(id) => id,
        Err(error) => return report_error(error, output),
    };
    let result = if approve { repository.approve(id) } else { repository.reject(id) };
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

pub(super) fn run_edit(scope: StoreScope, arguments: EditArgs, output: &Echo) -> i32 {
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

pub(super) fn run_show(scope: StoreScope, arguments: IdArgs, output: &Echo) -> i32 {
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

pub(super) fn run_list(scope: StoreScope, arguments: ListArgs, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths);
    match repository.list(arguments.all).context("could not list records") {
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

pub(super) fn run_supersede(scope: StoreScope, arguments: SupersedeArgs, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths.clone());
    let old_id = match parse_id(&arguments.id) {
        Ok(id) => id,
        Err(error) => return report_error(error, output),
    };
    let old = match repository.find(old_id).context("could not find record to supersede") {
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

pub(super) fn run_archive(scope: StoreScope, arguments: IdArgs, output: &Echo) -> i32 {
    run_transition(scope, arguments.id, true, output)
}

pub(super) fn run_restore(scope: StoreScope, arguments: IdArgs, output: &Echo) -> i32 {
    run_transition(scope, arguments.id, false, output)
}

fn run_transition(scope: StoreScope, raw_id: String, archive: bool, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let repository = core::RecordRepository::new(paths);
    let id = match parse_id(&raw_id) {
        Ok(id) => id,
        Err(error) => return report_error(error, output),
    };
    let result = if archive { repository.archive(id) } else { repository.restore(id) };
    match result.context("could not change record lifecycle") {
        Ok(stored) => {
            output.line(&format!("{}\t{}", stored.record().id, stored.record().status));
            0
        }
        Err(error) => report_error(error, output),
    }
}

pub(super) fn run_forget(scope: StoreScope, arguments: ForgetArgs, output: &Echo) -> i32 {
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
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() || !io::stderr().is_terminal() {
            output.error("noninteractive deletion requires --yes");
            return FAILURE;
        }
        let mut stderr = io::stderr().lock();
        let _ = write!(stderr, "Permanently delete {} ({})? [y/N] ", stored.record().title, id);
        let _ = stderr.flush();
        let mut answer = String::new();
        if let Err(error) = io::stdin().read_line(&mut answer) {
            return report_error(anyhow::Error::new(error).context("could not read confirmation"), output);
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
    paths: &core::StorePaths, _scope: StoreScope, title: Option<String>, kind: Option<String>, body: Option<String>,
) -> AnyhowResult<core::Record> {
    let now = core::Timestamp::now_utc();
    let record_scope = core::record_scope(paths).context("could not read the store identity")?;
    Ok(core::Record {
        id: core::RecordId::new_v7(),
        title: title.unwrap_or_else(|| "Untitled memory".to_owned()),
        kind: kind
            .unwrap_or_else(|| "fact".to_owned())
            .parse()
            .map_err(anyhow::Error::msg)
            .context("invalid memory kind")?,
        scope: record_scope,
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
            observed_at: None,
            revision: None,
            content_hash: None,
        }],
        body: body.unwrap_or_else(|| "Write the memory here.".to_owned()),
    })
}

fn parse_id(value: &str) -> AnyhowResult<core::RecordId> {
    value.parse().map_err(|error: String| anyhow::Error::msg(error))
}

fn parse_editor_record(markdown: &str) -> AnyhowResult<core::Record> {
    core::parse_markdown(Path::new("<editor>"), markdown).context("editor output is not a valid record")
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
    Ok(std::env::temp_dir().join(format!("stormbuffer-edit-{}-{stamp}.md", std::process::id())))
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
