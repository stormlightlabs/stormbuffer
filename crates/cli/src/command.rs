use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};

#[derive(Clone, Debug, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Parser)]
#[command(
    name = "stormbuffer",
    version,
    about = "A local-first memory store for sourced facts, decisions, procedures, and checkpoints.",
    long_about = "Stormbuffer keeps durable memory in readable Markdown and exposes the same command shell through stormbuffer, stormbuf, and sbuf.",
    propagate_version = true,
    arg_required_else_help = true,
    subcommand_required = true
)]
pub struct Cli {
    /// Select when human-facing output uses ANSI color.
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,

    /// Use the nearest project store instead of the global store.
    #[arg(long, global = true)]
    pub project: bool,

    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Initialize a global or project store without changing existing metadata.
    Init,
    /// Print the resolved store root, whether or not it is initialized.
    Root,
    /// Inspect the resolved store and report its initialization state.
    Status(StatusArgs),
    /// Add a human-authored memory (not implemented in this milestone).
    Add(WriteStubArgs),
    /// Propose an agent memory candidate (not implemented in this milestone).
    Propose(WriteStubArgs),
    /// Approve a candidate memory (not implemented in this milestone).
    Approve(IdArgs),
    /// Reject a candidate memory (not implemented in this milestone).
    Reject(IdArgs),
    /// Edit a memory (not implemented in this milestone).
    Edit(IdArgs),
    /// Show one memory (not implemented in this milestone).
    Show(IdArgs),
    /// List memories (not implemented in this milestone).
    List(ListArgs),
    /// Search indexed memories (not implemented in this milestone).
    Search(QueryArgs),
    /// Compile bounded context from indexed memories (not implemented in this milestone).
    Context(QueryArgs),
    /// Supersede a memory (not implemented in this milestone).
    Supersede(IdArgs),
    /// Archive a memory (not implemented in this milestone).
    Archive(IdArgs),
    /// Restore an archived memory (not implemented in this milestone).
    Restore(IdArgs),
    /// Permanently delete a memory only with explicit --destroy (not implemented in this milestone).
    Forget(ForgetArgs),
    /// Reconcile canonical Markdown with the disposable index (not implemented in this milestone).
    Sync,
    /// Watch for canonical Markdown changes (not implemented in this milestone).
    Watch,
    /// Rebuild the disposable index (not implemented in this milestone).
    Reindex,
    /// Remove disposable cache data (not implemented in this milestone).
    Gc,
    /// Diagnose canonical data and projections (not implemented in this milestone).
    Doctor,
    /// Export canonical records (not implemented in this milestone).
    Export(PathArgs),
    /// Import canonical records (not implemented in this milestone).
    Import(PathArgs),
    /// Invoke the versioned JSON protocol (not implemented in this milestone).
    Invoke(InvokeArgs),
    /// Run the MCP adapter over stdio (not implemented in this milestone).
    Mcp(McpArgs),
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Emit one machine-readable JSON object instead of human-readable lines.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct WriteStubArgs {
    /// Optional title that a future write command will use.
    #[arg(long)]
    pub title: Option<String>,
    /// Optional memory kind that a future write command will validate.
    #[arg(long)]
    pub kind: Option<String>,
    /// Optional body that a future write command will store.
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
    /// Include inactive records when listing is implemented.
    #[arg(long)]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct QueryArgs {
    /// Search query.
    pub query: String,
}

#[derive(Args, Debug)]
pub struct ForgetArgs {
    /// Memory identifier.
    pub id: String,
    /// Explicitly acknowledge the permanent-deletion path.
    #[arg(long)]
    pub destroy: bool,
}

#[derive(Args, Debug)]
pub struct PathArgs {
    /// Import or export path.
    pub path: Option<String>,
}

#[derive(Args, Debug)]
pub struct InvokeArgs {
    /// Protocol operation name.
    pub operation: String,
}

#[derive(Args, Debug)]
pub struct McpArgs {
    /// Select the stdio transport.
    #[arg(long)]
    pub stdio: bool,
}

pub fn command(invoked_name: &str) -> clap::Command {
    let name = if invoked_name.is_empty() {
        "stormbuffer".to_owned()
    } else {
        invoked_name.to_owned()
    };
    Cli::command().name(name)
}
