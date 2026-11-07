#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Build the committed, sanitized evidence subset perf/baselines/issue-10-evidence/ (#10) from the raw sources:
 * the gitignored perf/results/lxl-sweep/ chunk files and the operator's Claude transcript. Sources are only read.
 *
 *   node perf/build-issue10-evidence.mjs [--lxl-dir perf/results/lxl-sweep] [--transcript <session.jsonl>]
 *
 * Anyone without those sources can still run `node perf/verify-issue10-evidence.mjs` on the committed subset.
 * Full small files are copied byte for byte (same line numbers as the source); big ones (chunk results with
 * per-iteration long-task arrays, the transcript) are extracts with source line numbers. Redactions are counted
 * in MANIFEST.json and the extract is re-scanned afterwards.
 */
import { readFileSync, writeFileSync, mkdirSync, existsSync, statSync, copyFileSync, rmSync } from 'node:fs';
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
const TRANSCRIPT = resolve(opt('--transcript', join(homedir(), '.claude/projects/-workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl')));
const out = join(here, 'baselines/issue-10-evidence');
const sha = (b) => createHash('sha256').update(b).digest('hex');
const shaFile = (p) => sha(readFileSync(p));
const pub = (p) => p.replace(homedir(), '~').replace(root + '/', '');

// README.md is hand-written and kept; everything else is regenerated
for (const d of ['raw', 'extracts']) { rmSync(join(out, d), { recursive: true, force: true }); mkdirSync(join(out, d), { recursive: true }); }
for (const f of ['MANIFEST.json', 'cell-evidence.json', 'operator-events.jsonl']) rmSync(join(out, f), { force: true });
// history-lxl-pre-host-fix.jsonl is an audit snapshot written once by perf/correct-lxl-history-host.mjs; it is kept and re-registered below

const files = []; // manifest rows
const addFile = (rel, content, source, extra = {}) => {
  const buf = Buffer.isBuffer(content) ? content : Buffer.from(content);
  writeFileSync(join(out, rel), buf);
  files.push({ file: rel, bytes: buf.length, sha256: sha(buf), source: pub(source.path), source_sha256: source.sha256 ?? shaFile(source.path), kind: source.kind, ...(source.lines ? { source_lines: source.lines } : {}), ...extra });
};

// ---- 1. full copies ---------------------------------------------------------------------------------
const copies = [
  [join(lxlDir, 'sweep.log'), 'raw/sweep.log'],
  [join(here, 'baselines/box-conditions.jsonl'), 'raw/box-conditions.jsonl'],
  [join(here, 'baselines/ts-lxl-8961e71.failures.jsonl'), 'raw/ts-lxl-8961e71.failures.jsonl'],
  [join(here, 'baselines/ts-4451709.failures.jsonl'), 'raw/ts-4451709.failures.jsonl'],
  [join(here, 'baselines/ts-4451709.rerun-failures.jsonl'), 'raw/ts-4451709.rerun-failures.jsonl'],
];
for (const [src, rel] of copies) addFile(rel, readFileSync(src), { path: src, kind: 'verbatim copy' });

// ---- 2. chunk extracts (failures[] verbatim + metadata) ---------------------------------------------------
const chunkJsons = readdirJson(lxlDir);
function readdirJson(d) {
  return ['cold-1x-000', 'cold-1x-001', 'cold-1x-002', 'cold-4x-000', 'cold-4x-001', 'cold-4x-004', 'warm-1x-001', 'warm-4x-004'];
}
for (const name of chunkJsons) {
  const src = join(lxlDir, `${name}.json`);
  const j = JSON.parse(readFileSync(src, 'utf8'));
  const ex = {
    note: 'extract of a gitignored chunk result file: metadata and failures[] verbatim; per-iteration arrays omitted',
    chunk: name,
    meta: Object.fromEntries(['chrome', 'playwright', 'node', 'platform', 'cpu', 'host', 'corpus', 'mode', 'iterations', 'cpuThrottle', 'viewport', 'upstreamPin', 'date'].map((k) => [k, j[k]])),
    results_index: j.results.map((r) => ({ fixture: r.fixture, format: r.format, tier: r.tier })),
    failures: j.failures,
  };
  addFile(`extracts/${name}.json`, JSON.stringify(ex, null, 1) + '\n', { path: src, kind: 'extract (metadata + failures[] verbatim)' });
}

