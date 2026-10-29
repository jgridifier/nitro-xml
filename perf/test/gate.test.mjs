// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { evaluate, decide, overrideFrom, enginesComparable, regressed, loadEntries } from '../gate.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const th = JSON.parse(readFileSync(join(here, '../thresholds.json'), 'utf8'));
const HOST = { kind: 'box', id: 'b1', os: 'linux', cpu: 'x', cores: 8 };
const ent = (commit, med, o = {}) => ({
  commit, branch: 'main', engine: 'ts', fixture_id: 'docx-S', mode: 'cold', cpu_throttle: 1, chrome_version: '156',
  host: { ...HOST }, metrics: { first_paint_ms: { median: med }, ...(o.metrics ?? {}) }, ...o,
});
const hist = (...meds) => meds.map((m, i) => ent(`h${i}`, m));
const status = (rows, metric = 'first_paint_ms') => rows.find((r) => r.metric === metric).status;

test('a regression must exceed BOTH the relative and the absolute threshold', () => {
  // Catches: OR-ing the thresholds, failing 3 ms noise on a 10 ms metric or 1% on a 5 s metric.
  const c = th.classes.find((x) => x.name === 'time');
  assert.equal(regressed(10, 14, c), false); // +40% but only 4 ms
  assert.equal(regressed(5000, 5100, c), false); // +100 ms but only 2%
  assert.equal(regressed(100, 120, c), true);
  const mem = th.classes.find((x) => x.name === 'memory');
  assert.equal(regressed(10e6, 11.5e6, mem), false); // +15% but 1.5 MB
});

test('baseline is the median of the last 5 main entries only', () => {
  // Catches: using all history (or the mean) so one old fast run or an outlier moves the baseline.
  const h = hist(1000, 1000, 100, 100, 100, 100, 5000, 100);
  const rows = evaluate([ent('cur', 100)], h, th);
  assert.equal(rows[0].base, 100); // last 5 = 100,100,100,5000,100
  assert.equal(status(rows), 'pass');
  const also = evaluate([ent('cur', 130)], h, th);
  assert.equal(status(also), 'fail');
});

test('non-main entries are not baselines and same-commit reruns are skipped', () => {
  // Catches: a PR's own numbers (or a rerun of the commit) becoming its baseline.
  const h = [ent('a', 100), { ...ent('b', 1), branch: 'feature' }, ent('cur', 1)];
  assert.equal(evaluate([ent('cur', 100)], h, th)[0].base, 100);
});

test('different host or Chrome is incomparable, never pass or fail', () => {
  // Catches: subtracting numbers measured on different machines/Chrome builds.
  const other = hist(100, 100).map((e) => ({ ...e, host: { ...HOST, id: 'ci-7', kind: 'ci' } }));
  assert.equal(status(evaluate([ent('cur', 900)], other, th)), 'incomparable');
  const chrome = hist(100).map((e) => ({ ...e, chrome_version: '157' }));
  assert.equal(status(evaluate([ent('cur', 900)], chrome, th)), 'incomparable');
  assert.equal(decide(evaluate([ent('cur', 900)], other, th)).ok, true);
  assert.equal(status(evaluate([ent('cur', 900)], [], th)), 'no-baseline');
});

test('WASM-vs-TS only compares on the same host+Chrome or the same CI job', () => {
  // Catches: a WASM number from one machine "beating" a TS number from another.
  const a = ent('x', 1), b = { ...ent('x', 1), engine: 'wasm' };
  assert.equal(enginesComparable(a, b), true);
  assert.equal(enginesComparable(a, { ...b, host: { ...HOST, id: 'b2' } }), false);
  assert.equal(enginesComparable(a, { ...b, chrome_version: '157' }), false);
  const ci = (id) => ({ ...a, host: { kind: 'ci', id, os: 'l', cpu: 'c', cores: 4 }, ci_job: '99' });
  assert.equal(enginesComparable(ci('r1'), { ...ci('r2'), engine: 'wasm' }), true);
  assert.equal(enginesComparable(ci('r1'), { ...ci('r2'), engine: 'wasm', ci_job: '100' }), false);
});

test('disarmed cells are report-only and never fail', () => {
  // Catches: a bimodal/unexplained cell (xlsx-M warm 1x TTI) failing PRs at random.
  const cur = { ...ent('cur', 1, { metrics: { tti_ms: { median: 900 } } }), fixture_id: 'xlsx-M', mode: 'warm' };
  const base = hist(100, 100, 100).map((e) => ({ ...e, fixture_id: 'xlsx-M', mode: 'warm', metrics: { tti_ms: { median: 100 } } }));
  const rows = evaluate([cur], base, th);
  assert.equal(status(rows, 'tti_ms'), 'report-only');
  assert.equal(decide(rows).ok, true);
  assert.equal(status(evaluate([{ ...cur, mode: 'cold' }], base.map((e) => ({ ...e, mode: 'cold' })), th), 'tti_ms'), 'fail');
});

test('override needs the label AND a written reason, and is logged', () => {
  // Catches: the label alone silently waving through a regression.
  const rows = evaluate([ent('cur', 500)], hist(100, 100, 100), th);
  assert.equal(decide(rows).ok, false);
  const body = 'perf-regression-accepted: new parser trades 40ms for lossless save';
  const v = decide(rows, overrideFrom({ labels: ['perf-regression-accepted'], body }));
  assert.deepEqual([v.ok, v.overridden], [true, true]);
  assert.match(v.reason, /lossless save/);
  assert.equal(decide(rows, overrideFrom({ labels: ['perf-regression-accepted'], body: 'pls' })).ok, false);
  assert.equal(decide(rows, overrideFrom({ labels: [], body })).ok, false);
});

test('noise: gating the same data twice passes', () => {
  // Catches: false positives when nothing changed (including the seeded history against itself).
  const h = hist(100, 103, 98, 101, 99);
  const cur = ent('cur', 101);
  assert.equal(decide(evaluate([cur], h, th)).ok, true);
  assert.equal(decide(evaluate([cur], h, th)).ok, true);
  const seeded = loadEntries(join(here, '../history.jsonl'));
  const rerun = seeded.map((e) => ({ ...e, commit: 'rerun' }));
  const rows = evaluate(rerun, seeded, th);
  assert.equal(rows.filter((r) => r.status === 'fail').length, 0);
  assert.ok(rows.some((r) => r.status === 'pass'));
});
