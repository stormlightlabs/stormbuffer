import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const MCP_CANDIDATE_TOOL = /^mcp__stormbuffer__memory_(remember|update)$/;

function stateFile(options = {}) {
	const sessionId = options.session_id ?? options.sessionId;
	const turnId = options.turn_id ?? options.turnId;
	if (typeof sessionId !== 'string' || sessionId.length === 0 || typeof turnId !== 'string' || turnId.length === 0) {
		return null;
	}
	const user = typeof process.getuid === 'function' ? process.getuid() : 'user';
	const root = options.root ?? join(tmpdir(), `stormbuffer-codex-${user}`);
	const key = createHash('sha256').update(`${sessionId}\0${turnId}`).digest('hex');
	return { file: join(root, `${key}.candidate`), root };
}

export function markCandidateWrite(options = {}) {
	const state = stateFile(options);
	if (!state) return;
	try {
		mkdirSync(state.root, { recursive: true, mode: 0o700 });
		writeFileSync(state.file, '', { mode: 0o600 });
	} catch {
		// The candidate still succeeded when optional bookkeeping is unavailable.
	}
}

export function consumeCandidateWrite(options = {}) {
	const state = stateFile(options);
	if (!state) return false;
	try {
		if (!existsSync(state.file)) return false;
		rmSync(state.file, { force: true });
		return true;
	} catch {
		return false;
	}
}

function operationForEvent(event) {
	return typeof event?.tool_name === 'string' ? MCP_CANDIDATE_TOOL.exec(event.tool_name)?.[1] ?? null : null;
}

function successfulEnvelope(value, operation) {
	if (!value) return false;
	if (typeof value === 'string') {
		for (const line of [value, ...value.split('\n')]) {
			try {
				if (successfulEnvelope(JSON.parse(line), operation)) return true;
			} catch {
				// Tool responses often wrap the one-line protocol envelope in diagnostics.
			}
		}
		return false;
	}
	if (Array.isArray(value)) return value.some((item) => successfulEnvelope(item, operation));
	if (typeof value !== 'object') return false;
	if (value.ok === true && value.operation === operation) return true;
	return ['structuredContent', 'structured_content', 'output', 'stdout', 'content'].some((key) =>
		successfulEnvelope(value[key], operation)
	);
}

export function candidateWriteSucceeded(event) {
	const operation = operationForEvent(event);
	return operation ? successfulEnvelope(event?.tool_response, operation) : false;
}
