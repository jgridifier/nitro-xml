#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Catches: summarise ranking an entrant whose output differs from the reference (must stay unranked).
// Catches: Excel 1900-system serial conversion drifting across the fictitious 1900-02-29 boundary.
// Catches: package.json / competitors.json pin drift silently shipping a different SheetJS or ExcelJS build.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { dateToSerial, firstDifference } from '../../lib/normalise.mjs';
import { summarise } from '../../lib/summary.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '../..');

// --- equivalence: incomparable must not get a ratio ---
{
	const files = [
		{
			format: 'xlsx',
			file: 'a.xlsx',
			results: {
				'upstream-ts': { status: 'ok', comparable: true, medianMs: 10 },
				sheetjs: { status: 'ok', comparable: false, medianMs: 1, firstDifference: { reason: 'cell mismatch' } },
				exceljs: { status: 'ok', comparable: true, medianMs: 20 },
			},
		},
	];
	const entrants = [
		{ id: 'upstream-ts', formats: ['xlsx'], status: 'entered' },
		{ id: 'sheetjs', formats: ['xlsx'], status: 'entered' },
		{ id: 'exceljs', formats: ['xlsx'], status: 'entered' },
	];
	const s = summarise(files, entrants, 'upstream-ts');
	assert.equal(s.xlsx.sheetjs.comparable, 0);
	assert.equal(s.xlsx.sheetjs.notComparable, 1);
	assert.equal(s.xlsx.sheetjs.geomeanRatioVsReference, null);
	assert.equal(s.xlsx.sheetjs.sharedWithReference, 0);
	assert.equal(s.xlsx.exceljs.geomeanRatioVsReference, 2);
	assert.equal(s.xlsx.exceljs.lossesVsReference, 1);
	assert.equal(s.xlsx.exceljs.winsVsReference, 0);
}

// --- Excel / JS calendar mismatch around the fictitious 1900-02-29 ---
{
	// Catches: ISO dates go through the JS calendar (Feb28→Mar1 is one day), so an adapter that
	// emits Excel's 1900-system serials as plain numbers can disagree on early-1900 dates and must
	// be marked not comparable rather than ranked.
	const feb28 = dateToSerial('1900-02-28');
	const mar01 = dateToSerial('1900-03-01');
	assert.ok(Math.abs(feb28 - 60) < 1e-9, `1900-02-28 -> ${feb28}, want ~60`);
	assert.ok(Math.abs(mar01 - 61) < 1e-9, `1900-03-01 -> ${mar01}, want ~61`);
	assert.ok(Math.abs(mar01 - feb28 - 1) < 1e-9, 'JS calendar gap must be 1 day (no fake leap day)');
	const diff = firstDifference(
		'xlsx',
		[['Sheet1', 'A1', 'n', 59]],
		[['Sheet1', 'A1', 'n', 60]],
	);
	assert.ok(diff, 'serial 59 vs 60 must differ');
}

// --- pin drift: competitors.json versions must match package.json dependencies ---
{
	const competitors = JSON.parse(readFileSync(join(root, 'competitors.json'), 'utf8'));
	const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
	const expect = {
		sheetjs: { field: 'xlsx', want: 'https://cdn.sheetjs.com/xlsx-0.20.3/xlsx-0.20.3.tgz' },
		exceljs: { field: 'exceljs', want: '4.4.0' },
		'docx-preview': { field: 'docx-preview', want: '0.4.1' },
		mammoth: { field: 'mammoth', want: '1.13.0' },
	};
	for (const [id, { field, want }] of Object.entries(expect)) {
		const entrant = competitors.entrants.find((e) => e.id === id);
		assert.ok(entrant, `missing entrant ${id}`);
		assert.match(String(entrant.version), id === 'sheetjs' ? /0\.20\.3/ : new RegExp(want.replace(/\./g, '\\.')));
		assert.equal(pkg.dependencies[field], want, `${field} pin drifted: package.json has ${pkg.dependencies[field]}`);
	}
	const calamine = competitors.entrants.find((e) => e.id === 'calamine');
	const umya = competitors.entrants.find((e) => e.id === 'umya-spreadsheet');
	const docxRs = competitors.entrants.find((e) => e.id === 'docx-rs');
	assert.match(calamine.version, /0\.36\.1/);
	assert.match(umya.version, /3\.1\.0/);
	assert.match(docxRs.version, /0\.4\.22/);
	const defaults = JSON.parse(readFileSync(join(here, 'defaults.json'), 'utf8'));
	assert.equal(defaults.seed, 20261011);
	assert.ok(defaults.timedRuns >= 7, 'timed runs must stay high enough for a stable median');
	assert.ok(defaults.warmupRuns >= 2);
}

console.log('parse-race selftest: ok');