// the one partially measured cell: warm 1x gen-pptx-slides-200-b has a single sample row before its timeout
{
  const src = join(lxlDir, 'warm-1x-001.jsonl');
  const lines = readFileSync(src, 'utf8').split('\n');
  const hit = lines.map((l, i) => [i + 1, l]).filter(([, l]) => l && JSON.parse(l).fixture === 'gen-pptx-slides-200-b');
  if (hit.length !== 1) throw new Error(`expected one slides-200-b warm 1x row, got ${hit.length}`);
  addFile('extracts/warm-1x-001.gen-pptx-slides-200-b.row.jsonl', hit[0][1] + '\n', { path: src, kind: 'extract (one row verbatim)', lines: [hit[0][0]] });
}

// ---- 3. empty marker files ---------------------------------------------------------------------------------
const markerNames = ['cold-4x-002', 'warm-1x-000', 'warm-1x-002', 'warm-4x-000', 'warm-4x-001', 'warm-4x-002'];
const markers = markerNames.map((n) => {
  const p = join(lxlDir, `${n}.jsonl`);
  if (statSync(p).size !== 0) throw new Error(`${p} is not an empty marker`);
  return { marker: `${n}.jsonl`, bytes: 0, source: pub(p), source_sha256: shaFile(p), mtime_utc: statSync(p).mtime.toISOString(), has_result_json: existsSync(join(lxlDir, `${n}.json`)) };
});
addFile('extracts/markers.json', JSON.stringify({ note: '0-byte files that make baseline-sweep.mjs skip a chunk; created by the operator (see operator-events.jsonl), not by the harness', markers }, null, 1) + '\n', { path: join(lxlDir, 'sweep.log'), kind: 'listing of 0-byte marker files' });

// ---- 4. operator events (sanitized) ------------------------------------------------------------------------
const WANT = [299, 707, 732, 2089, 2101, 2104, 2121, 2286, 2295, 2298, 2307];
const redactions = { email: 0, home_path: 0, token: 0, bearer: 0, jwt: 0 };
const redact = (s) => String(s)
  .replace(/[\w.+-]+@[\w-]+\.[\w.-]+/g, () => (redactions.email++, '<email>'))
  .replace(/\/home\/[\w.-]+/g, () => (redactions.home_path++, '~'))
  .replace(/\b(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,})\b/g, () => (redactions.token++, '<token>'))
  .replace(/Bearer\s+[A-Za-z0-9._~+/-]{8,}=*/gi, () => (redactions.bearer++, 'Bearer <redacted>'))
  .replace(/\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{5,}/g, () => (redactions.jwt++, '<jwt>'));
const tl = readFileSync(TRANSCRIPT, 'utf8').split('\n');
const results = new Map(); // tool_use_id -> { line, text }
tl.forEach((l, i) => {
  if (!l) return;
  let o; try { o = JSON.parse(l); } catch { return; }
  for (const b of Array.isArray(o.message?.content) ? o.message.content : []) {
    if (b.type === 'tool_result') results.set(b.tool_use_id, { line: i + 1, text: Array.isArray(b.content) ? b.content.map((x) => x.text ?? '').join('') : String(b.content) });
  }
});
const events = [];
for (const n of WANT) {
  const o = JSON.parse(tl[n - 1]);
  const b = o.message.content.find((x) => x.type === 'tool_use' && x.name === 'Bash');
  if (!b) throw new Error(`transcript line ${n} has no Bash tool_use`);
  const r = results.get(b.id);
  events.push({
    transcript_line: n, timestamp: o.timestamp, session: o.sessionId ?? '1e83d572-c74b-4385-a8d7-e42d44880ae2', tool: 'Bash',
    description: redact(b.input.description ?? ''), command: redact(b.input.command),
    ...(r ? { result_line: r.line, result: redact(r.text).slice(0, 1500) } : {}),
  });
}
const evText = events.map((e) => JSON.stringify(e)).join('\n') + '\n';
addFile('operator-events.jsonl', evText, { path: TRANSCRIPT, kind: 'extract (selected Bash commands and their results; no assistant or user prose)', lines: WANT }, { redactions });

// post-scan of everything written
const SUSPECT = /(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-|Bearer\s+\S{8,}|eyJ[A-Za-z0-9_-]{10,}\.|[\w.+-]+@[\w-]+\.[\w.-]+|\/home\/|password\s*[:=]|secret\s*[:=]|api[_-]?key)/i;
for (const f of files) {
  const t = readFileSync(join(out, f.file), 'utf8');
  const m = SUSPECT.exec(t);
  // the S/M failure text legitimately says "password protected" (an engine error); only assignments are suspect
  if (m) throw new Error(`${f.file}: suspicious text left after redaction: ${m[0].slice(0, 40)}`);
}

