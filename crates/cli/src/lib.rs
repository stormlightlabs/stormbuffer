use std::ffi::OsString;

use anyhow::{Context, Result as AnyhowResult};
use clap::{CommandFactory, FromArgMatches};
use stormbuffer_core::{self as core, StoreScope};

mod backup;
mod command;
mod echo;
mod index;
mod protocol;
mod records;
mod skill;
mod store;

use command::*;
use echo::Echo;

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
    let output = Echo::new(parsed.color, machine);
    run_command(parsed, output)
}

fn parse(args: Vec<OsString>) -> Result<Cli, clap::Error> {
    let matches = Cli::command().try_get_matches_from(args)?;
    Cli::from_arg_matches(&matches)
}

fn run_command(cli: Cli, output: Echo) -> i32 {
    let scope = if cli.project {
        StoreScope::Project
    } else {
        StoreScope::Global
    };

    match cli.command {
        CliCommand::Init(arguments) => store::run_init(scope, arguments.shared, &output),
        CliCommand::Root => store::run_root(scope, &output),
        CliCommand::Status(arguments) => store::run_status(scope, arguments.json, &output),
        CliCommand::Add(arguments) => records::run_add(scope, arguments, &output),
        CliCommand::Edit(arguments) => records::run_edit(scope, arguments, &output),
        CliCommand::Show(arguments) => records::run_show(scope, arguments, &output),
        CliCommand::List(arguments) => records::run_list(scope, arguments, &output),
        CliCommand::Supersede(arguments) => records::run_supersede(scope, arguments, &output),
        CliCommand::Archive(arguments) => records::run_archive(scope, arguments, &output),
        CliCommand::Restore(arguments) => records::run_restore(scope, arguments, &output),
        CliCommand::Forget(arguments) => records::run_forget(scope, arguments, &output),
        CliCommand::Evaluate => index::run_evaluate(&output),
        CliCommand::Mcp(arguments) => protocol::run_mcp(scope, arguments, &output),
        CliCommand::Invoke(arguments) => protocol::run_invoke(scope, arguments, &output),
        CliCommand::Search(arguments) => index::run_search(scope, arguments, &output),
        CliCommand::Context(arguments) => index::run_context(scope, arguments, &output),
        CliCommand::Sync => index::run_sync(scope, &output),
        CliCommand::Watch(arguments) => index::run_watch(scope, arguments, &output),
        CliCommand::Reindex => index::run_reindex(scope, &output),
        CliCommand::Doctor => index::run_doctor(scope, &output),
        CliCommand::Propose(arguments) => records::run_propose(scope, arguments, &output),
        CliCommand::Approve(arguments) => records::run_approve(scope, arguments, &output),
        CliCommand::Reject(arguments) => records::run_reject(scope, arguments, &output),
        CliCommand::Gc(arguments) => index::run_gc(scope, arguments, &output),
        CliCommand::Export(arguments) => backup::run_export(scope, arguments, &output),
        CliCommand::Import(arguments) => backup::run_import(scope, arguments, &output),
        CliCommand::Skill(arguments) => match arguments.command {
            SkillCommand::Install(arguments) => skill::run_install(scope, arguments, &output),
        },
    }
}

fn resolve(scope: StoreScope) -> AnyhowResult<core::StorePaths> {
    let cwd = std::env::current_dir().context("could not determine current directory")?;
    core::resolve_store(scope, &cwd).context("could not resolve store")
}

fn report_error(error: anyhow::Error, output: &Echo) -> i32 {
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
