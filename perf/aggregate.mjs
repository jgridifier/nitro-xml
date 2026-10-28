#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Aggregate perf JSONL (one object per fixture, engine, mode, iteration; written by run.mjs --jsonl)
 * into median / p95 / bootstrap CI per (fixture, engine, mode, cpuThrottle, metric).
 *
 *   node perf/aggregate.mjs results.jsonl [--out summary.json]
 *   node perf/aggregate.mjs --check          # recompute perf/reference and compare to the recorded answers
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { summarize } from './stats.mjs';

const here = dirname(fileURLToPath(import.meta.url));

export function aggregate(rows) {
  const groups = new Map();
  for (const r of rows) {
    const key = [r.fixture, r.engine, r.mode, r.cpuThrottle ?? 1].join('|');
    if (!groups.has(key)) groups.set(key, { fixture: r.fixture, engine: r.engine, mode: r.mode, cpuThrottle: r.cpuThrottle ?? 1, samples: [] });
    groups.get(key).samples.push(r.metrics);
  }
  return [...groups.values()].map((g) => {
    const names = new Set(g.samples.flatMap((s) => Object.keys(s)));
    const metrics = {};
    for (const k of names) {
      const xs = g.samples.map((s) => s[k]).filter((v) => typeof v === 'number' && Number.isFinite(v));
      if (xs.length) metrics[k] = summarize(xs);
    }
    return { fixture: g.fixture, engine: g.engine, mode: g.mode, cpuThrottle: g.cpuThrottle, iterations: g.samples.length, metrics };
  });
}

export const readJsonl = (file) =>
  readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));

/** Compare aggregate() on the recorded dataset with the recorded answers; returns a list of mismatches. */
export function checkReference(dir = join(here, 'reference')) {
  const ref = JSON.parse(readFileSync(join(dir, 'aggregate-reference.json'), 'utf8'));
  const rows = readJsonl(join(dir, 'aggregate-reference.jsonl'));
  const got = aggregate(rows);
  const bad = [];
  for (const want of ref.expected) {
    const g = got.find((x) => x.fixture === want.fixture && x.engine === want.engine && x.mode === want.mode);
    if (!g) {
      bad.push(`${want.fixture}/${want.engine}: group missing`);
      continue;
    }
    for (const [metric, w] of Object.entries(want.metrics)) {
      const s = g.metrics[metric];
      for (const [field, wv] of Object.entries(w)) {
        const gv = s?.[field];
        const eq = Array.isArray(wv) ? wv.every((x, i) => Math.abs(x - gv?.[i]) <= ref.tolerance) : Math.abs(wv - gv) <= ref.tolerance;
        if (!eq) bad.push(`${want.fixture}/${want.engine} ${metric}.${field}: expected ${JSON.stringify(wv)}, got ${JSON.stringify(gv)}`);
      }
    }
  }
  return bad;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2);
  if (argv.includes('--check')) {
    const bad = checkReference();
    if (bad.length) {
      console.error(`aggregate reference mismatch:\n${bad.join('\n')}`);
      process.exit(1);
    }
    console.log('aggregate reference: ok');
  } else {
    const file = argv.find((a) => !a.startsWith('--'));
    if (!file) throw new Error('usage: node perf/aggregate.mjs results.jsonl [--out summary.json] | --check');
    const out = argv.includes('--out') ? argv[argv.indexOf('--out') + 1] : null;
    const agg = aggregate(readJsonl(file));
    if (out) writeFileSync(out, JSON.stringify(agg, null, 2));
    for (const g of agg) {
      const m = g.metrics.first_render_ms;
      console.log(`${g.fixture.padEnd(10)} ${g.engine.padEnd(5)} ${g.mode.padEnd(5)} n=${g.iterations} first_render median ${m?.median.toFixed(1)} p95 ${m?.p95.toFixed(1)} CI95 [${m?.median_ci95.map((x) => x.toFixed(1)).join(', ')}]`);
    }
  }
}
