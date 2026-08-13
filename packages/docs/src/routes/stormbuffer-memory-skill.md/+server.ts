import { skillSource } from '$lib/content/skill';

export const prerender = true;

export function GET(): Response {
	return new Response(skillSource, { headers: { 'content-type': 'text/markdown; charset=utf-8' } });
}
