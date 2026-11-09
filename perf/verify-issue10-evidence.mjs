#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Verify the committed #10 evidence subset (perf/baselines/issue-10-evidence/) using only committed files:
 *   1. every file's sha256 and size match MANIFEST.json (and nothing unlisted is present);
 *   2. every classified cell in cell-evidence.json points at evidence that exists in the subset;
 *   3. each cell's status is RE-DERIVED from that evidence (not read from the claim) and must equal the claim;
 *   4. the status totals re-derived here equal the ones in perf/baselines/failure-provenance.json.
 *   --sources   also compare the recorded source_sha256 of each original with the file on disk, when present.
 *
 *   node perf/verify-issue10-evidence.mjs [--sources]       exit 1 on any failure
 */
import { readFileSync, readdirSync, existsSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { homedir } from 'node:os';
import { classifyRawRow, STATUSES } from './provenance.mjs';
import { sourceHost } from './aggregate.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const dir = process.env.ISSUE10_EVIDENCE_DIR ?? join(here, 'baselines/issue-10-evidence');
const problems = [];
const bad = (m) => problems.push(m);
const read = (rel) => readFileSync(join(dir, rel), 'utf8');
const sha = (b) => createHash('sha256').update(b).digest('hex');
const jsonl = (rel) => read(rel).split('\n').filter(Boolean).map((l) => JSON.parse(l));

// 1. manifest
const manifest = JSON.parse(read('MANIFEST.json'));
const listed = new Set(manifest.files.map((f) => f.file));
for (const f of manifest.files) {
  const p = join(dir, f.file);
  if (!existsSync(p)) { bad(`missing ${f.file}`); continue; }
  const buf = readFileSync(p);
  if (sha(buf) !== f.sha256) bad(`sha256 mismatch ${f.file}`);
  if (buf.length !== f.bytes) bad(`size mismatch ${f.file}`);
}
const walk = (d, pre = '') => readdirSync(join(dir, d), { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(d, e.name), `${pre}${e.name}/`) : [`${pre}${e.name}`]));
for (const f of walk('.')) if (!listed.has(f) && !['MANIFEST.json', 'README.md'].includes(f)) bad(`unlisted file ${f}`);
if (!existsSync(join(dir, 'README.md'))) bad('README.md missing');
if (process.argv.includes('--sources')) {
  for (const f of manifest.files) {
    const p = f.source.startsWith('~/') ? join(homedir(), f.source.slice(2)) : join(root, f.source);
    if (existsSync(p) && sha(readFileSync(p)) !== f.source_sha256) bad(`source changed since extract: ${f.source}`);
    else if (!existsSync(p)) console.log(`note: source not on this machine, skipped: ${f.source}`);
  }
}

// 2/3. cells
const ce = JSON.parse(read('cell-evidence.json')).cells;
const box = jsonl('raw/box-conditions.jsonl');
const sweepLog = read('raw/sweep.log').split('\n');
const lxlFail = jsonl('raw/ts-lxl-8961e71.failures.jsonl');
const markers = JSON.parse(read('extracts/markers.json')).markers.map((m) => m.marker);
const events = jsonl('operator-events.jsonl');
const extract = new Map();
const ex = (name) => { if (!extract.has(name)) extract.set(name, JSON.parse(read(name))); return extract.get(name); };
const rawLine = (rel, n) => { const l = read(rel).split('\n')[n - 1]; return l ? JSON.parse(l) : null; };
const fixtureId = (c) => c.fixture;
const logLineFor = (chunk) => { const m = /^(cold|warm)-(\d)x-(\d+)$/.exec(chunk); return sweepLog.find((l) => new RegExp(` ${m[1]} ${m[2]}x chunk ${Number(m[3])}/6 `).test(l)); };

function derive(c) {
  if (c.source !== 'lxl-sweep') {
    const e = c.evidence.find((x) => x.line && /^raw\/ts-4451709/.test(x.file));
    const row = e && rawLine(e.file, e.line);
    if (!row || row.fixture !== c.fixture || row.mode !== c.mode || (row.cpuThrottle ?? 1) !== c.cpu_throttle) return 'unknown';
    return classifyRawRow(row);
  }
  for (const e of c.evidence) {
    if (e.pointer) {
      const f = ex(e.file).failures.find((x) => x.fixture === c.fixture);
      if (f && ex(e.file).meta.mode === c.mode && ex(e.file).meta.cpuThrottle === c.cpu_throttle) return classifyRawRow(f);
    }
  }
  for (const e of c.evidence) {
    if (e.line && e.file === 'raw/ts-lxl-8961e71.failures.jsonl') {
      const r = rawLine(e.file, e.line);
      if (r && !r.fixture && r.mode === c.mode && r.cpuThrottle === c.cpu_throttle && new RegExp(`^FAIL ${fixtureId(c)} ts: `).test(r.error)) return classifyRawRow({ error: r.error.replace(/^FAIL \S+ ts: /, '') });
    }
  }
  const started = box.some((r) => r.fixture_id === c.fixture && r.mode === c.mode && r.cpu === c.cpu_throttle);
  const logLine = logLineFor(c.chunk);
  const killed = !!logLine && /exit null/.test(logLine);
  const killEvent = events.some((e) => /kill( -9| -SIGKILL)/.test(e.command));
  if (started && killed && killEvent) return 'operator_aborted';
  if (!started && killed && killEvent) return 'not_run';
  if (!started && !logLine && markers.includes(`${c.chunk}.jsonl`)) return 'not_run';
  return 'unknown';
}

