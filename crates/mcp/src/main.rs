use std::process::ExitCode;

use stormbuffer_core::StoreScope;
use stormbuffer_mcp::{McpConfig, McpWritePolicy, run_stdio_with_config};

fn main() -> ExitCode {
    let mut stdio = false;
    let mut scope = None;
    let mut config = McpConfig::default();
    for argument in std::env::args_os().skip(1) {
        match argument.to_str() {
            Some("--help" | "-h") => {
                println!(
                    "Usage: stormbuffer-mcp --stdio [--global | --project | --local] [--allow-candidate-writes | --allow-writes]"
                );
                println!();
                println!("Run the Stormbuffer MCP JSON-RPC adapter over stdio.");
                println!("Writes are disabled by default.");
                println!("--allow-candidate-writes enables remember and update only.");
                println!("--allow-writes additionally enables archival.");
                return ExitCode::SUCCESS;
            }
            Some("--version" | "-V") => {
                println!("stormbuffer-mcp {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            Some("--stdio") => stdio = true,
            Some("--global") if scope.is_some() => {
                eprintln!("stormbuffer-mcp: select only one of --global, --project, or --local");
                return ExitCode::from(2);
            }
            Some("--global") => scope = Some(StoreScope::Global),
            Some("--project") if scope.is_some() => {
                eprintln!("stormbuffer-mcp: select only one of --global, --project, or --local");
                return ExitCode::from(2);
            }
            Some("--project") => scope = Some(StoreScope::Project),
            Some("--local") if scope.is_some() => {
                eprintln!("stormbuffer-mcp: select only one of --global, --project, or --local");
                return ExitCode::from(2);
            }
            Some("--local") => scope = Some(StoreScope::Local),
            Some("--allow-candidate-writes") if config.write_policy != McpWritePolicy::ReadOnly => {
                eprintln!(
                    "stormbuffer-mcp: select only one of --allow-candidate-writes or --allow-writes"
                );
                return ExitCode::from(2);
            }
            Some("--allow-candidate-writes") => {
                config.write_policy = McpWritePolicy::CandidateOnly;
            }
            Some("--allow-writes") if config.write_policy != McpWritePolicy::ReadOnly => {
                eprintln!(
                    "stormbuffer-mcp: select only one of --allow-candidate-writes or --allow-writes"
                );
                return ExitCode::from(2);
            }
            Some("--allow-writes") => config.write_policy = McpWritePolicy::All,
            _ => {
                eprintln!("stormbuffer-mcp: unknown argument");
                return ExitCode::from(2);
            }
        }
    }

    if !stdio {
        eprintln!("stormbuffer-mcp: --stdio is required");
        return ExitCode::from(2);
    }

    if let Some(scope) = scope {
        config.scope = scope;
    }

    match run_stdio_with_config(config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("stormbuffer-mcp: could not start the stdio adapter");
            ExitCode::from(1)
        }
    }
}
