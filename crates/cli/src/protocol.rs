use std::ffi::OsString;
use std::io::{self, Read};
use std::process::Command;

use serde_json::Value;
use stormbuffer_core::{self as core, StoreScope};

use crate::command::{InvokeArgs, McpArgs};
use crate::echo::Echo;
use crate::{FAILURE, resolve};

pub(super) fn run_mcp(scope: StoreScope, arguments: McpArgs, output: &Echo) -> i32 {
    if !arguments.stdio {
        output.error("mcp requires --stdio");
        return FAILURE;
    }
    let executable = std::env::var_os("STORMBUFFER_MCP_BIN")
        .unwrap_or_else(|| OsString::from("stormbuffer-mcp"));
    let mut command = Command::new(executable);
    command.arg("--stdio");
    match scope {
        StoreScope::Global => {}
        StoreScope::Project => {
            command.arg("--project");
        }
        StoreScope::Local => {
            command.arg("--local");
        }
    }
    if arguments.allow_writes {
        command.arg("--allow-writes");
    }
    match command
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
    {
        Ok(status) => status.code().unwrap_or(FAILURE),
        Err(error) => {
            output.error(&format!("could not start stormbuffer-mcp: {error}"));
            FAILURE
        }
    }
}

pub(super) fn run_invoke(scope: StoreScope, arguments: InvokeArgs, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(_) => {
            output.line(
                &serde_json::to_string(&core::invoke_envelope(
                    &arguments.operation,
                    Err(core::InvokeFailure::new(
                        "internal_error",
                        "could not resolve the selected store",
                    )),
                ))
                .unwrap_or_else(|_| {
                    "{\"version\":1,\"operation\":\"invoke\",\"ok\":false}".to_owned()
                }),
            );
            return FAILURE;
        }
    };
    let mut input = Vec::new();
    let result = match io::stdin()
        .take((core::MAX_INVOKE_INPUT + 1) as u64)
        .read_to_end(&mut input)
    {
        Ok(_) if input.len() <= core::MAX_INVOKE_INPUT => {
            core::invoke_request(&paths, &arguments.operation, &input)
        }
        Ok(_) => Err(core::InvokeFailure::new(
            "input_too_large",
            "request exceeds the bounded input limit",
        )),
        Err(_) => Err(core::InvokeFailure::new(
            "invalid_request",
            "could not read the JSON request",
        )),
    };
    let response = core::invoke_envelope(&arguments.operation, result);
    let mut encoded = serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"version":1,"operation":"invoke","ok":false,"error":{"code":"internal_error","message":"could not encode protocol response"}}"#.to_owned()
    });
    let response = if encoded.len().saturating_add(1) > core::MAX_INVOKE_OUTPUT {
        let bounded = core::invoke_envelope(
            &arguments.operation,
            Err(core::InvokeFailure::new(
                "output_too_large",
                "response exceeds the bounded protocol output",
            )),
        );
        encoded = serde_json::to_string(&bounded)
            .unwrap_or_else(|_| r#"{"version":1,"operation":"invoke","ok":false}"#.to_owned());
        bounded
    } else {
        response
    };
    output.line(&encoded);
    if response.get("ok") == Some(&Value::Bool(true)) {
        0
    } else {
        FAILURE
    }
}
