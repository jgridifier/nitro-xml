// SPDX-License-Identifier: MIT
/**
 * Log box conditions (memory, swap, load) to perf/baselines/box-conditions.jsonl.
 * Called by baseline-sweep.mjs before each chunk; also usable standalone:
 *   node perf/box-conditions.mjs [--fixture id] [--mode cold|warm] [--cpu 1|4]
 */
import { readFileSync, appendFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));

function readMeminfo() {
  const text = readFileSync('/proc/meminfo', 'utf8');
  const kv = {};
  for (const line of text.split('\n')) {
    const m = /^(\w+):\s+(\d+)/.exec(line);
    if (m) kv[m[1]] = Number(m[2]);
  }
  return kv;
}

function readLoadavg() {
  const [l1, l5, l15] = readFileSync('/proc/loadavg', 'utf8').trim().split(' ');
  return { l1: Number(l1), l5: Number(l5), l15: Number(l15) };
}

export function sampleBoxConditions({ fixture_id = null, mode = null, cpu = null } = {}) {
  const mem = readMeminfo();
  const load = readLoadavg();
  return {
    timestamp: new Date().toISOString(),
    free_mem_mb: Math.round((mem.MemAvailable ?? mem.MemFree ?? 0) / 1024),
    swap_used_mb: Math.round(((mem.SwapTotal ?? 0) - (mem.SwapFree ?? 0)) / 1024),
    loadavg_1m: load.l1,
    loadavg_5m: load.l5,
    loadavg_15m: load.l15,
    fixture_id,
    mode,
    cpu,
  };
}

export function logBoxConditions(opts = {}, outFile = join(here, 'baselines/box-conditions.jsonl')) {
  mkdirSync(dirname(outFile), { recursive: true });
  const entry = sampleBoxConditions(opts);
  appendFileSync(outFile, JSON.stringify(entry) + '\n');
  return entry;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const a = {};
  for (let i = 2; i < process.argv.length; i++) {
    if (!process.argv[i].startsWith('--')) continue;
    const next = process.argv[i + 1];
    a[process.argv[i].slice(2)] = next === undefined || next.startsWith('--') ? true : (i++, next);
  }
  const entry = logBoxConditions({ fixture_id: a.fixture ?? null, mode: a.mode ?? null, cpu: a.cpu ? Number(a.cpu) : null });
  console.log(JSON.stringify(entry, null, 2));
}
