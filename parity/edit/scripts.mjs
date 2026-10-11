// SPDX-License-Identifier: MIT
/**
 * Load edits/<format>/*.json and resolve fixture rows from fixtures/manifest.json.
 */

import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { ROOT, loadManifest, fixtureAbs, fixtureExists } from '../roundtrip/corpus.mjs';

const EDITS_ROOT = join(ROOT, 'edits');
const FORMATS = ['xlsx', 'docx', 'pptx'];

/**
 * @typedef {{
 *   id: string,
 *   format: 'xlsx'|'docx'|'pptx',
 *   fixture: string,
 *   description?: string,
 *   ops: object[],
 *   path: string,
 * }} EditScript
 */

export function loadSchema() {
	return JSON.parse(readFileSync(join(EDITS_ROOT, 'schema.json'), 'utf8'));
}

/**
 * Lightweight required-field check (full JSON Schema not required in CI).
 * @param {unknown} script
 * @param {string} path
 * @returns {string[]}
 */
export function validateScriptShape(script, path = '') {
	/** @type {string[]} */
	const errors = [];
	if (!script || typeof script !== 'object') {
		errors.push(`${path}: not an object`);
		return errors;
	}
	const s = /** @type {Record<string, unknown>} */ (script);
	for (const key of ['id', 'format', 'fixture', 'ops']) {
		if (s[key] === undefined || s[key] === null || s[key] === '') {
			errors.push(`${path}: missing ${key}`);
		}
	}
	if (s.format && !FORMATS.includes(/** @type {string} */ (s.format))) {
		errors.push(`${path}: format must be xlsx|docx|pptx`);
	}
	if (!Array.isArray(s.ops) || s.ops.length === 0) {
		errors.push(`${path}: ops must be a non-empty array`);
	} else {
		s.ops.forEach((op, i) => {
			if (!op || typeof op !== 'object' || !/** @type {any} */ (op).op) {
				errors.push(`${path}: ops[${i}] missing op`);
			}
		});
	}
	return errors;
}

/**
 * @param {{ format?: string, id?: string }} [opts]
 * @returns {EditScript[]}
 */
export function loadEditScripts(opts = {}) {
	/** @type {EditScript[]} */
	const out = [];
	for (const format of FORMATS) {
		if (opts.format && opts.format !== format) continue;
		const dir = join(EDITS_ROOT, format);
		if (!existsSync(dir)) continue;
		for (const name of readdirSync(dir).sort()) {
			if (!name.endsWith('.json')) continue;
			const path = join(dir, name);
			const script = JSON.parse(readFileSync(path, 'utf8'));
			if (opts.id && script.id !== opts.id) continue;
			const errs = validateScriptShape(script, path);
			if (errs.length) {
				throw new Error(`invalid edit script:\n${errs.join('\n')}`);
			}
			if (script.format !== format) {
				throw new Error(`${path}: format ${script.format} != directory ${format}`);
			}
			out.push({ ...script, path });
		}
	}
	return out;
}

/**
 * @param {EditScript} script
 */
export function resolveFixture(script) {
	const manifest = loadManifest();
	const row = manifest.fixtures.find((f) => f.id === script.fixture);
	if (!row) throw new Error(`fixture id not in manifest: ${script.fixture} (${script.id})`);
	if (!fixtureExists(row)) {
		throw new Error(`fixture missing on disk: ${row.path} (${script.id})`);
	}
	return row;
}

export { ROOT, fixtureAbs, FORMATS, EDITS_ROOT };
