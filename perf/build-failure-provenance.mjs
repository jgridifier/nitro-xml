#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Build perf/baselines/failure-provenance.json (#10): one status + evidence pointer per excluded cell.
 * Raw evidence (chunk results, failures.jsonl, sweep.log, operator transcript) is only read, never edited.
 *
 *   node perf/build-failure-provenance.mjs [--lxl-dir perf/results/lxl-sweep] [--out perf/baselines/failure-provenance.json]
 *
 * S/M cells come from the raw harness failure rows in perf/baselines (status from the row's own error text).
 * L/XL cells come from the chunk result files; chunks that have no result file are classified from the
 * operator events below, each with a quote and a pointer. Anything not determinable would be `unknown`.
 */
import { readFileSync, existsSync, writeFileSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join, resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { homedir } from 'node:os';
import { classifyRawRow } from './provenance.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const argv = process.argv.slice(2);
const opt = (n, d) => (argv.includes(n) ? argv[argv.indexOf(n) + 1] : d);
const lxlDir = resolve(opt('--lxl-dir', join(here, 'results/lxl-sweep')));
const outFile = resolve(opt('--out', join(here, 'baselines/failure-provenance.json')));
const rel = (p) => relative(root, p);
const sha = (p) => createHash('sha256').update(readFileSync(p)).digest('hex');
const lines = (f) => readFileSync(f, 'utf8').split('\n').map((l, i) => [i + 1, l]).filter(([, l]) => l.trim()).map(([n, l]) => [n, JSON.parse(l)]);

const cells = [];
const files = new Map(); // evidence file -> sha256 (so edits to the evidence are detectable)
const track = (p, tracked) => { files.set(p, { sha256: sha(p), tracked }); };

// ---- S/M: raw harness rows (committed) --------------------------------------------------------------
for (const [file, source] of [['ts-4451709.failures.jsonl', 'sm-sweep'], ['ts-4451709.rerun-failures.jsonl', 'sm-rerun']]) {
  const p = join(here, 'baselines', file);
  track(p, true);
  const seen = new Map();
  for (const [n, r] of lines(p)) {
    if (!r.fixture) continue;
    const k = `${r.fixture}|${r.mode}|${r.cpuThrottle ?? 1}`;
    if (seen.has(k)) { seen.get(k).evidence.push({ file: `perf/baselines/${file}`, line: n }); continue; }
    const c = {
      fixture: r.fixture, mode: r.mode, cpu_throttle: r.cpuThrottle ?? 1, source,
      status: classifyRawRow(r),
      basis: 'raw harness failure row written from a completed chunk result; status from its error text',
      error: String(r.error).split('\n')[0].slice(0, 160),
      evidence: [{ file: `perf/baselines/${file}`, line: n }],
    };
    seen.set(k, c); cells.push(c);
  }
}

// ---- L/XL ---------------------------------------------------------------------------------------------
const need = (p) => { if (!existsSync(p)) throw new Error(`missing evidence file ${p}; run from the worktree that holds perf/results/lxl-sweep`); return p; };
const sweepLog = need(join(lxlDir, 'sweep.log'));
track(sweepLog, false);
const logLines = readFileSync(sweepLog, 'utf8').split('\n');
const logLine = (re) => { const i = logLines.findIndex((l) => re.test(l)); if (i < 0) throw new Error(`sweep.log has no line ${re}`); return { file: rel(sweepLog), line: i + 1, text: logLines[i] }; };
const noLogLine = (re) => { if (logLines.some((l) => re.test(l))) throw new Error(`sweep.log unexpectedly has ${re}`); };

const boxFile = join(here, 'baselines/box-conditions.jsonl');
track(boxFile, true);
const box = lines(boxFile);
const boxEv = (fixture, mode, cpu, after) => {
  const hit = box.find(([, r]) => r.fixture_id === fixture && r.mode === mode && r.cpu === cpu && r.timestamp >= after);
  if (!hit) throw new Error(`box-conditions has no ${fixture} ${mode} ${cpu}x after ${after}`);
  return { file: 'perf/baselines/box-conditions.jsonl', line: hit[0], text: `${hit[1].timestamp} per-fixture start sample for ${fixture}` };
};

const lxlFailFile = join(here, 'baselines/ts-lxl-8961e71.failures.jsonl');
track(lxlFailFile, true);
const lxlFail = lines(lxlFailFile);
const failRow = (fixture, mode, cpu, re) => {
  const hit = lxlFail.find(([, r]) => r.fixture === fixture && r.mode === mode && r.cpuThrottle === cpu && (!re || re.test(r.error)));
  return hit ? { file: 'perf/baselines/ts-lxl-8961e71.failures.jsonl', line: hit[0] } : null;
};

const TRANSCRIPT = join(homedir(), '.claude/projects/-workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl');
const tr = (line, quote) => ({ file: '~/.claude/projects/-workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl', line, tracked: false, text: quote });
if (existsSync(TRANSCRIPT)) {
  const t = readFileSync(TRANSCRIPT, 'utf8').split('\n');
  for (const [n, re] of [[2089, /touch .*cold-4x-002/], [2104, /kill -9 4066898/], [2295, /touch perf\/results\/lxl-sweep\/warm-1x-000/], [2307, /kill -9 4088154/]]) {
    if (!re.test(t[n - 1] ?? '')) throw new Error(`operator transcript line ${n} no longer matches ${re}`);
  }
  track(TRANSCRIPT, false);
}

const CHUNKS = [
  ['gen-docx-pages-500', 'gen-docx-pages-500-b', 'gen-docx-pages-500-c', 'gen-docx-xl-a'],
  ['gen-docx-xl-b', 'gen-docx-xl-c', 'gen-pptx-slides-200', 'gen-pptx-slides-200-b'],
  ['gen-pptx-slides-200-c', 'gen-pptx-xl-a', 'gen-pptx-xl-b', 'gen-pptx-xl-c'],
  ['real-pptx-poi-customGeo', 'real-pptx-python-pptx-cht-chart-type', 'upstream-61fc791e15c0', 'gen-xlsx-cells-1m'],
  ['gen-xlsx-cells-1m-pad-l', 'gen-xlsx-cells-1m-pad-l2', 'gen-xlsx-xl-a', 'gen-xlsx-xl-b'],
  ['gen-xlsx-xl-c'],
];
const CELLS = [['cold', 1], ['cold', 4], ['warm', 1], ['warm', 4]];
const chunkFile = (mode, cpu, n) => join(lxlDir, `${mode}-${cpu}x-${String(n).padStart(3, '0')}`);
const add = (fixture, mode, cpu, status, basis, evidence, extra = {}) => cells.push({ fixture, mode, cpu_throttle: cpu, source: 'lxl-sweep', status, basis, ...extra, evidence });

// measured sample counts per cell from the merged L/XL rows
const lxlRows = lines(join(here, 'baselines/ts-lxl-8961e71.jsonl')).map(([, r]) => r);
const measuredN = (fx, mode, cpu) => lxlRows.filter((r) => r.fixture === fx && r.mode === mode && r.cpuThrottle === cpu).length;

for (const [mode, cpu] of CELLS) for (const [ci, fixtures] of CHUNKS.entries()) {
  const base = chunkFile(mode, cpu, ci);
  const hasJson = existsSync(`${base}.json`);
  const chunkName = `${mode}-${cpu}x-${String(ci).padStart(3, '0')}`;
  if (hasJson) {
    track(`${base}.json`, false);
    const j = JSON.parse(readFileSync(`${base}.json`, 'utf8'));
    for (const fx of fixtures) {
      const f = j.failures.find((x) => x.fixture === fx);
      if (!f) continue;
      const status = classifyRawRow(f);
      add(fx, mode, cpu, status, 'harness chunk result file lists this fixture under failures[]', [
        { file: rel(`${base}.json`), pointer: `failures[fixture=${fx}]`, text: String(f.error).split('\n')[0].slice(0, 120) },
        ...[failRow(fx, mode, cpu, /^(?!.*pre-created)/)].filter(Boolean),
      ], { error: String(f.error).split('\n')[0].slice(0, 160), measured_n: measuredN(fx, mode, cpu) });
    }
    continue;
  }
  // No result file for this chunk: events recorded by the operator transcript and box-conditions.
  const emptyMarker = `${base}.jsonl`;
  if (!existsSync(emptyMarker) || statSync(emptyMarker).size !== 0) throw new Error(`${emptyMarker} should be an empty marker`);
  track(emptyMarker, false);
  if (chunkName === 'cold-4x-002') {
    const log = logLine(/cold 4x chunk 2\/6 exit null/);
    add('gen-pptx-slides-200-c', mode, cpu, 'observed_timeout', 'run.mjs printed its own FAIL line for this fixture (Timeout 300000ms); its start and the next fixture start are 301 s apart in box-conditions', [
      (() => { const h = lxlFail.find(([, r]) => !r.fixture && /^FAIL gen-pptx-slides-200-c ts: .*Timeout 300000ms/.test(r.error ?? '') && r.mode === 'cold' && r.cpuThrottle === 4); if (!h) throw new Error('FAIL row for slides-200-c missing'); return { file: 'perf/baselines/ts-lxl-8961e71.failures.jsonl', line: h[0], text: 'stderr FAIL line captured by the sweep (no fixture field)' }; })(),
      boxEv('gen-pptx-slides-200-c', 'cold', 4, '2026-11-06T02:02'), boxEv('gen-pptx-xl-a', 'cold', 4, '2026-11-06T02:07'), log,
    ], { error: 'page.waitForFunction: Timeout 300000ms exceeded.' });
    add('gen-pptx-xl-a', mode, cpu, 'operator_aborted', 'fixture started (box-conditions) and the chunk process was killed with kill -9 about 23 s later; no result written', [
      boxEv('gen-pptx-xl-a', 'cold', 4, '2026-11-06T02:07'), tr(2104, 'kill -9 4066898  (Killed PID 4066898 at Fri Nov  6 02:08:00 UTC 2026)'), log,
    ]);
    for (const fx of ['gen-pptx-xl-b', 'gen-pptx-xl-c']) {
      if (box.some(([, r]) => r.fixture_id === fx && r.mode === 'cold' && r.cpu === 4)) throw new Error(`${fx} cold 4x has a start sample`);
      add(fx, mode, cpu, 'not_run', 'chunk process was killed while on the previous fixture; no per-fixture start sample for this one', [log, tr(2104, 'kill -9 4066898')], { note: 'the failures.jsonl row calling this a synchronous JS hang is an operator annotation, not an observation' });
    }
    continue;
  }
  if (chunkName === 'warm-1x-000') {
    noLogLine(/warm 1x chunk 0\/6 exit 0/);
    const log = logLine(/warm 1x chunk 0\/6 exit null/);
    const ev = [log, tr(2307, 'kill -9 4088154  (run.mjs --mode warm --cpu-throttle 1 --fixtures gen-docx-pages-500,-b,-c,gen-docx-xl-a)'), tr(2295, 'touch warm-1x-000.jsonl "to skip the stuck/slow chunks" (written after the chunk had been running ~5.6 h)')];
    for (const fx of ['gen-docx-pages-500', 'gen-docx-pages-500-b', 'gen-docx-pages-500-c']) {
      add(fx, mode, cpu, 'operator_aborted', 'fixture started (box-conditions) and the chunk process was killed with kill -9 (sweep.log exit null); run.mjs writes its result file only at the end, so nothing was saved', [boxEv(fx, 'warm', 1, '2026-11-06T05:44'), ...ev], { note: 'whether this fixture finished its iterations before the kill cannot be determined; no timeout was observed. The "render avg 226s" style numbers in failures.jsonl are operator estimates with no raw source' });
    }
    if (box.some(([, r]) => r.fixture_id === 'gen-docx-xl-a' && r.mode === 'warm' && r.cpu === 1)) throw new Error('docx-xl-a warm 1x has a start sample');
    add('gen-docx-xl-a', mode, cpu, 'not_run', 'chunk was killed while on gen-docx-pages-500-c; no per-fixture start sample for this fixture', ev, { note: 'the 25 MiB limit is observed for this fixture at cold 1x and cold 4x only; not observed warm' });
    continue;
  }
  // Remaining marker-only chunks were created before they could start: sweep.log has no line for them.
  const m = /^(cold|warm)-(\d)x-(\d+)$/.exec(chunkName);
  noLogLine(new RegExp(`${m[1]} ${m[2]}x chunk ${Number(m[3])}/6`));
  const created = chunkName === 'warm-1x-002' || chunkName === 'warm-4x-002' ? tr(2089, 'touch cold-4x-002.jsonl / warm-1x-002.jsonl / warm-4x-002.jsonl "to skip pptx-xl synchronous hang fixtures"') : tr(2295, 'touch warm-1x-000.jsonl warm-4x-000.jsonl warm-4x-001.jsonl "to skip the stuck/slow chunks"');
  for (const fx of fixtures) {
    if (box.some(([, r]) => r.fixture_id === fx && r.mode === mode && r.cpu === cpu)) throw new Error(`${fx} ${mode} ${cpu}x has a start sample`);
    add(fx, mode, cpu, 'not_run', 'empty marker file created by the operator before the chunk started; sweep.log has no line for the chunk and box-conditions has no start sample', [{ file: rel(emptyMarker), text: '0-byte marker file' }, created], { note: 'any "timeout expected" / "render ~Ns" text in failures.jsonl for this cell is a prediction, not a measurement' });
  }
}

// every L/XL cell without a measured sample must have a status
const lxlFixtures = [...new Set(CHUNKS.flat().concat(lxlRows.map((r) => r.fixture)))];
for (const fx of lxlFixtures) for (const [mode, cpu] of CELLS) {
  if (measuredN(fx, mode, cpu) === 0 && !cells.some((c) => c.fixture === fx && c.mode === mode && c.cpu_throttle === cpu && c.source === 'lxl-sweep')) throw new Error(`no provenance for ${fx} ${mode} ${cpu}x`);
}

// failure rows that cannot be tied to a fixture
const unattributed = lxlFail.filter(([, r]) => !r.fixture).map(([n, r]) => ({ file: 'perf/baselines/ts-lxl-8961e71.failures.jsonl', line: n, status: /^FAIL gen-pptx-slides-200-c/.test(r.error ?? '') ? 'attributed above (cold 4x gen-pptx-slides-200-c)' : 'unknown', note: r.error ? 'stderr FAIL line captured by the sweep' : 'blank error row with no fixture; not attributable to any cell' }));

const out = {
  schema: 'failure-provenance/1',
  statuses: {
    observed_timeout: 'harness actually hit its timeout during a run',
    observed_error: 'run finished with an error (engine limit, throw, password); not a timeout',
    operator_aborted: 'process killed by the operator / OOM / manually before the cell finished',
    not_run: 'never started, including cells skipped because a timeout was predicted',
    unknown: 'provenance not determinable',
  },
  note: 'Raw evidence files are never edited. Files marked tracked:false live under gitignored perf/results or outside the repo; their sha256 is recorded so later edits are detectable.',
  evidence_files: Object.fromEntries([...files].map(([p, v]) => [p.startsWith(homedir()) ? p.replace(homedir(), '~') : rel(p), v]).sort()),
  unattributed,
  cells: cells.sort((a, b) => a.source.localeCompare(b.source) || a.fixture.localeCompare(b.fixture) || a.mode.localeCompare(b.mode) || a.cpu_throttle - b.cpu_throttle),
};
writeFileSync(outFile, JSON.stringify(out, null, 1) + '\n');
const tally = {};
for (const c of out.cells) tally[`${c.source} ${c.status}`] = (tally[`${c.source} ${c.status}`] ?? 0) + 1;
console.log(`wrote ${rel(outFile)}: ${out.cells.length} cells`, tally);
