use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Parser)]
#[command(
    name = "sbuf",
    version,
    about = "A memory store for facts, decisions, procedures, and checkpoints.",
    long_about = "Stormbuffer keeps memories in readable, indexed Markdown.",
    propagate_version = true,
    arg_required_else_help = true,
    subcommand_required = true
)]
pub struct Cli {
    /// Select when human-facing output uses ANSI color.
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,

    /// Use the nearest project store together with applicable global memories.
    #[arg(long, global = true, conflicts_with = "local")]
    pub project: bool,

    /// Use only the nearest project store.
    #[arg(long, global = true, conflicts_with_all = ["project", "global"])]
    pub local: bool,

    /// Select the global store explicitly.
    #[arg(long, global = true, conflicts_with_all = ["project", "local"])]
    pub global: bool,

    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Initialize a global or project store without changing existing metadata.
    Init(InitArgs),
    /// Print the resolved store root, whether or not it is initialized.
    Root,
    /// Inspect the resolved store and report its initialization state.
    Status(StatusArgs),
    /// Add a human-authored memory.
    Add(AddArgs),
    /// Propose an agent memory candidate for review.
    Propose(WriteStubArgs),
    /// Approve a candidate memory.
    Approve(IdArgs),
    /// Reject a candidate memory by archiving it.
    Reject(IdArgs),
    /// Edit a memory.
    Edit(EditArgs),
    /// Show one memory.
    Show(IdArgs),
    /// List memories.
    List(ListArgs),
    /// Search indexed memories.
    Search(SearchArgs),
    /// Compile bounded context from indexed memories.
    Context(ContextArgs),
    /// Supersede a memory with a new active record.
    Supersede(SupersedeArgs),
    /// Archive a memory.
    Archive(IdArgs),
    /// Restore an archived memory.
    Restore(IdArgs),
    /// Permanently delete a memory only with explicit --destroy.
    Forget(ForgetArgs),
    /// Reconcile canonical Markdown with the disposable index.
    Sync,
    /// Watch for canonical Markdown changes.
    Watch(WatchArgs),
    /// Rebuild the disposable index.
    Reindex,
    /// Remove disposable cache, projection, lock, and temporary data.
    Gc(GcArgs),
    /// Diagnose canonical data and projections.
    Doctor(DoctorArgs),
    /// Export canonical records and provenance as a portable JSON archive.
    Export(PathArgs),
    /// Verify an export archive without importing it.
    VerifyExport(VerifyExportArgs),
    /// Import a portable canonical-record archive.
    Import(ImportArgs),
    /// Permanently remove the entire selected store.
    DestroyStore(DestroyStoreArgs),
    /// Review candidate memories awaiting a lifecycle decision.
    Inbox(InboxArgs),
    /// Audit memory health without changing canonical records.
    Audit(AuditArgs),
    /// Invoke a versioned, noninteractive JSON operation.
    Invoke(InvokeArgs),
    /// Run the checked-in retrieval evaluation corpus.
    Evaluate,
    /// Run the MCP adapter over stdio.
    Mcp(McpArgs),
    /// Manage agent skills shipped with Stormbuffer.
    Skill(SkillArgs),
}

#[derive(Args, Debug)]
pub struct SkillArgs {
    #[command(subcommand)]
    pub command: SkillCommand,
}

#[derive(Debug, Subcommand)]
pub enum SkillCommand {
    /// Install the selected scope's memory skill into an agent skill directory.
    Install(SkillInstallArgs),
}

#[derive(Args, Debug)]
pub struct SkillInstallArgs {
    /// Agent skill directory in which to create the Stormbuffer skill.
    #[arg(long, value_name = "DIRECTORY")]
    pub directory: PathBuf,