// ---- 5. cell -> evidence mapping ----------------------------------------------------------------------------
const prov = JSON.parse(readFileSync(join(here, 'baselines/failure-provenance.json'), 'utf8'));
const CHUNK_OF = {};
const CH = [['gen-docx-pages-500', 'gen-docx-pages-500-b', 'gen-docx-pages-500-c', 'gen-docx-xl-a'], ['gen-docx-xl-b', 'gen-docx-xl-c', 'gen-pptx-slides-200', 'gen-pptx-slides-200-b'], ['gen-pptx-slides-200-c', 'gen-pptx-xl-a', 'gen-pptx-xl-b', 'gen-pptx-xl-c'], ['real-pptx-poi-customGeo', 'real-pptx-python-pptx-cht-chart-type', 'upstream-61fc791e15c0', 'gen-xlsx-cells-1m'], ['gen-xlsx-cells-1m-pad-l', 'gen-xlsx-cells-1m-pad-l2', 'gen-xlsx-xl-a', 'gen-xlsx-xl-b'], ['gen-xlsx-xl-c']];
CH.forEach((fx, i) => fx.forEach((f) => (CHUNK_OF[f] = i)));
const chunkName = (c) => `${c.mode}-${c.cpu_throttle}x-${String(CHUNK_OF[c.fixture]).padStart(3, '0')}`;
const base = (p) => p.split('/').pop();
const map = (e) => {
  const f = base(e.file);
  if (e.pointer) return { file: `extracts/${f}`, pointer: e.pointer };
  if (e.file.startsWith('~/')) { const line = events.find((x) => x.transcript_line === e.line); if (!line) throw new Error(`no operator event for transcript line ${e.line}`); return { file: 'operator-events.jsonl', transcript_line: e.line }; }
  if (/\.jsonl$/.test(f) && /^(cold|warm)-/.test(f)) return { file: 'extracts/markers.json', marker: f };
  return { file: `raw/${f}`, ...(e.line ? { line: e.line } : {}) };
};
const cells = prov.cells.map((c) => ({
  fixture: c.fixture, mode: c.mode, cpu_throttle: c.cpu_throttle, source: c.source, status: c.status,
  ...(c.source === 'lxl-sweep' ? { chunk: chunkName(c) } : {}),
  evidence: c.evidence.map(map),
}));
addFile('cell-evidence.json', JSON.stringify({ note: 'one row per classified cell (S/M and L/XL); evidence points into this directory', cells }, null, 1) + '\n', { path: join(here, 'baselines/failure-provenance.json'), kind: 'derived mapping from failure-provenance.json' });

// pre-correction snapshot of the 42 L/XL history lines (see correct-lxl-history-host.mjs)
const snapPath = join(out, 'history-lxl-pre-host-fix.jsonl');
const PRE_FIX_HISTORY_SHA = '3e30e7a2a861e1041d1e33d43bbc670e7fa9820df6708a00836e8b34b68edefd'; // perf/history.jsonl at 0d59e36, 994 lines
if (existsSync(snapPath)) {
  const buf = readFileSync(snapPath);
  files.push({ file: 'history-lxl-pre-host-fix.jsonl', bytes: buf.length, sha256: sha(buf), source: 'perf/history.jsonl (git 0d59e36, lines 953-994, before the host correction)', source_sha256: PRE_FIX_HISTORY_SHA, kind: 'verbatim copy of 42 lines of the pre-correction history', source_lines: [953, 994] });
}
const historyCorrection = { snapshot: 'history-lxl-pre-host-fix.jsonl', pre_fix_history_sha256: PRE_FIX_HISTORY_SHA, pre_fix_history_lines: 994, corrected_line_range: [953, 994], corrected_by: 'perf/correct-lxl-history-host.mjs', host_source: 'perf/baselines/ts-lxl-8961e71.jsonl (row.host, identical in the 18 chunk result files)' };
writeFileSync(join(out, 'MANIFEST.json'), JSON.stringify({
  schema: 'issue-10-evidence/1',
  note: 'sha256 of each file in this directory, where it came from, and the sha256 of that original (so the extract can be re-derived from the originals).',
  originals_are_gitignored: 'perf/results/lxl-sweep/*, and the operator transcript outside the repo; they are not edited by this tooling',
  redactions,
  history_correction: historyCorrection,
  files: files.sort((a, b) => a.file.localeCompare(b.file)),
}, null, 1) + '\n');
console.log(`wrote ${relative(root, out)}: ${files.length} files + MANIFEST.json; ${cells.length} cells; redactions ${JSON.stringify(redactions)}`);
