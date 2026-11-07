// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { median, p95 } from '../stats.mjs';
import { MIN_N, STATUSES, classifyRawRow, indexProvenance, statusFor, cellText, aggregateText, insufficient } from '../provenance.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const read = (rel) => readFileSync(join(here, '..', rel), 'utf8');
const jsonl = (rel) => read(rel).split('\n').filter(Boolean).map((l) => JSON.parse(l));
const fmt = (v) => v.toFixed(0);

test('classifyRawRow', () => {
  // Catches: an engine error or a password prompt being reported as a timeout (and vice versa)
  assert.equal(classifyRawRow({ error: 'page.waitForFunction: Timeout 300000ms exceeded.' }), 'observed_timeout');
  assert.equal(classifyRawRow({ error: 'x/ts: Error: perf host: timed out waiting for xlsx grid cells' }), 'observed_timeout');
  assert.equal(classifyRawRow({ error: 'gen-docx-xl-a/ts: Error: word/document.xml exceeds the 25 MiB limit' }), 'observed_error');
  assert.equal(classifyRawRow({ error: '' }), 'unknown');
});

test('statusFor never guesses', () => {
  // Catches: a failed cell with no provenance record silently shown as a timeout
  const prov = indexProvenance({ cells: [{ fixture: 'a', mode: 'warm', cpu_throttle: 4, source: 'lxl-sweep', status: 'not_run' }] });
  assert.equal(statusFor(prov, 'a', 'warm', 4), 'not_run');
  assert.equal(statusFor(prov, 'a', 'warm', 1), 'unknown');
  assert.equal(statusFor(prov, 'b', 'warm', 4), 'unknown');
});

test('a later source overrides an earlier one for the same cell', () => {
  // Catches: the supplemental re-run status being masked by the original sweep status
  const prov = indexProvenance({ cells: [
    { fixture: 'a', mode: 'cold', cpu_throttle: 1, source: 'sm-rerun', status: 'observed_error' },
    { fixture: 'a', mode: 'cold', cpu_throttle: 1, source: 'sm-sweep', status: 'observed_timeout' },
  ] });
  assert.equal(statusFor(prov, 'a', 'cold', 1), 'observed_error');
});

test('cellText gives no number below MIN_N', () => {
  // Catches: a median/p95 (or CI) being claimed from n=3 samples
  const few = [100, 200, 300];
  const t = cellText(few, median, p95, fmt);
  assert.equal(t, 'insufficient samples (n=3)');
  assert.doesNotMatch(t, /\d{3}/);
  assert.equal(cellText(undefined, median, p95, fmt), 'no samples');
  assert.equal(cellText([], median, p95, fmt), 'no samples');
  const ok = cellText([1, 2, 3, 4, 5], median, p95, fmt);
  assert.match(ok, /^3 \/ 5 \(n=5\)$/);
  assert.equal(MIN_N, 5);
});

test('aggregateText leaves out thin fixtures and reports n', () => {
  // Catches: a tier aggregate silently averaging in a fixture with 3 samples
  const full = [10, 11, 12, 13, 14];
  const items = [{ vals: full }, { vals: [1, 2, 3] }];
  const t = aggregateText(items, median, fmt);
  assert.match(t, /^12 \(12\.\.12\) n=5, 1\/2 fixtures omitted/);
  assert.match(aggregateText([{ vals: [1, 2, 3] }], median, fmt), /^insufficient samples \(n=3\)$/);
  assert.equal(aggregateText([{ vals: undefined }], median, fmt), '-');
  assert.equal(insufficient(0), 'no samples');
});

test('failure-provenance.json: every cell has a valid status and an evidence pointer', () => {
  // Catches: a status without evidence, or a status outside the five allowed values
  const j = JSON.parse(read('baselines/failure-provenance.json'));
  assert.ok(j.cells.length > 0);
  for (const c of j.cells) {
    assert.ok(STATUSES.includes(c.status), `${c.fixture} ${c.mode} ${c.cpu_throttle}: ${c.status}`);
    assert.ok(c.evidence?.length, `${c.fixture} ${c.mode} ${c.cpu_throttle} has no evidence`);
  }
});

test('never-started cells are not recorded as timeouts', () => {
  // Catches: HoP finding that predicted/skipped warm cells were labelled "timeout" in the excluded table
  const j = JSON.parse(read('baselines/failure-provenance.json'));
  const starts = jsonl('baselines/box-conditions.jsonl');
  const started = (c) => starts.some((r) => r.fixture_id === c.fixture && r.mode === c.mode && r.cpu === c.cpu_throttle);
  for (const c of j.cells.filter((x) => x.status === 'not_run')) assert.ok(!started(c), `${c.fixture} ${c.mode} ${c.cpu_throttle}x has a start sample but is not_run`);
  const by = (f, m, cpu) => j.cells.find((c) => c.source === 'lxl-sweep' && c.fixture === f && c.mode === m && c.cpu_throttle === cpu)?.status;
  assert.equal(by('gen-pptx-xl-a', 'warm', 1), 'not_run');
  assert.equal(by('gen-pptx-xl-a', 'cold', 4), 'operator_aborted');
  assert.equal(by('gen-pptx-xl-a', 'cold', 1), 'observed_timeout');
  assert.equal(by('gen-docx-pages-500', 'warm', 1), 'operator_aborted');
  assert.equal(by('gen-docx-pages-500', 'warm', 4), 'not_run');
  assert.equal(by('gen-docx-pages-500', 'cold', 4), 'observed_timeout');
});

test('BASELINE.md shows status per cell, n, and the min-n rule', () => {
  // Catches: the report going back to one "timeout" label for cells that were never run, or showing n<5 numbers
  const md = read('BASELINE.md');
  const row = (id) => md.split('\n').find((l) => l.startsWith(`| ${id} |`) && l.includes('observed') || l.startsWith(`| ${id} |`) && l.includes('not run'));
  const xl = row('gen-pptx-xl-a');
  assert.match(xl, /\| observed timeout \| operator aborted \| not run \| not run \|/);
  const pages = row('gen-docx-pages-500');
  assert.match(pages, /\| ok \(n=3\) \| observed timeout \| operator aborted \| not run \|/);
  assert.match(md, /no speed number, ranking, tier aggregate or 95% CI is reported for a cell with n < 5/);
  assert.match(md, /insufficient samples \(n=3\)/);
  assert.doesNotMatch(md, /pending #117/);
});
