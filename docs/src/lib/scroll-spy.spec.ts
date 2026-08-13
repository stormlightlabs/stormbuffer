import { describe, expect, test } from 'vitest';
import { findActiveHeading, type HeadingPosition, type ViewportPosition } from './scroll-spy';

const headings: HeadingPosition[] = [
	{ id: 'first', top: 400, scrollMarginTop: 104 },
	{ id: 'second', top: 900, scrollMarginTop: 104 },
	{ id: 'third', top: 1400, scrollMarginTop: 104 }
];

function viewport(overrides: Partial<ViewportPosition> = {}): ViewportPosition {
	return { scrollY: 0, height: 700, documentHeight: 2200, ...overrides };
}

describe('findActiveHeading', () => {
	test('has no active section before the first heading reaches the content offset', () => {
		expect(findActiveHeading(headings, viewport({ scrollY: 290 }))).toBeNull();
	});

	test('has no active section at the top of a short page', () => {
		expect(findActiveHeading(headings, viewport({ height: 2200 }))).toBeNull();
	});

	test('selects the last heading above the content offset', () => {
		expect(findActiveHeading(headings, viewport({ scrollY: 800 }))).toBe('second');
	});

	test('selects the final heading at the bottom of a short final section', () => {
		expect(findActiveHeading(headings, viewport({ scrollY: 1500, height: 700 }))).toBe('third');
	});

	test('handles a page without observed headings', () => {
		expect(findActiveHeading([], viewport())).toBeNull();
	});
});