const derived = {};
for (const c of ce) {
  if (!c.evidence?.length) { bad(`no evidence: ${c.fixture} ${c.mode} ${c.cpu_throttle}x`); continue; }
  for (const e of c.evidence) {
    if (!listed.has(e.file)) bad(`evidence file not in subset: ${e.file}`);
    else if (e.line && !(read(e.file).split('\n')[e.line - 1] ?? '').trim()) bad(`${e.file}:${e.line} is empty (${c.fixture})`);
    if (e.transcript_line && !events.some((x) => x.transcript_line === e.transcript_line)) bad(`operator event L${e.transcript_line} missing`);
    if (e.marker && !markers.includes(e.marker)) bad(`marker ${e.marker} missing`);
  }
  const d = derive(c);
  if (d !== c.status) bad(`${c.source} ${c.fixture} ${c.mode} ${c.cpu_throttle}x: claimed ${c.status}, evidence derives ${d}`);
  const k = `${c.source} ${d}`;
  derived[k] = (derived[k] ?? 0) + 1;
}

// 4. totals against the provenance file
const prov = JSON.parse(readFileSync(join(here, 'baselines/failure-provenance.json'), 'utf8'));
const provTotals = {};
for (const c of prov.cells) { const k = `${c.source} ${c.status}`; provTotals[k] = (provTotals[k] ?? 0) + 1; }
if (prov.cells.length !== ce.length) bad(`cell count ${ce.length} != provenance ${prov.cells.length}`);
for (const k of new Set([...Object.keys(derived), ...Object.keys(provTotals)])) if ((derived[k] ?? 0) !== (provTotals[k] ?? 0)) bad(`total ${k}: derived ${derived[k] ?? 0}, provenance ${provTotals[k] ?? 0}`);

// 5. history host correction: only the 42 L/XL lines changed, the pre-correction lines are preserved, and the new host is
// exactly what the committed source rows record
const hc = manifest.history_correction;
if (!hc) bad('MANIFEST.history_correction missing');
else {
  const snap = read(hc.snapshot).split('\n').filter(Boolean);
  const histLines = readFileSync(join(here, 'history.jsonl'), 'utf8').split('\n');
  const [lo, hi] = hc.corrected_line_range;
  if (snap.length !== hi - lo + 1) bad(`snapshot has ${snap.length} lines, expected ${hi - lo + 1}`);
  const rebuilt = [...histLines.slice(0, lo - 1), ...snap, ...histLines.slice(hi)].join('\n');
  if (sha(rebuilt) !== hc.pre_fix_history_sha256) bad('history.jsonl with the snapshot put back does not hash to the recorded pre-correction file: some other line changed');
  const src = readFileSync(join(root, 'perf/baselines/ts-lxl-8961e71.jsonl'), 'utf8').split('\n');
  histLines.forEach((l, i) => {
    if (!l) return;
    const e = JSON.parse(l);
    const inRange = i + 1 >= lo && i + 1 <= hi;
    if (!inRange) { if (e.host_import_recorded || e.provenance?.host) bad(`history line ${i + 1} outside the corrected range carries host provenance`); return; }
    const old = JSON.parse(snap[i + 1 - lo]);
    if (JSON.stringify(e.host_import_recorded) !== JSON.stringify(old.host)) bad(`history line ${i + 1}: host_import_recorded != recorded pre-correction host`);
    const rowsAt = (e.provenance?.host?.source_row_lines ?? []).map((n) => JSON.parse(src[n - 1]));
    if (rowsAt.length !== e.n || rowsAt.some((r) => r.fixture !== e.fixture_id || r.mode !== e.mode)) bad(`history line ${i + 1}: source rows do not match the entry`);
    else if (JSON.stringify(sourceHost(rowsAt[0])) !== JSON.stringify(e.host)) bad(`history line ${i + 1}: host != host recorded by its source rows`);
  });
}

const byStatus = Object.fromEntries(STATUSES.map((s) => [s, Object.entries(derived).filter(([k]) => k.endsWith(` ${s}`)).reduce((n, [, v]) => n + v, 0)]));
console.log(`files ${manifest.files.length} (+MANIFEST, README); cells ${ce.length}; re-derived totals:`, derived);
console.log('by status:', byStatus);
if (problems.length) {
  console.error(`FAIL: ${problems.length} problem(s)\n${problems.slice(0, 30).join('\n')}`);
  process.exit(1);
}
console.log('OK: hashes match, every classified cell has evidence in the subset, statuses re-derived and equal to the claims');
