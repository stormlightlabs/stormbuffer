import assert from "node:assert/strict";
import test from "node:test";
import {
	CAPTURE_MARKER,
	MAX_CONTEXT_CHARS,
	contextFromOutput,
} from "@stormlightlabs/codex-plugin-stormbuffer";
import { createLifecycle } from "../src/index.js";

function envelope(overrides = {}) {
	return JSON.stringify({
		version: 1,
		operation: "context",
		ok: true,
		result: {
			blocks: [{ record_id: "rec-1", text: "evidence" }],
			receipt: { receipt_id: "receipt-1" },
			...overrides,
		},
	});
}

function host(stdout = envelope(), code = 0) {
	const calls = { recall: [], messages: [] };
	return {
		calls,
		pi: {
			sendMessage(...args) {
				calls.messages.push(args);
			},
		},
		options: {
			async retrieveContext(invocation) {
				calls.recall.push(invocation);
				return code === 0 ? contextFromOutput(stdout) : null;
			},
		},
	};
}

test("recall uses shared behavior and returns hidden persistent context for every scope", async () => {
	for (const scope of ["global", "project", "local"]) {
		const { pi, calls, options } = host();
		const lifecycle = createLifecycle(pi, { ...options, scope });
		const result = await lifecycle.beforeAgentStart({ prompt: "current prompt" }, { cwd: "/work" });
		assert.equal(result.message.customType, "stormbuffer-context");
		assert.equal(result.message.display, false);
		assert.match(result.message.content, /receipt-1/);
		assert.equal(calls.recall.length, 1);
		assert.deepEqual(JSON.parse(calls.recall[0].input), { version: 1, query: "current prompt", budget: 512 });
		assert.deepEqual(calls.recall[0].args, scope === "global" ? ["invoke", "context"] : [`--${scope}`, "invoke", "context"]);
	}
});

test("empty, malformed, unavailable, and oversized results fail open for every scope", async () => {
	for (const scope of ["global", "project", "local"]) {
		for (const [stdout, code] of [
			[envelope({ blocks: [] }), 0],
			["not json", 0],
			["", 1],
			[envelope({ blocks: [{ record_id: "rec-1", text: "x".repeat(MAX_CONTEXT_CHARS) }] }), 0],
		]) {
			const { pi, options } = host(stdout, code);
			assert.equal(await createLifecycle(pi, { ...options, scope }).beforeAgentStart({ prompt: "prompt" }, {}), undefined);
		}
	}
});

test("agent_settled schedules one tagged capture follow-up", () => {
	const { pi, calls } = host();
	const lifecycle = createLifecycle(pi);
	lifecycle.agentSettled();
	lifecycle.agentSettled();
	assert.equal(calls.messages.length, 1);
	assert.equal(calls.messages[0][0].customType, "stormbuffer-capture-review");
	assert.match(calls.messages[0][0].content, /Submit nothing for routine completion/);
	assert.deepEqual(calls.messages[0][1], { triggerTurn: true, deliverAs: "followUp" });
});

test("capture turn neither recalls nor schedules itself", async () => {
	const { pi, calls } = host();
	const lifecycle = createLifecycle(pi);
	assert.equal(await lifecycle.beforeAgentStart({ prompt: `${CAPTURE_MARKER} internal` }, {}), undefined);
	lifecycle.agentSettled();
	assert.equal(calls.recall.length, 0);
	assert.equal(calls.messages.length, 0);
});

test("host failures do not block the turn", async () => {
	const { pi } = host();
	const unavailable = { retrieveContext: async () => { throw new Error("missing"); } };
	assert.equal(await createLifecycle(pi, unavailable).beforeAgentStart({ prompt: "prompt" }, {}), undefined);
	assert.equal(await createLifecycle(pi).beforeAgentStart({}, {}), undefined);
});
