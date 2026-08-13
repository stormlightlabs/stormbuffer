import type { Component } from 'svelte';
import type { Doc, DocFrontmatter, DocHeading, DocSection } from './types';
import { docSections } from './types';

type MarkdownModule = { default: Component; metadata?: unknown };

const markdownModules = import.meta.glob<MarkdownModule>('/src/content/docs/**/*.md', { eager: true });
const markdownSources = import.meta.glob<string>('/src/content/docs/**/*.md', {
	eager: true,
	query: '?raw',
	import: 'default'
});

const sectionSet = new Set<string>(docSections);
const slugPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*(?:\/[a-z0-9]+(?:-[a-z0-9]+)*)*$/;
const headingSlugPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function requiredString(value: Record<string, unknown>, key: string, source: string): string {
	const candidate = value[key];
	if (typeof candidate !== 'string' || candidate.trim() === '') {
		throw new Error(`Invalid frontmatter in ${source}: ${key} must be a non-empty string`);
	}
	return candidate.trim();
}

function requiredOrder(value: Record<string, unknown>, source: string): number {
	const candidate = value.order;
	if (typeof candidate !== 'number' || !Number.isInteger(candidate) || candidate < 1) {
		throw new Error(`Invalid frontmatter in ${source}: order must be a positive integer`);
	}
	return candidate;
}

function requiredToc(value: Record<string, unknown>, source: string): DocHeading[] {
	if (!Array.isArray(value.toc) || value.toc.length === 0) {
		throw new Error(`Invalid frontmatter in ${source}: toc must contain at least one heading`);
	}

	return value.toc.map((heading, index) => {
		if (!isRecord(heading)) {
			throw new Error(`Invalid frontmatter in ${source}: toc[${index}] must be an object`);
		}

		const title = requiredString(heading, 'title', source);
		const slug = requiredString(heading, 'slug', source);
		const level = heading.level;

		if (!headingSlugPattern.test(slug)) {
			throw new Error(`Invalid frontmatter in ${source}: toc[${index}].slug is invalid`);
		}
		if (level !== 2 && level !== 3) {
			throw new Error(`Invalid frontmatter in ${source}: toc[${index}].level must be 2 or 3`);
		}

		return { title, slug, level };
	});
}

export function validateFrontmatter(value: unknown, source: string): DocFrontmatter {
	if (!isRecord(value)) {
		throw new Error(`Invalid frontmatter in ${source}: expected an object`);
	}

	const section = requiredString(value, 'section', source);
	if (!sectionSet.has(section)) {
		throw new Error(`Invalid frontmatter in ${source}: section must be one of ${docSections.join(', ')}`);
	}

	const frontmatter: DocFrontmatter = {
		title: requiredString(value, 'title', source),
		description: requiredString(value, 'description', source),
		section: section as DocSection,
		group: requiredString(value, 'group', source),
		order: requiredOrder(value, source),
		toc: requiredToc(value, source)
	};

	return frontmatter;
}

function sourceToSlug(source: string): string {
	const slug = source.replace('/src/content/docs/', '').replace(/\.md$/, '');
	if (!slugPattern.test(slug)) {
		throw new Error(`Invalid documentation path: ${source}`);
	}
	return slug;
}

export const docs: Doc[] = Object.entries(markdownModules)
	.map(([source, module]) => ({
		...validateFrontmatter(module.metadata, source),
		slug: sourceToSlug(source),
		component: module.default,
		markdown: markdownSources[source]
	}))
	.sort((left, right) => left.order - right.order);

export function getDocs(): Doc[] {
	return docs;
}

export function getDoc(slug: string): Doc | undefined {
	return docs.find((doc) => doc.slug === slug);
}

export function getAdjacentDocs(slug: string): { previous?: Doc; next?: Doc } {
	const index = docs.findIndex((doc) => doc.slug === slug);
	return {
		previous: index > 0 ? docs[index - 1] : undefined,
		next: index >= 0 && index < docs.length - 1 ? docs[index + 1] : undefined
	};
}
