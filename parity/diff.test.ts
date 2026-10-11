// SPDX-License-Identifier: MIT
// Catches: allow-list path prefixes failing to suppress nested diffs (CI false red).
import { describe, expect, test } from 'bun:test';
import { diffIr, filterKnownDiffs } from './diff.ts';

describe('diff + known-diffs', () => {
	test('prefix allow-list covers nested paths; unrelated paths still fail', () => {
		const diffs = diffIr(
			{ sheets: [{ items: [{ view: { text: 'A' } }] }], meta: 1 },
			{ sheets: [{ items: [{ view: { text: 'B' } }] }], meta: 2 },
		);
		const { unexpected, allowed } = filterKnownDiffs(diffs, 'xlsx', 'fix-1', [
			{
				format: 'xlsx',
				fixture: 'fix-1',
				path: '/sheets',
				reason: 'sheet paint still landing in #18',
			},
		]);
		expect(allowed.length).toBeGreaterThan(0);
		expect(unexpected.some((d) => d.path === '/meta')).toBe(true);
		expect(unexpected.some((d) => d.path.startsWith('/sheets'))).toBe(false);
	});

	test('suffix **/field allow-list covers nested text without hiding sibling fields', () => {
		// Catches: per-field known-diffs must not swallow transform/size regressions.
		const diffs = diffIr(
			{ slides: [{ elements: [{ text: 'A', width: 1 }] }] },
			{ slides: [{ elements: [{ text: 'B', width: 2 }] }] },
		);
		const { unexpected, allowed } = filterKnownDiffs(diffs, 'pptx', 'f1', [
			{
				format: 'pptx',
				fixture: 'f1',
				path: '**/text',
				reason: 'text body extraction lands in #26',
			},
		]);
		expect(allowed.some((d) => d.path.endsWith('/text'))).toBe(true);
		expect(unexpected.some((d) => d.path.endsWith('/width'))).toBe(true);
	});
});
