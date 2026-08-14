import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { CAPTURE_MARKER, MAX_CONTEXT_CHARS, contextFromOutput } from '@stormlightlabs/codex-plugin-stormbuffer';
import { createLifecycle } from '../src/index.js';

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

function host(stdout = envelope(), code = 0) {
	const calls = { recall: [], messages: [] };
	return {
		calls,
		pi: {
			sendMessage(...args) {
				calls.messages.push(args);
			}
		},
		options: {
			async retrieveContext(invocation) {
				calls.recall.push(invocation);
				return code === 0 ? contextFromOutput(stdout) : null;
			}
		}
	};
}

test('recall uses shared behavior and returns hidden persistent context for every scope', async () => {
	for (const scope of ['global', 'project', 'local']) {
		const { pi, calls, options } = host();
		const lifecycle = createLifecycle(pi, { ...options, scope });
		const result = await lifecycle.beforeAgentStart({ prompt: 'current prompt' }, { cwd: '/work' });
		assert.equal(result.message.customType, 'stormbuffer-context');
		assert.equal(result.message.display, false);
		assert.match(result.message.content, /receipt-1/);
		assert.equal(calls.recall.length, 1);
		assert.deepEqual(JSON.parse(calls.recall[0].input), { version: 1, query: 'current prompt', budget: 512 });
		assert.deepEqual(
			calls.recall[0].args,
			scope === 'global' ? ['invoke', 'context'] : [`--${scope}`, 'invoke', 'context']
		);
	}
});

test('empty, malformed, unavailable, and oversized results fail open for every scope', async () => {
	for (const scope of ['global', 'project', 'local']) {
		for (const [stdout, code] of [
			[envelope({ blocks: [] }), 0],
			['not json', 0],
			['', 1],
			[
				(() => {
					const result = JSON.parse(envelope());
					result.result.blocks[0].text = 'x'.repeat(MAX_CONTEXT_CHARS);
					return JSON.stringify(result);
				})(),
				0
			]
		]) {
			const { pi, options } = host(stdout, code);
			assert.equal(
				await createLifecycle(pi, { ...options, scope }).beforeAgentStart({ prompt: 'prompt' }, {}),
				undefined
			);
		}
	}
});

test('agent_settled schedules one tagged capture follow-up', () => {
	const { pi, calls } = host();
	const lifecycle = createLifecycle(pi);
	lifecycle.agentSettled();
	lifecycle.agentSettled();
	assert.equal(calls.messages.length, 1);
	assert.equal(calls.messages[0][0].customType, 'stormbuffer-capture-review');
	assert.match(calls.messages[0][0].content, /Submit nothing for routine completion/);
	assert.deepEqual(calls.messages[0][1], { triggerTurn: true, deliverAs: 'followUp' });
});

test('capture review retains the selected scope', () => {
	const { pi, calls } = host();
	createLifecycle(pi, { scope: 'local' }).agentSettled();
	assert.match(calls.messages[0][0].content, /selected local scope/);
});

test('capture review derives project scope from the active working directory', async () => {
	const root = await mkdtemp(join(tmpdir(), 'stormbuffer-pi-scope-'));
	const nested = join(root, 'src');
	await mkdir(join(root, '.sbuf'), { recursive: true });
	await mkdir(nested);
	await writeFile(join(root, '.sbuf', 'store.toml'), 'version = 1\n');
	const { pi, calls, options } = host();
	const lifecycle = createLifecycle(pi, options);

	await lifecycle.beforeAgentStart({ prompt: 'current prompt' }, { cwd: nested });
	lifecycle.agentSettled();

	assert.deepEqual(calls.recall[0].args, ['--project', 'invoke', 'context']);
	assert.match(calls.messages[0][0].content, /selected project scope/);
});

test('capture turn neither recalls nor schedules itself', async () => {
	const { pi, calls } = host();
	const lifecycle = createLifecycle(pi);
	assert.equal(await lifecycle.beforeAgentStart({ prompt: `${CAPTURE_MARKER} internal` }, {}), undefined);
	lifecycle.agentSettled();
	assert.equal(calls.recall.length, 0);
	assert.equal(calls.messages.length, 0);
});

test('host failures do not block the turn', async () => {
	const { pi } = host();
	const unavailable = {
		retrieveContext: async () => {
			throw new Error('missing');
		}
	};
	assert.equal(await createLifecycle(pi, unavailable).beforeAgentStart({ prompt: 'prompt' }, {}), undefined);
	assert.equal(await createLifecycle(pi).beforeAgentStart({}, {}), undefined);
});