    /// Replace different existing skill content atomically.
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Make a project store shareable by tracking its configuration and canonical Markdown.
    #[arg(long)]
    pub shared: bool,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Emit one machine-readable JSON object instead of human-readable lines.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct WriteStubArgs {
    /// Optional candidate title.
    #[arg(long)]
    pub title: Option<String>,
    /// Optional candidate memory kind.
    #[arg(long)]
    pub kind: Option<String>,
    /// Optional candidate body.
    #[arg(long)]
    pub body: Option<String>,
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Optional initial title; the editor can change it.
    #[arg(long)]
    pub title: Option<String>,
    /// Optional initial memory kind.
    #[arg(long)]
    pub kind: Option<String>,
    /// Optional initial body; the editor can change it.
    #[arg(long)]
    pub body: Option<String>,
}

#[derive(Args, Debug)]
pub struct EditArgs {
    /// Memory identifier.
    pub id: String,
}

#[derive(Args, Debug)]
pub struct SupersedeArgs {
    /// Memory identifier to supersede.
    pub id: String,
    /// Optional initial replacement title.
    #[arg(long)]
    pub title: Option<String>,
    /// Optional initial replacement kind.
    #[arg(long)]
    pub kind: Option<String>,
    /// Optional initial replacement body.
    #[arg(long)]
    pub body: Option<String>,
}

#[derive(Args, Debug)]
pub struct IdArgs {
    /// Memory or candidate identifier.
    pub id: String,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Include inactive records.
    #[arg(long)]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Search query.
    pub query: String,
    /// Maximum number of chunks to return.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    /// Include candidate, superseded, and archived records.
    #[arg(long)]
    pub all: bool,
    /// Emit a JSON array instead of human-readable result cards.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ContextArgs {
    /// Search query.
    pub query: String,
    /// Maximum whitespace-delimited tokens in the evidence blocks.
    #[arg(long, default_value_t = 512)]
    pub budget: usize,
    /// Maximum number of matching chunks considered.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    /// Include candidate, superseded, and archived records.
    #[arg(long)]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct WatchArgs {
    /// Run one reconciliation cycle and exit.
    #[arg(long)]
    pub once: bool,
    /// Poll interval for canonical Markdown changes.
    #[arg(long, default_value_t = 500)]
    pub interval_ms: u64,
}

#[derive(Args, Debug)]
pub struct ForgetArgs {
    /// Memory identifier.
    pub id: String,
    /// Explicitly acknowledge the permanent-deletion path.
    #[arg(long)]
    pub destroy: bool,
    /// Skip the interactive confirmation prompt.
    #[arg(long, requires = "destroy")]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct PathArgs {
    /// Export path. Omit it or use `-` for stdout.
    pub path: Option<String>,
}

#[derive(Args, Debug)]
pub struct ImportArgs {
    /// Import path, or `-` to read stdin.
    pub path: String,
    /// Policy for records with an existing ID: fail, skip, overwrite, or remap.
    #[arg(long, visible_alias = "id-policy")]
    pub on_id: Option<String>,
    /// Policy for records outside the selected scope: fail, skip, or remap.
    #[arg(long, visible_alias = "scope-policy")]
    pub on_scope: Option<String>,
    /// Policy for an existing equivalent record: fail, skip, or overwrite.
    #[arg(long, visible_alias = "existing-policy")]
    pub on_existing: Option<String>,
    /// Report the resolved import actions without writing anything.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct VerifyExportArgs {
    /// Export path, or `-` to read stdin.
    pub path: String,
}

#[derive(Args, Debug)]
pub struct DestroyStoreArgs {
    /// Stable identity printed by the destruction preview.
    #[arg(long)]
    pub store_id: Option<String>,
    /// Export canonical records here before destruction.
    #[arg(long, value_name = "PATH")]
    pub export: Option<PathBuf>,
    /// Skip the interactive confirmation prompt.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct InboxArgs {
    /// Include candidates at least this many days old.
    #[arg(long)]
    pub min_age_days: Option<u64>,
    /// Filter by memory kind.
    #[arg(long)]
    pub kind: Option<String>,
    /// Filter by provenance source kind.
    #[arg(long)]
    pub source: Option<String>,
    /// Filter by exact record scope.
    #[arg(long)]
    pub scope: Option<String>,
    /// Show only candidates with a possible overlapping record.
    #[arg(long)]
    pub possible_overlap: bool,
    /// Emit a machine-readable JSON array.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct AuditArgs {
    /// Treat active checkpoints at least this many days old as stale.
    #[arg(long, default_value_t = 30)]
    pub stale_after_days: u64,
    /// Emit a machine-readable JSON object.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct GcArgs {
    /// Report disposable data without removing it.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Repair only disposable state with one unambiguous recovery path.
    #[arg(long)]
    pub repair: bool,
}

#[derive(Args, Debug)]
pub struct InvokeArgs {
    /// Version 1 operation: search, context, get, remember, update, propose, supersede, or archive.
    #[arg(value_name = "OPERATION")]
    pub operation: String,
}

#[derive(Args, Debug)]
pub struct McpArgs {
    /// Select the stdio transport.
    #[arg(long)]
    pub stdio: bool,

    /// Explicitly enable MCP write tools.
    #[arg(long)]
    pub allow_writes: bool,
}
