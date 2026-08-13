#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import {
	codexPromptOutput,
	codexStopOutput,
	contextFromOutput,
	contextInvocation,
} from "./lifecycle.mjs";

const MAX_EVENT_BYTES = 64 * 1024;

async function readEvent() {
	let input = "";
	for await (const chunk of process.stdin) {
		input += chunk;
		if (input.length > MAX_EVENT_BYTES) return null;
	}
	try {
		const event = JSON.parse(input);
		return event && typeof event === "object" && !Array.isArray(event) ? event : null;
	} catch {
		return null;
	}
}

function recall(event) {
	const invocation = contextInvocation(event?.prompt);
	if (!invocation) return null;
	const result = spawnSync(invocation.command, invocation.args, {
		cwd: typeof event.cwd === "string" ? event.cwd : undefined,
		input: invocation.input,
		encoding: "utf8",
		timeout: 5_000,
		maxBuffer: 262_144,
		stdio: ["pipe", "pipe", "ignore"],
	});
	return result.status === 0 ? contextFromOutput(result.stdout) : null;
}

const event = await readEvent();
let output = {};
try {
	if (process.argv[2] === "prompt" && event) {
		output = codexPromptOutput(recall(event));
	} else if (process.argv[2] === "stop") {
		output = codexStopOutput(event);
	}
} catch {
	// Hook failures must not block the host.
}
process.stdout.write(`${JSON.stringify(output)}\n`);
