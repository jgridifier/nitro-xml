// SPDX-License-Identifier: MIT
/**
 * Build xlsx viewer visual cases from fixtures/real/xlsx (#150 follow-up).
 *
 * Hard rows are those measured ≤0.1% at DPR1, DPR2 and print on branch tip;
 * the rest stay informational. `poi-45544` stays the static hard case in
 * `registry.mjs` (stable id `xlsx-viewer-poi-45544`); this corpus skips it.
 */

import { existsSync, readdirSync } from 'node:fs';
import { join, dirname, relative, extname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const XLSX_EXT = /\.(xlsx|xlsm|xltx|xltm)$/i;

/** Already gated as static `xlsx-viewer-poi-45544` (#150). */
const STATIC_HARD = 'fixtures/real/xlsx/poi-45544.xlsx';

/**
 * Fixture paths (repo-relative) that pass ≤0.1% at DPR1 + DPR2 + print.
 * Visual runner launches Chromium with `--disable-lcd-text` (same as docx).
 */
export const HARD_FIXTURES = new Set([
	// Populated as fixtures converge; poi-45544 is the static hard case.
]);

function listXlsxFiles() {
	const out = [];
	const walk = (dir) => {
		if (!existsSync(dir)) return;
		for (const name of readdirSync(dir, { withFileTypes: true })) {
			const p = join(dir, name.name);
			if (name.isDirectory()) walk(p);
			else if (XLSX_EXT.test(name.name)) out.push(p);
		}
	};
	walk(join(ROOT, 'fixtures/real/xlsx'));
	return out.filter((p) => XLSX_EXT.test(p)).sort();
}

/**
 * @returns {import('./registry.mjs').VisualCase[]}
 */
export function xlsxVisualCorpus() {
	const paths = listXlsxFiles();
	const maxRaw = process.env.VISUAL_XLSX_MAX;
	const max = maxRaw != null && maxRaw !== '' ? Number(maxRaw) : Infinity;
	const limited = Number.isFinite(max) && max >= 0 ? paths.slice(0, max) : paths;

	return limited
		.map((abs) => {
			const fixture = relative(ROOT, abs).replace(/\\/g, '/');
			if (fixture === STATIC_HARD) return null;
			const ext = extname(fixture).replace(/^\./, '').toLowerCase();
			const base = basename(fixture, extname(fixture));
			const slug = `real-xlsx-${base}-${ext}`
				.replace(/[^a-zA-Z0-9]+/g, '-')
				.replace(/^-+|-+$/g, '')
				.toLowerCase();
			const hard = HARD_FIXTURES.has(fixture);
			return {
				id: `xlsx-viewer-${slug || 'fixture'}`,
				format: 'xlsx',
				kind: 'xlsx-viewer',
				ready: true,
				fixture,
				width: 800,
				height: 480,
				rust: true,
				threshold: 0.001,
				informational: !hard,
			};
		})
		.filter(Boolean);
}
