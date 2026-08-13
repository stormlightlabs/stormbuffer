#!/usr/bin/env node
import {
	codexPromptOutput,
	codexStopOutput,
	contextInvocation,
	retrieveContext,
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

async function recall(event) {
	const invocation = contextInvocation(event?.prompt);
	if (!invocation) return null;
	return retrieveContext(invocation, {
		cwd: typeof event.cwd === "string" ? event.cwd : undefined,
		timeout: 5_000,
	});
}

const event = await readEvent();
let output = {};
try {
	if (process.argv[2] === "prompt" && event) {
		output = codexPromptOutput(await recall(event));
	} else if (process.argv[2] === "stop") {
		output = codexStopOutput(event);
	}
} catch {
	// Hook failures must not block the host.
}
process.stdout.write(`${JSON.stringify(output)}\n`);
