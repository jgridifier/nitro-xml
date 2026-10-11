// SPDX-License-Identifier: MIT
// Catches: pptx visual corpus rows accidentally registered as hard-fail or with print enabled (CI blow-up / false red).
import { test, expect } from 'bun:test';
import { pptxVisualCorpus } from '../pptx-corpus.mjs';

test('pptx visual corpus cases are informational, skip print, and use pptx-viewer', () => {
	const prev = process.env.VISUAL_PPTX_MAX;
	process.env.VISUAL_PPTX_MAX = '5';
	try {
		const cases = pptxVisualCorpus();
		expect(cases.length).toBeGreaterThan(0);
		expect(cases.length).toBeLessThanOrEqual(5);
		for (const c of cases) {
			expect(c.format).toBe('pptx');
			expect(c.kind).toBe('pptx-viewer');
			expect(c.ready).toBe(true);
			expect(c.rust).toBe(true);
			expect(c.informational).toBe(true);
			expect(c.skipPrint).toBe(true);
			expect(c.fixture).toMatch(/\.(pptx|pptm|potx|potm)$/i);
		}
	} finally {
		if (prev === undefined) delete process.env.VISUAL_PPTX_MAX;
		else process.env.VISUAL_PPTX_MAX = prev;
	}
});
