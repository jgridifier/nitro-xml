// SPDX-License-Identifier: MIT
/**
 * Build informational docx viewer visual cases from fixtures/real/docx (#23).
 *
 * Same walk as `parity/comparisons/docx-layout.ts`. Each case compares
 * `<nitro-docx>` vs upstream Print Layout at DPR 1/2. Mismatches are reported,
 * not hard-failed, until the orchestrator flips `informational` after measuring.
 *
 * Caps: `VISUAL_DOCX_MAX` (default unlimited).
 */

import { existsSync, readdirSync } from 'node:fs';
import { join, dirname, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const DOCX_EXT = /\.(docx|docm|dotx|dotm)$/i;

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
		const slug = fixture
			.replace(/^fixtures\//, '')
			.replace(/\.(docx|docm|dotx|dotm)$/i, '')
			.replace(/[^a-zA-Z0-9]+/g, '-')
			.replace(/^-+|-+$/g, '')
			.toLowerCase();
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
			informational: true,
		};
	});
}
