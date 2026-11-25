// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { scrollToText } from '../lazy/scroll-text.js';

test('invalid occurrence cannot wrap around the find engine', async () => {
  // Catches: negative, fractional or infinite occurrence silently wrapping to another match.
  const el = { document: {}, shellAction() { throw new Error('must not search'); } };
  for (const occurrence of [-1, 0.5, Infinity, NaN]) {
    await assert.rejects(scrollToText(el, 'word', { occurrence }), RangeError);
  }
  assert.equal(await scrollToText(el, ''), null);
});
