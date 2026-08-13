import type { Component } from 'svelte';

export const docSections = ['Get started', 'Reference', 'Concepts'] as const;
export type DocSection = (typeof docSections)[number];

export type DocHeading = { title: string; slug: string; level: 2 | 3 };

export type DocFrontmatter = {
	title: string;
	description: string;
	section: DocSection;
	group: string;
	order: number;
	toc: DocHeading[];
};

export type Doc = DocFrontmatter & { slug: string; component: Component; markdown: string };
