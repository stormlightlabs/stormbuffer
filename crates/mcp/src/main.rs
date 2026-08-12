use std::process::ExitCode;

use stormbuffer_core::StoreScope;
use stormbuffer_mcp::{McpConfig, run_stdio_with_config};

fn main() -> ExitCode {
    let mut stdio = false;
    let mut scope = None;
    let mut config = McpConfig::default();
    for argument in std::env::args_os().skip(1) {
        match argument.to_str() {
            Some("--help" | "-h") => {
                println!("Usage: stormbuffer-mcp --stdio [--global | --project] [--allow-writes]");
                println!();
                println!("Run the Stormbuffer MCP JSON-RPC adapter over stdio.");
                println!("Writes are disabled unless --allow-writes is explicitly supplied.");
                return ExitCode::SUCCESS;
            }
            Some("--version" | "-V") => {
                println!("stormbuffer-mcp {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            Some("--stdio") => stdio = true,
            Some("--global") if scope == Some(StoreScope::Project) => {
                eprintln!("stormbuffer-mcp: --global conflicts with --project");
                return ExitCode::from(2);
            }
            Some("--global") => scope = Some(StoreScope::Global),
            Some("--project") if scope == Some(StoreScope::Global) => {
                eprintln!("stormbuffer-mcp: --project conflicts with --global");
                return ExitCode::from(2);
            }
            Some("--project") => scope = Some(StoreScope::Project),
            Some("--allow-writes") => config.allow_writes = true,
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
