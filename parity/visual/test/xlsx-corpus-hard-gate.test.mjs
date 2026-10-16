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
	// Catches: hard-row regressions being ignored, even after every real fixture is hardened.
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
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-62629_target.xlsm')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-DataTableCities.xlsx')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/usgov-fhwa-2022-vm1.xlsx')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-45540_classic_Header.xlsx')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-45540_classic_Footer.xlsx')).toBe(true);
	expect(HARD_FIXTURES.has('fixtures/real/xlsx/poi-ConditionalFormattingSamples.xlsx')).toBe(
		true,
	);
	for (const c of hard) {
		expect(hardFailForMismatch(c, false)).toBe(true);
		expect(hardFailForMismatch(c, true)).toBe(false);
	}

	// Negative control: exercise corpus classification with one real fixture unlisted.
	const fixture = hard[0].fixture;
	HARD_FIXTURES.delete(fixture);
	try {
		const soft = xlsxVisualCorpus().find((c) => c.fixture === fixture);
		expect(soft).toBeDefined();
		expect(soft.informational).toBe(true);
		expect(hardFailForMismatch(soft, false)).toBe(false);
	} finally {
		HARD_FIXTURES.add(fixture);
	}
	expect(xlsxVisualCorpus().find((c) => c.fixture === fixture).informational).toBe(false);
});
