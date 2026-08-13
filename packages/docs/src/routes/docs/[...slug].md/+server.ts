import { error } from '@sveltejs/kit';
import { getDoc, getDocs } from '$lib/content';

export const prerender = true;

export function entries() {
	return getDocs().map((doc) => ({ slug: doc.slug }));
}

export function GET({ params }): Response {
	const doc = getDoc(params.slug.replace(/\/+$/, ''));
	if (!doc) throw error(404, 'Documentation page not found');

	return new Response(doc.markdown, { headers: { 'content-type': 'text/markdown; charset=utf-8' } });
}
