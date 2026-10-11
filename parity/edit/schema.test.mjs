// SPDX-License-Identifier: MIT
import { describe, expect, test } from 'bun:test';
import { loadEditScripts, loadSchema, validateScriptShape, FORMATS } from './scripts.mjs';

describe('edit scripts', () => {
	// Catches: edit scripts missing id/format/fixture/ops slip into the corpus.
	test('every script has required shape fields', () => {
		const scripts = loadEditScripts();
		expect(scripts.length).toBeGreaterThanOrEqual(60);
		for (const s of scripts) {
			const errs = validateScriptShape(s, s.path);
			expect(errs).toEqual([]);
			expect(FORMATS).toContain(s.format);
			expect(s.ops.length).toBeGreaterThan(0);
		}
	});

	// Catches: a format directory falling below the ≥20 scenarios/format bar.
	test('at least 20 scenarios per format', () => {
		const scripts = loadEditScripts();
		for (const f of FORMATS) {
			const n = scripts.filter((s) => s.format === f).length;
			expect(n).toBeGreaterThanOrEqual(20);
		}
	});

	// Catches: schema.json drifting away from the documented op vocabulary.
	test('schema enumerates xlsx/docx/pptx ops', () => {
		const schema = loadSchema();
		const ops = schema.$defs.op.properties.op.enum;
		for (const need of [
			'setCellInput',
			'applyStyle',
			'addSheet',
			'setParagraphText',
			'setTableCellText',
			'setShapeText',
			'patchShapeGeometry',
		]) {
			expect(ops).toContain(need);
		}
	});
});
