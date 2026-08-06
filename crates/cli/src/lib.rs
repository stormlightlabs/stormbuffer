use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

use clap::FromArgMatches;
use owo_colors::OwoColorize;
use stormbuffer_core::{self as core, StoreScope};

mod command;

pub use command::{
    Cli, CliCommand, ColorMode, ForgetArgs, IdArgs, InvokeArgs, ListArgs, McpArgs, PathArgs,
    QueryArgs, StatusArgs, WriteStubArgs, command,
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
        || matches!(&parsed.command, CliCommand::Invoke(_));
    let output = Output::new(parsed.color.clone(), machine);
    run_command(parsed, output)
}

fn parse(args: Vec<OsString>, invoked_name: &str) -> Result<Cli, clap::Error> {
    let matches = command(invoked_name).try_get_matches_from(args)?;
    Cli::from_arg_matches(&matches)
}

fn run_command(cli: Cli, output: Output) -> i32 {
    let scope = if cli.project {
        StoreScope::Project
    } else {
        StoreScope::Global
    };

    match cli.command {
        CliCommand::Init => run_init(scope, &output),
        CliCommand::Root => run_root(scope, &output),
        CliCommand::Status(arguments) => run_status(scope, arguments.json, &output),
        CliCommand::Forget(arguments) => {
            if !arguments.destroy {
                output.error(
                    "forget requires --destroy for permanent deletion; deletion is not implemented yet",
                );
                FAILURE
            } else {
                stub("forget", &output)
            }
        }
        CliCommand::Mcp(arguments) => {
            if !arguments.stdio {
                output.error("mcp currently requires --stdio; the adapter is not implemented yet");
                FAILURE
            } else {
                stub("mcp", &output)
            }
        }
        CliCommand::Invoke(_) => {
            let machine_output = Output::new(ColorMode::Never, true);
            stub("invoke", &machine_output)
        }
        CliCommand::Add(_)
        | CliCommand::Propose(_)
        | CliCommand::Approve(_)
        | CliCommand::Reject(_)
        | CliCommand::Edit(_)
        | CliCommand::Show(_)
        | CliCommand::List(_)
        | CliCommand::Search(_)
        | CliCommand::Context(_)
        | CliCommand::Supersede(_)
        | CliCommand::Archive(_)
        | CliCommand::Restore(_)
        | CliCommand::Sync
        | CliCommand::Watch
        | CliCommand::Reindex
        | CliCommand::Gc
        | CliCommand::Doctor
        | CliCommand::Export(_)
        | CliCommand::Import(_) => stub(command_name(&cli.command), &output),
    }
}

fn run_init(scope: StoreScope, output: &Output) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let created = match core::initialize_store(&paths) {
        Ok(created) => created,
        Err(error) => return report_error(error, output),
    };
    let action = if created {
        "Initialized"
    } else {
        "Already initialized"
    };
    output.line(&format!(
        "{} {} store at {}",
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
    let status = match core::inspect_store(&paths) {
        Ok(status) => status,
        Err(error) => return report_error(error, output),
    };

    if json {
        let root = json_escape(&status.root.display().to_string());
        output.line(&format!(
            "{{\"scope\":\"{}\",\"root\":\"{}\",\"initialized\":{},\"record_count\":{}}}",
            status.scope, root, status.initialized, status.record_count
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
    output.line(&format!("Records: {}", status.record_count));
    0
}

fn resolve(scope: StoreScope) -> core::Result<core::StorePaths> {
    let cwd = std::env::current_dir().map_err(|_| core::Error::InvalidWorkingDirectory)?;
    core::resolve_store(scope, &cwd)
}

fn report_error(error: core::Error, output: &Output) -> i32 {
    output.error(&error.to_string());
    FAILURE
}

fn stub(name: &str, output: &Output) -> i32 {
    output.error(&format!(
        "{name} is not implemented yet; no data was changed"
    ));
    FAILURE
}

fn command_name(command: &CliCommand) -> &'static str {
    match command {
        CliCommand::Init => "init",
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
        CliCommand::Watch => "watch",
        CliCommand::Reindex => "reindex",
        CliCommand::Gc => "gc",
        CliCommand::Doctor => "doctor",
        CliCommand::Export(_) => "export",
        CliCommand::Import(_) => "import",
        CliCommand::Invoke(_) => "invoke",
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
            let usage = command(name).render_help().to_string();
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
