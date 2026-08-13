import { error } from '@sveltejs/kit';
import { getDoc, getDocs } from '$lib/content';

export const prerender = true;

export function entries() {
	return getDocs().map((doc) => ({ slug: doc.slug }));
}

export function load({ params }) {
	const slug = params.slug.replace(/\/+$/, '');
	if (!getDoc(slug)) {
		throw error(404, 'Documentation page not found');
	}

	return { slug };
}
