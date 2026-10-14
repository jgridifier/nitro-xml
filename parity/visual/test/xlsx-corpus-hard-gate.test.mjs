// SPDX-License-Identifier: MIT
// Catches: xlsx HARD_FIXTURES rows marked informational so pixel regressions do not fail CI.
import { test, expect } from 'bun:test';
import { HARD_FIXTURES, xlsxVisualCorpus } from '../xlsx-corpus.mjs';

/** Mirror `recordRow` in `parity/visual/run.mjs`. */
function hardFailForMismatch(c, ok) {
	const informational = Boolean(c.informational);
	return !ok && !informational;
}

test('hard xlsx corpus cases fail the gate on mismatch; soft stay informational', () => {
	const corpus = xlsxVisualCorpus();
	expect(corpus.length).toBeGreaterThan(0);
	expect(corpus.every((c) => c.fixture !== 'fixtures/real/xlsx/poi-45544.xlsx')).toBe(true);

	const hard = corpus.filter((c) => !c.informational);
	expect(hard.length).toBe(HARD_FIXTURES.size);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-WithChartSheet.xlsx')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-no_drawing_patriarch.xlsx')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-57181.xlsm')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-46535.xlsx')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-ExcelWithAttachments.xlsm')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-49156.xlsx')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/usgov-fhwa-2022-vm1.xlsx')).toBe(true);
	for (const c of hard) {
		expect(hardFailForMismatch(c, false)).toBe(true);
		expect(hardFailForMismatch(c, true)).toBe(false);
	}

	const soft = corpus.find((c) => c.informational);
	expect(soft).toBeDefined();
	expect(hardFailForMismatch(soft, false)).toBe(false);
});
