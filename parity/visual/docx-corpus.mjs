// SPDX-License-Identifier: MIT
/**
 * Build docx viewer visual cases from fixtures/real/docx (#23).
 *
 * Same walk as `parity/comparisons/docx-layout.ts`. Hard rows are those measured
 * ≤0.1% at DPR1, DPR2 and print on branch tip; the rest stay informational.
 */

import { existsSync, readdirSync } from 'node:fs';
import { join, dirname, relative, extname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const DOCX_EXT = /\.(docx|docm|dotx|dotm)$/i;

/** Fixture paths (repo-relative) that pass ≤0.1% at DPR1 + DPR2 + print. */
const HARD_FIXTURES = new Set([
	'fixtures/real/docx/poi-58067.docx',
	'fixtures/real/docx/poi-60316.docx',
	'fixtures/real/docx/poi-60316.dotx',
	'fixtures/real/docx/poi-60329.docx',
	'fixtures/real/docx/poi-61470.docx',
	'fixtures/real/docx/poi-SimpleMacro.docm',
	'fixtures/real/docx/poi-testComment.docx',
	'fixtures/real/docx/python-docx-comments-rich-para.docx',
	'fixtures/real/docx/python-docx-doc-access-sections.docx',
	'fixtures/real/docx/python-docx-having-images.docx',
	'fixtures/real/docx/python-docx-hdr-header-footer.docx',
	'fixtures/real/docx/python-docx-sct-first-page-hdrftr.docx',
	'fixtures/real/docx/python-docx-sct-inner-content.docx',
	'fixtures/real/docx/python-docx-sct-section-props.docx',
	'fixtures/real/docx/python-docx-shp-inline-shape-access.docx',
	'fixtures/real/docx/python-docx-tbl-cell-access.docx',
	'fixtures/real/docx/python-docx-tbl-cell-props.docx',
	'fixtures/real/docx/python-docx-tbl-on-off-props.docx',
	'fixtures/real/docx/poi-bug65649.docx',
]);

function listDocxFiles() {
	const out = [];
	const walk = (dir) => {
		if (!existsSync(dir)) return;
		for (const name of readdirSync(dir, { withFileTypes: true })) {
			const p = join(dir, name.name);
			if (name.isDirectory()) walk(p);
			else if (DOCX_EXT.test(name.name)) out.push(p);
		}
	};
	walk(join(ROOT, 'fixtures/real/docx'));
	return out.filter((p) => DOCX_EXT.test(p)).sort();
}

/**
 * @returns {import('./registry.mjs').VisualCase[]}
 */
export function docxVisualCorpus() {
	const paths = listDocxFiles();
	const maxRaw = process.env.VISUAL_DOCX_MAX;
	const max = maxRaw != null && maxRaw !== '' ? Number(maxRaw) : Infinity;
	const limited = Number.isFinite(max) && max >= 0 ? paths.slice(0, max) : paths;

	return limited.map((abs) => {
		const fixture = relative(ROOT, abs).replace(/\\/g, '/');
		const ext = extname(fixture).replace(/^\./, '').toLowerCase();
		const base = basename(fixture, extname(fixture));
		const slug = `real-docx-${base}-${ext}`
			.replace(/[^a-zA-Z0-9]+/g, '-')
			.replace(/^-+|-+$/g, '')
			.toLowerCase();
		const hard = HARD_FIXTURES.has(fixture);
		return {
			id: `docx-viewer-${slug || 'fixture'}`,
			format: 'docx',
			kind: 'docx-viewer',
			ready: true,
			fixture,
			width: 800,
			height: 600,
			rust: true,
			threshold: 0.001,
			informational: !hard,
		};
	});
}
