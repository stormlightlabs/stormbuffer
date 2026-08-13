import { base } from '$app/paths';
import { getDocs } from '$lib/content';

export const prerender = true;

export function GET(): Response {
	const pages = getDocs().map((doc) => `- [${doc.title}](${base}/docs/${doc.slug}.md): ${doc.description}`);
	const body = [
		'# Stormbuffer',
		'',
		'> Local-first, human-reviewed project memory for people and coding agents.',
		'',
		'Stormbuffer stores canonical memories as readable Markdown with TOML frontmatter. Use these pages to install it, operate a store, integrate an agent, and understand its data and retrieval contracts.',
		'',
		'## Documentation',
		'',
		...pages,
		'',
		'## Agent resource',
		'',
		`- [Stormbuffer memory skill](${base}/stormbuffer-memory-skill.md): Canonical instructions for agents using Stormbuffer.`,
		''
	].join('\n');

	return new Response(body, { headers: { 'content-type': 'text/plain; charset=utf-8' } });
}
