// SPDX-License-Identifier: MIT
// Catches: volatile runtime ids and float noise flipping parity when the IR is equal.
import { describe, expect, test } from 'bun:test';
import { normalize, normalizeJson } from './normalize.ts';

describe('normalize', () => {
	test('drops volatile id keys and rounds floats to 1e-3', () => {
		const a = {
			uuid: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
			sizePx: 13.333333333,
			nested: { instanceId: 99, w: 1.23456 },
		};
		const b = {
			sizePx: 13.333,
			nested: { w: 1.235 },
			uuid: 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb',
		};
		expect(normalizeJson(a)).toBe(normalizeJson(b));
	});

	test('sorts object keys so insertion order cannot cause a false diff', () => {
		const left = normalize({ z: 1, a: { y: 2, b: 3 } });
		const right = normalize({ a: { b: 3, y: 2 }, z: 1 });
		expect(JSON.stringify(left)).toBe(JSON.stringify(right));
	});
});
