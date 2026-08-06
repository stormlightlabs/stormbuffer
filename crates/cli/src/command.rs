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

    /// Use the nearest project store instead of the global store.
    #[arg(long, global = true)]
    pub project: bool,

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
    /// Propose an agent memory candidate (not implemented).
    Propose(WriteStubArgs),
    /// Approve a candidate memory (not implemented).
    Approve(IdArgs),
    /// Reject a candidate memory (not implemented).
    Reject(IdArgs),
    /// Edit a memory.
    Edit(EditArgs),
    /// Show one memory.
    Show(IdArgs),
    /// List memories.
    List(ListArgs),
    /// Search indexed memories (not implemented).
    Search(QueryArgs),
    /// Compile bounded context from indexed memories (not implemented).
    Context(QueryArgs),
    /// Supersede a memory with a new active record.
    Supersede(SupersedeArgs),
    /// Archive a memory.
    Archive(IdArgs),
    /// Restore an archived memory.
    Restore(IdArgs),
    /// Permanently delete a memory only with explicit --destroy.
    Forget(ForgetArgs),
    /// Reconcile canonical Markdown with the disposable index (not implemented).
    Sync,
    /// Watch for canonical Markdown changes (not implemented).
    Watch,
    /// Rebuild the disposable index (not implemented).
    Reindex,
    /// Remove disposable cache data (not implemented).
    Gc,
    /// Diagnose canonical data and projections (not implemented).
    Doctor,
    /// Export canonical records (not implemented).
    Export(PathArgs),
    /// Import canonical records (not implemented).
    Import(PathArgs),
    /// Invoke the versioned JSON protocol (not implemented).
    Invoke(InvokeArgs),
    /// Run the MCP adapter over stdio (not implemented).
    Mcp(McpArgs),
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
    /// Skip the interactive confirmation prompt.
    #[arg(long, requires = "destroy")]
    pub yes: bool,
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

pub fn command_name(invoked_name: &str) -> clap::Command {
    let name = if invoked_name.is_empty() {
        "stormbuffer".to_owned()
    } else {
        invoked_name.to_owned()
    };
    Cli::command().name(name)
}
