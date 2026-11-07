// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { baselineFor, loadEntries } from '../gate.mjs';
import { groupId, rowAsEntry, groupsFromRows, groupsFromHistory, loadReportGroups, nMismatches, rerunFlags } from '../protocol-rows.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const perf = join(here, '..');
const manifest = JSON.parse(readFileSync(join(perf, '../fixtures/manifest.json'), 'utf8')).fixtures;
const info = (id) => manifest.find((m) => m.id === id) ?? manifest.find((m) => m.id === id.replace(/\.[a-z]+$/, ''));
const hist = loadEntries(join(perf, 'history.jsonl')).filter((e) => !e.n_fixtures && info(e.fixture_id));
const report = loadReportGroups(join(perf, 'baselines'), manifest, '8961e71');
const gate = groupsFromHistory(hist);

const FOUR = [
  'real-xlsx-poi-WithEmbeded|cold|1',
  'real-xlsx-usgov-census-NST-EST2024-POP|cold|1',
  'gen-xlsx-cells-100k-b|cold|4',
  'real-xlsx-poi-45540_classic_Footer|cold|4',
];

test('report, gate key and history agree on per-group n in every cell', () => {
  // Catches: the report (rows) and the gate/inventory (history) grouping samples differently, e.g. pooling in one of them
  assert.deepEqual(nMismatches(report, gate), []);
  // group ids are identical wherever history records the run's own host (everything except the L/XL sweep entries)
  for (const [k, m] of report) {
    if (['L', 'XL'].includes(info(k.split('|')[0]).tier)) continue;
    assert.deepEqual([...m.keys()].sort(), [...gate.get(k).keys()].sort(), k);
  }
});

test('the gate\'s baseline for a group never contains another group\'s entries, and its n equals the group n', () => {
  // Catches: the gate pooling the original sweep and the scroll-fixed re-run of one fixture-cell
  for (const k of FOUR) {
    const [fixture, mode, cpu] = k.split('|');
    for (const [g, n] of gate.get(k)) {
      const cur = { ...hist.find((e) => e.fixture_id === fixture && e.mode === mode && String(e.cpu_throttle) === cpu && groupId(e) === g), commit: 'new' };
      const { comparable } = baselineFor(cur, hist, 99);
      assert.ok(comparable.length > 0);
      assert.ok(comparable.every((e) => groupId(e) === g), `${k} ${g}`);
      assert.equal(comparable.reduce((a, e) => a + e.n, 0), n);
    }
  }
});

test('the four xlsx cells that used to be added together are split into n=1 and n=10', () => {
  // Catches: orig n=1 + re-run n=10 being reported as one cell with n=11 (full n reached by pooling incompatible runs)
  for (const k of FOUR) {
    assert.deepEqual([...report.get(k).values()].sort((a, b) => a - b), [1, 10], `report ${k}`);
    assert.deepEqual([...gate.get(k).values()].sort((a, b) => a - b), [1, 10], `history ${k}`);
  }
  const full = readFileSync(join(perf, 'BASELINE.md'), 'utf8');
  assert.doesNotMatch(full, /n=11/);
  const md = full.slice(full.indexOf('## Every fixture'));
  for (const id of ['real-xlsx-poi-WithEmbeded', 'gen-xlsx-cells-100k-b']) {
    const rows = md.split('\n').filter((l) => l.startsWith(`| ${id} | P`));
    assert.equal(rows.length, 2, id);
    assert.ok(rows.some((l) => /n=1, thin sample/.test(l)) && rows.some((l) => /\(n=10\)/.test(l)), id);
  }
  const inv = readFileSync(join(perf, 'baselines/ISSUE-10-CELL-INVENTORY.md'), 'utf8');
  assert.match(inv, /were added together \(n=1 \+ n=10 = 11\)/);
});

test('generic: rows that differ in any protocol field are separate groups, never pooled', () => {
  // Catches: any gate protocol field being ignored when grouping samples
  const base = { fixture: 'f', mode: 'cold', cpuThrottle: 1, chrome: '156', host: { kind: 'box', id: 'b1', os: 'linux', arch: 'x64', cpu_model: 'x', cores: 8, memory_gb: 16, node: 'v20', concurrent_runs: 2 }, metrics: {} };
  const variants = {
    rerun: [base, false, base, true],
    chrome: [base, false, { ...base, chrome: '157' }, false],
    fixture_sha256: [base, false, { ...base, fixture_sha256: 'aa' }, false],
    'host.id': [base, false, { ...base, host: { ...base.host, id: 'b2' } }, false],
    'host.concurrent_runs': [base, false, { ...base, host: { ...base.host, concurrent_runs: 1 } }, false],
    'host.node': [base, false, { ...base, host: { ...base.host, node: 'v22' } }, false],
  };
  for (const [field, [a, fa, b, fb]] of Object.entries(variants)) {
    const rows = [...Array(10).fill(a), ...Array(10).fill(b)];
    const flags = [...Array(10).fill(fa), ...Array(10).fill(fb)];
    const m = groupsFromRows(rows, flags).get('f|cold|1');
    assert.equal(m.size, 2, field);
    assert.deepEqual([...m.values()], [10, 10], field); // never 20
  }
  // identical protocol: one group
  assert.deepEqual([...groupsFromRows(Array(7).fill(base), Array(7).fill(false)).get('f|cold|1').values()], [7]);
  assert.equal(groupId(rowAsEntry(base, false)), groupId(rowAsEntry({ ...base }, false)));
});

test('the assembled dataset layout is verified, so a row cannot be mislabelled', () => {
  // Catches: tagging rows as original/re-run by position after the combined file changed
  const rows = readFileSync(join(perf, 'baselines/ts-combined-8961e71.jsonl'), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));
  assert.equal(rerunFlags(rows, join(perf, 'baselines/ts-combined-8961e71.jsonl'), manifest).filter(Boolean).length, 1121);
  assert.throws(() => rerunFlags(rows.slice(1), join(perf, 'baselines/ts-combined-8961e71.jsonl'), manifest), /not original \+ promoted/);
});
