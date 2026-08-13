#!/usr/bin/env node
import { spawn } from "node:child_process";
import { scopeArgs, selectedScope } from "./lifecycle.mjs";

const command = process.env.STORMBUFFER_BIN || "sbuf";
const child = spawn(command, [...scopeArgs(selectedScope()), "mcp", "--stdio"], {
	stdio: "inherit",
});

for (const signal of ["SIGINT", "SIGTERM"]) {
	process.on(signal, () => child.kill(signal));
}

child.on("error", () => process.exit(1));
child.on("exit", (code, signal) => {
	if (signal) process.kill(process.pid, signal);
	else process.exit(code ?? 1);
});
