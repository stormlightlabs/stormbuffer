import { spawnSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';

const iterations = Number.parseInt(process.env.STORMBUFFER_BENCH_ITERATIONS || '20', 10);
const cwd = process.env.STORMBUFFER_BENCH_CWD || process.cwd();
const hook = new URL('../hooks/codex.mjs', import.meta.url);
const event = JSON.stringify({ prompt: 'warm retrieval latency', cwd });
const timings = [];

for (let index = 0; index <= iterations; index += 1) {
	const started = performance.now();
	const result = spawnSync(process.execPath, [hook.pathname, 'prompt'], {
		input: event,
		encoding: 'utf8',
		env: process.env
	});
	const elapsed = performance.now() - started;
	if (result.status !== 0) throw new Error('Stormbuffer prompt hook failed');
	let output;
	try {
		output = JSON.parse(result.stdout);
	} catch {
		throw new Error('Stormbuffer prompt hook returned malformed JSON');
	}
	const context = output?.hookSpecificOutput?.additionalContext;
	if (typeof context !== 'string' || !context.includes('"receipt_id"')) {
		throw new Error('Stormbuffer prompt hook returned no recalled context');
	}
	if (index > 0) timings.push(elapsed);
}

timings.sort((left, right) => left - right);
const percentile = (fraction) => timings[Math.min(timings.length - 1, Math.floor(timings.length * fraction))];
process.stdout.write(
	`${JSON.stringify({
		iterations,
		p50_ms: Number(percentile(0.5).toFixed(1)),
		p95_ms: Number(percentile(0.95).toFixed(1)),
		max_ms: Number(timings.at(-1).toFixed(1))
	})}\n`
);
