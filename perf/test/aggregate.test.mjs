// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { checkReference } from '../aggregate.mjs';
import { bootstrapCI, median } from '../stats.mjs';

test('aggregation matches the recorded reference dataset', () => {
  // Catches: a percentile or bootstrap change silently shifting every reported median, p95 and CI.
  assert.deepEqual(checkReference(), []);
});

test('bootstrap CI is deterministic and brackets the median', () => {
  // Catches: an unseeded bootstrap making the same JSONL produce a different interval on every run.
  const xs = [12, 15, 11, 19, 14, 13, 40, 12, 16, 15];
  const a = bootstrapCI(xs);
  assert.deepEqual(a, bootstrapCI(xs));
  assert.ok(a[0] <= median(xs) && median(xs) <= a[1]);
});
