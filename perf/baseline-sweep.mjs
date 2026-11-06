#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * TS baseline sweep (#10): every S/M xlsx/docx/pptx fixture of fixtures/manifest.json, in the four cells
 * cold/warm x 1x/4x CPU, cold n=10 and warm n=20 (+1 discarded warm-up), engine ts. Runs `run.mjs` in chunks
 * of CHUNK fixtures (a warm chunk shares one persistent context, as in the original baseline), resumable:
 * a chunk whose result file exists is skipped. Merge with `--merge out.jsonl` once all chunks are done.
 *
 *   node perf/baseline-sweep.mjs [--workers 1|2] [--dir perf/results/sweep] [--chunk 8]
 *   node perf/baseline-sweep.mjs --workers 2 --dir perf/results/rerun --fixtures id1,id2   (subset, e.g. a re-run)
 *   node perf/baseline-sweep.mjs --merge perf/baselines/ts-<commit>.jsonl
 *
 * With --workers 2 the two workers always run the same cell on disjoint chunks, and the host fingerprint
 * records concurrent_runs=2. A fixture that fails (e.g. an encrypted file the TS engine rejects) is written to
 * <dir>/failures.jsonl and left out of the baseline; the sweep carries on.
 */
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync, appendFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { logBoxConditions } from './box-conditions.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const a = {};
for (let i = 2; i < process.argv.length; i++) {
  if (!process.argv[i].startsWith('--')) continue;
  const next = process.argv[i + 1];
  a[process.argv[i].slice(2)] = next === undefined || next.startsWith('--') ? true : (i++, next);
}
const dir = resolve(a.dir ?? join(here, 'results/sweep'));
const CELLS = [['cold', 1], ['cold', 4], ['warm', 1], ['warm', 4]];

if (a.merge) {
  const files = readdirSync(dir).filter((f) => f.endsWith('.jsonl') && f !== 'failures.jsonl').sort();
  const lines = files.flatMap((f) => readFileSync(join(dir, f), 'utf8').split('\n').filter(Boolean));
  const outPath = resolve(a.merge);
  writeFileSync(outPath, lines.join('\n') + '\n');
  // Also copy failures.jsonl alongside the merged output for use by assemble-combined.mjs.
  const failSrc = join(dir, 'failures.jsonl');
  const failDst = outPath.replace(/\.jsonl$/, '.failures.jsonl');
  if (existsSync(failSrc)) writeFileSync(failDst, readFileSync(failSrc));
  else writeFileSync(failDst, '');
  console.log(`merged ${files.length} chunk files, ${lines.length} rows -> ${outPath}`);
  process.exit(0);
}

const WORKERS = Number(a.workers ?? 1);
const CHUNK = Number(a.chunk ?? 8);
mkdirSync(dir, { recursive: true });
const only = a.fixtures ? new Set(String(a.fixtures).split(',')) : null; // --fixtures a,b,c: re-run just these ids
const ids = spawnSync('node', [join(here, 'run.mjs'), '--corpus', 'manifest', '--list'], { encoding: 'utf8' }).stdout.split('\n').filter(Boolean).filter((id) => !only || only.has(id));
const chunks = [];
for (let i = 0; i < ids.length; i += CHUNK) chunks.push(ids.slice(i, i + CHUNK));
console.log(`${ids.length} fixtures, ${chunks.length} chunks per cell, ${WORKERS} worker(s)`);

const runChunk = (mode, thr, n, ids) =>
  new Promise((done) => {
    const out = join(dir, `${mode}-${thr}x-${String(n).padStart(3, '0')}.json`);
    if (existsSync(out.replace(/\.json$/, '.jsonl'))) return done();
    logBoxConditions({ fixture_id: ids[0] ?? null, mode, cpu: thr });
    const t0 = Date.now();
    const timeoutMs = a.timeout ? Number(a.timeout) : 120000;
    const extraArgs = [
      '--timeout', String(timeoutMs),
      ...(a.iterations ? ['--iterations', String(a.iterations)] : []),
    ];
    const p = spawn('node', [join(here, 'run.mjs'), '--corpus', 'manifest', '--engine', 'ts', '--mode', mode, '--cpu-throttle', String(thr), '--fixtures', ids.join(','), '--out', out, ...extraArgs], {
      env: { ...process.env, PERF_CONCURRENT_RUNS: String(WORKERS) },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    // Wall-clock kill: Playwright waitForFunction timeout doesn't fire when JS is synchronous.
    // Allow each fixture × iteration up to timeout, plus 60s overhead, then kill.
    const iterCount = a.iterations ? Number(a.iterations) : (mode === 'cold' ? 10 : 20);
    const killMs = ids.length * iterCount * (timeoutMs + 1000) + 60000;
    const killTimer = setTimeout(() => {
      console.error(`${new Date().toISOString()} SIGKILL chunk ${n} (stuck >${(killMs / 60000).toFixed(1)}min)`);
      p.kill('SIGKILL');
    }, killMs);
    let err = '';
    p.stderr.on('data', (d) => (err += d));
    p.stdout.on('data', () => {});
    p.on('close', (code) => {
      clearTimeout(killTimer);
      // run.mjs exits 1 when some fixture failed but still writes the rows of those that passed
      const failed = existsSync(out) ? JSON.parse(readFileSync(out, 'utf8')).failures : [{ error: err.slice(-400) }];
      for (const f of failed) appendFileSync(join(dir, 'failures.jsonl'), JSON.stringify({ mode, cpuThrottle: thr, ...f }) + '\n');
      console.log(`${new Date().toISOString()} ${mode} ${thr}x chunk ${n}/${chunks.length} exit ${code} ${((Date.now() - t0) / 60000).toFixed(1)} min, ${failed.length} failure(s)`);
      done();
    });
  });

for (const [mode, thr] of CELLS) {
  const queue = chunks.map((c, n) => [n, c]);
  await Promise.all(Array.from({ length: WORKERS }, async () => {
    for (let job; (job = queue.shift()); ) await runChunk(mode, thr, job[0], job[1]);
  }));
}
console.log('sweep done');
