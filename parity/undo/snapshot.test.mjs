// SPDX-License-Identifier: MIT
import { describe, expect, test } from 'bun:test';
import { snapshotDocx, snapshotPptx, snapshotXlsx, snapshotsEqual, stableStringify } from './snapshot.mjs';

describe('canonical snapshots', () => {
	// Catches: key order in snapshot JSON flipping between runs breaks undo parity.
	test('stableStringify sorts object keys', () => {
		expect(stableStringify({ b: 1, a: 2 })).toBe(stableStringify({ a: 2, b: 1 }));
	});

	// Catches: xlsx snapshot ignoring Map-based sheet.rows so undo never sees cell edits.
	test('snapshotXlsx reads Map rows', () => {
		const wb = {
			sheets: [
				{
					name: 'S',
					rows: new Map([
						[0, new Map([[0, { v: 1, t: 'n' }], [1, { v: 'x', t: 's' }]])],
					]),
				},
			],
		};
		const snap = snapshotXlsx(wb);
		expect(snap.sheets[0].cells).toEqual([
			{ ref: '0:0', v: 1, t: 'n', f: null, s: null },
			{ ref: '0:1', v: 'x', t: 's', f: null, s: null },
		]);
	});

	// Catches: docx/pptx snapshot equality treating equivalent projections as different.
	test('snapshotsEqual is deep via stable stringify', () => {
		const a = snapshotDocx({
			blocks: [{ type: 'paragraph', runs: [{ text: 'hi', bold: true }] }],
		});
		const b = snapshotDocx({
			blocks: [{ type: 'paragraph', runs: [{ bold: true, text: 'hi' }] }],
		});
		expect(snapshotsEqual(a, b)).toBe(true);
		const p = snapshotPptx([
			{ elements: [{ id: 'e1', x: 1, y: 2, width: 3, height: 4, text: 't' }] },
		]);
		expect(p.slides[0].elements[0].text).toBe('t');
	});
});
