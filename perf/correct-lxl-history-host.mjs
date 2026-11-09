#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * One-off, idempotent correction of the 42 L/XL entries of perf/history.jsonl (commit 8961e71, branch issue-10-lxl-real).
 * They were appended by history-append.mjs on another machine and recorded THAT machine's host (grok-bot-vm-..., no
 * arch/memory/node/concurrent_runs). The authoritative host is in the raw result rows they were built from
 * (perf/baselines/ts-lxl-8961e71.jsonl, each row's `host`, produced by run.mjs on the measuring machine; the chunk files
 * perf/results/lxl-sweep/*.json carry the identical host). Only those sources are used; nothing is read from this machine.
 *
 *   node perf/correct-lxl-history-host.mjs           rewrites ONLY the 42 lines in place; writes the pre-correction snapshot
 *   node perf/correct-lxl-history-host.mjs --check   exits 1 unless the file is already corrected
 *
 * Each corrected entry keeps the originally recorded host in `host_import_recorded` and gets `provenance.host` (source
 * file, source row lines, source sha256, per-field recovered/unknown). Pre-correction lines: issue-10-evidence/history-lxl-pre-host-fix.jsonl.
 */
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { sourceHost } from './aggregate.mjs';
import { isUnknown } from './unknown.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const HIST = join(here, 'history.jsonl');
const SRC = 'perf/baselines/ts-lxl-8961e71.jsonl';
const SNAP = join(here, 'baselines/issue-10-evidence/history-lxl-pre-host-fix.jsonl');
const sha = (b) => createHash('sha256').update(b).digest('hex');

const histBuf = readFileSync(HIST, 'utf8');
const lines = histBuf.split('\n');
const srcBuf = readFileSync(join(here, '..', SRC));
const srcLines = srcBuf.toString('utf8').split('\n');
const srcRows = srcLines.map((l, i) => [i + 1, l]).filter(([, l]) => l).map(([n, l]) => [n, JSON.parse(l)]);

const targets = lines.map((l, i) => [i, l]).filter(([, l]) => l && (() => { const e = JSON.parse(l); return e.commit === '8961e71' && e.branch === 'issue-10-lxl-real'; })());
if (targets.length !== 42) throw new Error(`expected 42 L/XL entries, found ${targets.length}`);
const already = targets.every(([, l]) => JSON.parse(l).host_import_recorded);
if (process.argv.includes('--check')) { console.log(already ? 'corrected' : 'NOT corrected'); process.exit(already ? 0 : 1); }
if (already) { console.log('already corrected; nothing to do'); process.exit(0); }

const FIELDS = ['kind', 'id', 'os', 'arch', 'cpu', 'cores', 'memory_gb', 'chrome_version', 'node', 'concurrent_runs'];
const out = [...lines];
for (const [i, l] of targets) {
  const e = JSON.parse(l);
  const mine = srcRows.filter(([, r]) => r.fixture === e.fixture_id && r.mode === e.mode && (r.cpuThrottle ?? 1) === e.cpu_throttle);
  if (mine.length !== e.n) throw new Error(`${e.fixture_id} ${e.mode} ${e.cpu_throttle}x: ${mine.length} source rows, entry n=${e.n}`);
  const hosts = new Set(mine.map(([, r]) => JSON.stringify(sourceHost(r))));
  if (hosts.size !== 1) throw new Error(`${e.fixture_id}: source rows disagree on host`);
  const host = sourceHost(mine[0][1]);
  const fields = Object.fromEntries(FIELDS.map((f) => [f, isUnknown(host[f]) ? 'unknown' : 'recovered']));
  const next = {};
  for (const [k, v] of Object.entries(e)) {
    next[k] = k === 'host' ? host : k === 'chrome_version' ? host.chrome_version : v;
    if (k === 'host') next.host_import_recorded = e.host;
  }
  next.provenance = { host: { source: SRC, source_row_lines: mine.map(([n]) => n), source_sha256: sha(srcBuf), fields, corrected_by: 'perf/correct-lxl-history-host.mjs', original_history_line: i + 1 } };
  out[i] = JSON.stringify(next);
}
writeFileSync(SNAP, targets.map(([, l]) => l).join('\n') + '\n');
writeFileSync(HIST, out.join('\n'));
console.log(`corrected ${targets.length} entries (history lines ${targets[0][0] + 1}-${targets[targets.length - 1][0] + 1}); pre-correction snapshot ${SNAP}`);
console.log('pre-correction history.jsonl sha256', sha(histBuf), 'lines', histBuf.split('\n').filter(Boolean).length);
