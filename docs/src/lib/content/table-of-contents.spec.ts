import { describe, expect, test } from 'vitest';
import { extractTableOfContents } from './table-of-contents';

describe('documentation table of contents extraction', () => {
	test('derives h2 and h3 entries from the rendered headings', () => {
		const file = { data: { fm: { title: 'Example' } } };
		const tree = {
			type: 'root',
			children: [
				{ type: 'element', tagName: 'h1', properties: { id: 'example' }, children: [] },
				{
					type: 'element',
					tagName: 'h2',
					properties: { id: 'first-section' },
					children: [{ type: 'text', value: 'First section' }]
				},
				{
					type: 'element',
					tagName: 'h3',
					properties: { id: 'nested-section' },
					children: [
						{ type: 'text', value: 'Nested ' },
						{ type: 'element', tagName: 'code', children: [{ type: 'text', value: 'section' }] }
					]
				}
			]
		};

		extractTableOfContents()(tree, file);

		expect(file.data.fm).toEqual({
			title: 'Example',
			toc: [
				{ title: 'First section', slug: 'first-section', level: 2 },
				{ title: 'Nested section', slug: 'nested-section', level: 3 }
			]
		});
	});
});
