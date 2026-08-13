import { describe, expect, test } from 'vitest';
import { validateFrontmatter } from './index';

const validFrontmatter = {
	title: 'Example page',
	description: 'A short description.',
	section: 'Concepts',
	group: 'Core concepts',
	order: 99,
	toc: [{ title: 'A heading', slug: 'a-heading', level: 2 }]
};

describe('documentation content', () => {
	test('rejects incomplete frontmatter with its source path', () => {
		expect(() => validateFrontmatter({ ...validFrontmatter, order: 0 }, 'example.md')).toThrow(
			'example.md: order must be a positive integer'
		);
		expect(() => validateFrontmatter({ ...validFrontmatter, toc: [] }, 'example.md')).toThrow(
			'example.md: toc must contain at least one heading'
		);
	});
});
