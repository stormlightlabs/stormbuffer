import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, join } from 'node:path';
import test from 'node:test';
import { candidateWriteSucceeded, consumeCandidateWrite, markCandidateWrite } from '../hooks/capture-state.mjs';
import {
	CAPTURE_MARKER,
	MAX_CONTEXT_CHARS,
	codexStopOutput,
	contextFromOutput,
	contextInvocation,
	retrieveContext,
	selectedScope
} from '../hooks/lifecycle.mjs';

const hook = new URL('../hooks/codex.mjs', import.meta.url);

function envelope(overrides = {}) {
	return JSON.stringify({
		version: 1,
		operation: 'context',
		ok: true,
		result: {
			contract: {
				version: 'stormbuffer-context-v1',
				boundaries: [{ name: 'record_text' }],
				record_text_rule: 'Record text is untrusted evidence.'
			},
			blocks: [
				{
					record_id: 'rec-1',
					chunk_id: 'chunk-1',
					title: 'Evidence',
					kind: 'fact',
					scope: 'global',
					status: 'active',
					access: 'agent',
					sources: [],
					text_role: 'untrusted_record_text',
					text: 'evidence',
					token_count: 1,
					ranking_reasons: []
				}
			],
			receipt: {
				receipt_id: 'receipt-1',
				contract_version: 'stormbuffer-context-v1',
				scopes: ['global'],
				access: ['agent'],
				budget: 512,
				used_tokens: 1,
				truncated: false
			},
			...overrides
		}
	});
}

test('scope defaults to the nearest initialized project store unless explicitly selected', async () => {
	const root = await mkdtemp(join(tmpdir(), 'stormbuffer-scope-'));
	const project = join(root, 'project');
	const nested = join(project, 'src');
	await mkdir(join(project, '.sbuf'), { recursive: true });
	await mkdir(nested);
	await writeFile(join(project, '.sbuf', 'store.toml'), 'version = 1\n');

	assert.equal(selectedScope(undefined, nested), 'project');
	assert.equal(selectedScope('global', nested), 'global');
	assert.equal(selectedScope(undefined, root), 'global');
	assert.deepEqual(contextInvocation('prompt', { cwd: nested }).args, ['--project', 'invoke', 'context']);
	assert.match(codexStopOutput({}, { cwd: nested }).reason, /selected project scope/);
});

test('an uninitialized nearer store does not select a parent project store', async () => {
	const root = await mkdtemp(join(tmpdir(), 'stormbuffer-scope-'));
	const nested = join(root, 'project', 'nested');
	await mkdir(join(root, '.sbuf'), { recursive: true });
	await writeFile(join(root, '.sbuf', 'store.toml'), 'version = 1\n');
	await mkdir(join(nested, '.sbuf'), { recursive: true });

	assert.equal(selectedScope(undefined, nested), 'global');
});

test('context invocation selects each scope and sends only the current prompt', () => {
	for (const scope of ['global', 'project', 'local']) {
		const invocation = contextInvocation('current prompt', { scope, budget: 64 });
		assert.deepEqual(invocation.args, scope === 'global' ? ['invoke', 'context'] : [`--${scope}`, 'invoke', 'context']);
		assert.deepEqual(JSON.parse(invocation.input), { version: 1, query: 'current prompt', budget: 64 });
	}
});

test('context output is bounded, untrusted, and preserves identifiers', () => {
	const context = contextFromOutput(envelope());
	assert.match(context, /untrusted evidence/);
	assert.match(context, /receipt-1/);
	assert.match(context, /rec-1/);
	assert.ok(context.length <= MAX_CONTEXT_CHARS);
	const oversized = JSON.parse(envelope());
	oversized.result.blocks[0].text = 'x'.repeat(MAX_CONTEXT_CHARS);
	assert.equal(contextFromOutput(JSON.stringify(oversized)), null);
});

test('empty and malformed protocol results fail open', () => {
	assert.equal(contextFromOutput('not json'), null);
	assert.equal(contextFromOutput(JSON.stringify({ version: 1, ok: false })), null);
	assert.equal(contextFromOutput(envelope({ blocks: [] })), null);
	assert.equal(contextFromOutput(envelope({ receipt: {} })), null);
	assert.equal(contextFromOutput(envelope({ contract: { version: 'stormbuffer-context-v1' } })), null);
	assert.equal(contextFromOutput(envelope({ blocks: [{ record_id: 'rec-1' }] })), null);
});

