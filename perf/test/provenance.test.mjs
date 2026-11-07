// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { median, p95 } from '../stats.mjs';
import { DESIGN_N, STATUSES, classifyRawRow, indexProvenance, statusFor, cellText, medianText, aggregateText, nLabel } from '../provenance.mjs';

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

test('thin cells keep their measured median and are labelled, never suppressed', () => {
  // Catches: an invented n cut-off hiding measured medians, or n=3 passing as a normal sample
  const few = [100, 200, 300];
  assert.equal(cellText(few, median, p95, fmt, 'cold'), '200 / 290 (n=3, thin sample)');
  assert.equal(cellText(few, median, p95, fmt, 'warm'), '200 / 290 (n=3, thin sample)');
  assert.equal(cellText(Array.from({ length: 10 }, (_, i) => i), median, p95, fmt, 'cold'), '5 / 9 (n=10)');
  assert.equal(cellText(Array.from({ length: 10 }, (_, i) => i), median, p95, fmt, 'warm'), '5 / 9 (n=10, thin sample)'); // warm design is 20
  assert.equal(cellText(undefined, median, p95, fmt, 'cold'), 'no samples');
  assert.equal(cellText([], median, p95, fmt, 'warm'), 'no samples');
  assert.equal(medianText(few, median, fmt, 'cold'), '200 (n=3, thin sample)');
  assert.deepEqual(DESIGN_N, { cold: 10, warm: 20 }); // the harness defaults in perf/run.mjs, not a new threshold
  assert.equal(nLabel(0, 'cold'), 'no samples');
});

test('aggregateText shows every fixture, the n range and how many are thin', () => {
  // Catches: dropping thin fixtures from an aggregate, or hiding that it rests on n=3
  const ten = Array.from({ length: 10 }, (_, i) => 10 + i);
  const t = aggregateText([{ vals: ten }, { vals: [1, 2, 3] }, { vals: undefined }], median, fmt, 'cold');
  assert.match(t, /^\d+ \(2\.\.15\) n=3–10, thin sample in 1\/2 fixtures, 1 fixtures no samples$/);
  assert.equal(aggregateText([{ vals: undefined }], median, fmt, 'cold'), 'no samples');
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

test('BASELINE.md shows status per cell, n, thin labels, and makes no CI or win claim', () => {
  // Catches: the report going back to one "timeout" label for never-run cells, an invented n cut-off, or a CI/win claim
  const md = read('BASELINE.md');
  const row = (id) => md.split('\n').find((l) => l.startsWith(`| ${id} |`) && /observed|not run/.test(l));
  assert.match(row('gen-pptx-xl-a'), /\| observed timeout \| operator aborted \| not run \| not run \|/);
  assert.match(row('gen-docx-pages-500'), /\| ok \(n=3\) \| observed timeout \| operator aborted \| not run \|/);
  assert.match(md, /n=3, thin sample/);
  assert.doesNotMatch(md, /insufficient samples|n < 5|minimum-n/);
  assert.match(md, /empirical 95th percentile/);
  assert.match(md, /computes \*\*no 95% confidence interval\*\* /);
  assert.match(md, /no speed-win or WASM-vs-TS claim/);
  assert.doesNotMatch(md, /pending #117/);
  // every table cell with a p95 carries an n
  for (const l of md.split('\n').filter((x) => / \/ [\d.]+ \(/.test(x))) assert.match(l, /\(n=\d+/);
});
