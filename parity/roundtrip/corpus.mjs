// SPDX-License-Identifier: MIT
/**
 * Fixture selection for the E0 round-trip gate (#49).
 * Full corpus from fixtures/manifest.json: xlsx/docx/pptx family including
 * macro and template extensions. Skip non-redistributable / empty-path rows.
 */

import { existsSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');

/** @typedef {{ id: string, format: string, path: string, tier?: string, features?: string[], redistributable?: boolean, on_demand?: boolean }} FixtureRow */

const PACKAGE_FORMATS = new Set([
	'xlsx',
	'xlsm',
	'xltx',
	'xltm',
	'docx',
	'docm',
	'dotx',
	'dotm',
	'pptx',
	'pptm',
	'potx',
	'potm',
]);

/** Normalize manifest format (often family) + path extension → package kind. */
export function packageKind(row) {
	const ext = (row.path || '').split('.').pop()?.toLowerCase() || '';
	if (PACKAGE_FORMATS.has(ext)) return ext;
	const fam = (row.format || '').toLowerCase();
	if (fam === 'xlsx' || fam === 'docx' || fam === 'pptx') return fam;
	return null;
}

export function familyOf(kind) {
	if (!kind) return null;
	if (kind.startsWith('xl')) return 'xlsx';
	if (kind.startsWith('do') || kind.startsWith('dot')) return 'docx';
	if (kind.startsWith('pp') || kind.startsWith('pot')) return 'pptx';
	return null;
}

export function loadManifest(root = ROOT) {
	return JSON.parse(readFileSync(join(root, 'fixtures/manifest.json'), 'utf8'));
}

/**
 * Every redistributable package fixture with an on-disk path.
 * Includes L/XL tiers and macro/template formats (user #49 brief).
 */
export function selectRoundtripCorpus(manifest, opts = {}) {
	const skipOnDemand = opts.skipOnDemand !== false;
	const formatFilter = opts.format; // family: xlsx|docx|pptx
	return manifest.fixtures.filter((f) => {
		if (f.redistributable === false) return false;
		if (skipOnDemand && f.on_demand) return false;
		if (!f.path) return false;
		const kind = packageKind(f);
		if (!kind) return false;
		const fam = familyOf(kind);
		if (formatFilter && fam !== formatFilter) return false;
		return true;
	});
}

export function fixtureAbs(row, root = ROOT) {
	return join(root, row.path);
}

export function fixtureExists(row, root = ROOT) {
	return existsSync(fixtureAbs(row, root));
}

export { ROOT };
