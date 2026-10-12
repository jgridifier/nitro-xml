// SPDX-License-Identifier: MIT
// Catches: docx HARD_FIXTURES rows marked informational so pixel regressions do not fail CI.
import { test, expect } from 'bun:test';
import { docxVisualCorpus } from '../docx-corpus.mjs';

/** Mirror `recordRow` in `parity/visual/run.mjs`. */
function hardFailForMismatch(c, ok) {
	const informational = Boolean(c.informational);
	return !ok && !informational;
}

test('hard docx corpus cases fail the gate on mismatch', () => {
	const hard = docxVisualCorpus().filter((c) => !c.informational);
	expect(hard.some((c) => c.fixture.endsWith('poi-bug65649.docx'))).toBe(true);
	for (const c of hard) {
		expect(hardFailForMismatch(c, false)).toBe(true);
		expect(hardFailForMismatch(c, true)).toBe(false);
	}
	const chartex = docxVisualCorpus().find((c) => c.fixture.endsWith('poi-chartex.docx'));
	expect(chartex).toBeDefined();
	expect(chartex.informational).toBe(false);
	const hardened61745 = docxVisualCorpus().find((c) => c.fixture.endsWith('poi-61745.docx'));
	expect(hardened61745).toBeDefined();
	expect(hardened61745.informational).toBe(false);
	expect(docxVisualCorpus().every((c) => !c.informational)).toBe(true);
});