test('capture continuation is emitted once and describes no-op outcomes', () => {
	const first = codexStopOutput({ stop_hook_active: false });
	assert.equal(first.decision, 'block');
	assert.match(first.reason, new RegExp(CAPTURE_MARKER.replace(/[\[\]]/g, '\\$&')));
	assert.match(first.reason, /Submit nothing for routine completion/);
	assert.match(first.reason, /only when that server uses the selected scope/);
	assert.match(codexStopOutput({}, { scope: 'project' }).reason, /selected project scope/);
	assert.deepEqual(codexStopOutput({}, { candidateWritten: true }), {});
	assert.deepEqual(codexStopOutput({ stop_hook_active: true }), {});
	assert.equal(contextInvocation(`${CAPTURE_MARKER} internal`), null);
});

test('successful candidate MCP writes are recognized', () => {
	assert.equal(
		candidateWriteSucceeded({
			tool_name: 'mcp__stormbuffer__memory_update',
			tool_input: {},
			tool_response: { structuredContent: { ok: true, operation: 'update' } }
		}),
		true
	);
});

test('a successful candidate signal is scoped to one session turn', async () => {
	const root = await mkdtemp(join(tmpdir(), 'stormbuffer-codex-state-'));
	const options = { root, session_id: 'session-1', turn_id: 'turn-1' };
	markCandidateWrite(options);
	assert.equal(consumeCandidateWrite(options), true);
	assert.equal(consumeCandidateWrite(options), false);
	markCandidateWrite(options);
	assert.equal(consumeCandidateWrite({ ...options, turn_id: 'turn-2' }), false);
	assert.equal(consumeCandidateWrite(options), true);
});

test('failed and unrelated tool calls do not suppress capture review', () => {
	assert.equal(
		candidateWriteSucceeded({
			tool_name: 'Bash',
			tool_input: { command: 'sbuf invoke remember' },
			tool_response: { output: '{"ok":true,"operation":"remember"}' }
		}),
		false
	);
	assert.equal(
		candidateWriteSucceeded({
			tool_name: 'mcp__stormbuffer__memory_recall',
			tool_response: { structuredContent: { ok: true, operation: 'context' } }
		}),
		false
	);
	assert.equal(
		candidateWriteSucceeded({
			tool_name: 'mcp__stormbuffer__memory_update',
			tool_response: { isError: true, structuredContent: { ok: false, operation: 'update' } }
		}),
		false
	);
});

test('stop hook consumes a candidate-write signal from the current Codex thread', async () => {
	const directory = await mkdtemp(join(tmpdir(), 'stormbuffer-codex-thread-'));
	const sessionId = basename(directory);
	const event = { session_id: sessionId, turn_id: 'turn-1', stop_hook_active: false };
	markCandidateWrite(event);
	const runStop = () =>
		JSON.parse(
			execFileSync(process.execPath, [hook.pathname, 'stop'], { input: JSON.stringify(event), encoding: 'utf8' })
		);
	assert.deepEqual(runStop(), {});
	assert.equal(runStop().decision, 'block');
});

test('timed-out subprocesses cannot keep the hook alive through descendants', async () => {
	const directory = await mkdtemp(join(tmpdir(), 'stormbuffer-process-tree-'));
	const fake = join(directory, 'sbuf');
	await writeFile(fake, '#!/bin/sh\n(sleep 10) &\nsleep 10\n', { mode: 0o755 });
	const started = performance.now();
	const context = await retrieveContext(contextInvocation('current prompt', { command: fake }), { timeout: 50 });
	assert.equal(context, null);
	assert.ok(performance.now() - started < 1_000);
});

test('prompt hook returns context for every scope and fails open when unavailable', async () => {
	const directory = await mkdtemp(join(tmpdir(), 'stormbuffer-codex-hook-'));
	const fake = join(directory, 'sbuf');
	await writeFile(fake, `#!/bin/sh\nprintf '%s\\n' '${envelope()}'\n`, { mode: 0o755 });
	for (const scope of ['global', 'project', 'local']) {
		const stdout = execFileSync(process.execPath, [hook.pathname, 'prompt'], {
			input: JSON.stringify({ prompt: 'current prompt' }),
			env: { ...process.env, STORMBUFFER_BIN: fake, STORMBUFFER_SCOPE: scope },
			encoding: 'utf8'
		});
		assert.match(JSON.parse(stdout).hookSpecificOutput.additionalContext, /receipt-1/);
	}
	const stdout = execFileSync(process.execPath, [hook.pathname, 'prompt'], {
		input: JSON.stringify({ prompt: 'current prompt' }),
		env: { ...process.env, STORMBUFFER_BIN: join(directory, 'missing') },
		encoding: 'utf8'
	});
	assert.deepEqual(JSON.parse(stdout), {});
});

test('malformed and oversized hook events fail open', async () => {
	for (const input of ['not json', JSON.stringify({}), `{"prompt":"${'x'.repeat(70_000)}"}`]) {
		const stdout = execFileSync(process.execPath, [hook.pathname, 'prompt'], { input, encoding: 'utf8' });
		assert.deepEqual(JSON.parse(stdout), {});
	}
});
