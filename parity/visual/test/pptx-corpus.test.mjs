// SPDX-License-Identifier: MIT
// Catches: pptx hard-flip set accidentally emptied, or print re-enabled on the corpus (CI blow-up).
import { test, expect } from 'bun:test';
import { PPTX_HARD_FIXTURE_IDS, pptxVisualCorpus } from '../pptx-corpus.mjs';

test('pptx visual corpus keeps skipPrint and hard-flips only the allow-listed fixtures', () => {
	const prev = process.env.VISUAL_PPTX_MAX;
	delete process.env.VISUAL_PPTX_MAX;
	try {
		expect(PPTX_HARD_FIXTURE_IDS.size).toBeGreaterThan(0);
		const cases = pptxVisualCorpus();
		expect(cases.length).toBeGreaterThan(PPTX_HARD_FIXTURE_IDS.size);
		let hard = 0;
		let info = 0;
		for (const c of cases) {
			expect(c.format).toBe('pptx');
			expect(c.kind).toBe('pptx-viewer');
			expect(c.ready).toBe(true);
			expect(c.rust).toBe(true);
			expect(c.skipPrint).toBe(true);
			expect(c.fixture).toMatch(/\.(pptx|pptm|potx|potm)$/i);
			const id = c.id.replace(/^pptx-/, '');
			if (PPTX_HARD_FIXTURE_IDS.has(id)) {
				expect(c.informational).toBe(false);
				hard += 1;
			} else {
				expect(c.informational).toBe(true);
				info += 1;
			}
		}
		expect(hard).toBe(PPTX_HARD_FIXTURE_IDS.size);
		expect(info).toBeGreaterThan(0);
	} finally {
		if (prev === undefined) delete process.env.VISUAL_PPTX_MAX;
		else process.env.VISUAL_PPTX_MAX = prev;
	}
});
