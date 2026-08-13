import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import {
	CAPTURE_MARKER,
	MAX_CONTEXT_CHARS,
	codexStopOutput,
	contextFromOutput,
	contextInvocation,
} from "../hooks/lifecycle.mjs";

const hook = new URL("../hooks/codex.mjs", import.meta.url);

function envelope(overrides = {}) {
	return JSON.stringify({
		version: 1,
		operation: "context",
		ok: true,
		result: {
			contract: { version: "1" },
			blocks: [{ record_id: "rec-1", text: "evidence" }],
			receipt: { receipt_id: "receipt-1" },
			...overrides,
		},
	});
}

test("context invocation selects each scope and sends only the current prompt", () => {
	for (const scope of ["global", "project", "local"]) {
		const invocation = contextInvocation("current prompt", { scope, budget: 64 });
		assert.deepEqual(invocation.args, scope === "global" ? ["invoke", "context"] : [`--${scope}`, "invoke", "context"]);
		assert.deepEqual(JSON.parse(invocation.input), { version: 1, query: "current prompt", budget: 64 });
	}
});

test("context output is bounded, untrusted, and preserves identifiers", () => {
	const context = contextFromOutput(envelope());
	assert.match(context, /untrusted evidence/);
	assert.match(context, /receipt-1/);
	assert.match(context, /rec-1/);
	assert.ok(context.length <= MAX_CONTEXT_CHARS);
	assert.equal(contextFromOutput(envelope({ blocks: [{ record_id: "rec-1", text: "x".repeat(MAX_CONTEXT_CHARS) }] })), null);
});

test("empty and malformed protocol results fail open", () => {
	assert.equal(contextFromOutput("not json"), null);
	assert.equal(contextFromOutput(JSON.stringify({ version: 1, ok: false })), null);
	assert.equal(contextFromOutput(envelope({ blocks: [] })), null);
	assert.equal(contextFromOutput(envelope({ receipt: {} })), null);
});

test("capture continuation is emitted once and describes no-op outcomes", () => {
	const first = codexStopOutput({ stop_hook_active: false });
	assert.equal(first.decision, "block");
	assert.match(first.reason, new RegExp(CAPTURE_MARKER.replace(/[\[\]]/g, "\\$&")));
	assert.match(first.reason, /Submit nothing for routine completion/);
	assert.deepEqual(codexStopOutput({ stop_hook_active: true }), {});
	assert.equal(contextInvocation(`${CAPTURE_MARKER} internal`), null);
});

test("prompt hook returns context for every scope and fails open when unavailable", async () => {
	const directory = await mkdtemp(join(tmpdir(), "stormbuffer-codex-hook-"));
	const fake = join(directory, "sbuf");
	await writeFile(fake, `#!/bin/sh\nprintf '%s\\n' '${envelope()}'\n`, { mode: 0o755 });
	for (const scope of ["global", "project", "local"]) {
		const stdout = execFileSync(process.execPath, [hook.pathname, "prompt"], {
			input: JSON.stringify({ prompt: "current prompt" }),
			env: { ...process.env, STORMBUFFER_BIN: fake, STORMBUFFER_SCOPE: scope },
			encoding: "utf8",
		});
		assert.match(JSON.parse(stdout).hookSpecificOutput.additionalContext, /receipt-1/);
	}
	const stdout = execFileSync(process.execPath, [hook.pathname, "prompt"], {
		input: JSON.stringify({ prompt: "current prompt" }),
		env: { ...process.env, STORMBUFFER_BIN: join(directory, "missing") },
		encoding: "utf8",
	});
	assert.deepEqual(JSON.parse(stdout), {});
});

test("malformed and oversized hook events fail open", async () => {
	for (const input of ["not json", JSON.stringify({}), `{"prompt":"${"x".repeat(70_000)}"}`]) {
		const stdout = execFileSync(process.execPath, [hook.pathname, "prompt"], {
			input,
			encoding: "utf8",
		});
		assert.deepEqual(JSON.parse(stdout), {});
	}
});
