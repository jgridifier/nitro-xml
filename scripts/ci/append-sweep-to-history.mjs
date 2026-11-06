#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// One-shot script: aggregate ts-4451709.jsonl + ts-4451709.rerun.jsonl into
// history.jsonl format rows and append them to perf/history.jsonl.
// Usage: node scripts/ci/append-sweep-to-history.mjs
// Idempotent: checks for existing rows at the same commit before appending.

import { readFileSync, appendFileSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';

const ROOT = new URL('../../', import.meta.url).pathname.replace(/\/$/, '');

const HISTORY_FILE = `${ROOT}/perf/history.jsonl`;
const SWEEP_FILE   = `${ROOT}/perf/baselines/ts-4451709.jsonl`;
const RERUN_FILE   = `${ROOT}/perf/baselines/ts-4451709.rerun.jsonl`;
const FAILURES_FILE = `${ROOT}/perf/baselines/ts-4451709.failures.jsonl`;
const RERUN_FAILURES_FILE = `${ROOT}/perf/baselines/ts-4451709.rerun-failures.jsonl`;

// Commit and date for this sweep (from BASELINE.md)
const SWEEP_COMMIT = '4451709';
const SWEEP_DATE   = '2026-11-04T00:00:00-05:00';
const SWEEP_BRANCH = 'main';

function readJsonl(path) {
  if (!existsSync(path)) return [];
  return readFileSync(path, 'utf8').split('\n').filter(Boolean).map(l => JSON.parse(l));
}

function percentile(sorted, p) {
  if (!sorted.length) return null;
  if (sorted.length === 1) return sorted[0];
  const idx = (p / 100) * (sorted.length - 1);
  const lo = Math.floor(idx), hi = Math.ceil(idx), frac = idx - lo;
  return sorted[lo] + frac * (sorted[hi] - sorted[lo]);
}

function aggregate(values) {
  const nums = values.filter(v => v != null && typeof v === 'number');
  if (!nums.length) return null;
  const sorted = [...nums].sort((a, b) => a - b);
  return {
    median: percentile(sorted, 50),
    p95:    percentile(sorted, 95),
    min:    sorted[0],
    max:    sorted[sorted.length - 1],
  };
}

function groupSamples(samples) {
  // Group by (fixture, engine, mode, cpuThrottle)
  const groups = new Map();
  for (const s of samples) {
    const key = `${s.fixture}|${s.engine}|${s.mode}|${s.cpuThrottle ?? 1}`;
    if (!groups.has(key)) groups.set(key, { samples: [], meta: s });
    groups.get(key).samples.push(s);
  }
  return groups;
}

function buildHistoryRow(key, { samples, meta }, opts = {}) {
  const { rerun = false } = opts;

  // Collect all metric keys across samples
  const metricKeys = new Set();
  for (const s of samples) {
    for (const k of Object.keys(s.metrics ?? {})) metricKeys.add(k);
  }

  const metrics = {};
  for (const mk of metricKeys) {
    const vals = samples.map(s => s.metrics?.[mk] ?? null);
    const agg = aggregate(vals);
    if (agg) metrics[mk] = agg;
  }

  const host = meta.host ?? {};
  return {
    commit:          SWEEP_COMMIT,
    date:            SWEEP_DATE,
    branch:          SWEEP_BRANCH,
    engine:          meta.engine,
    fixture_id:      meta.fixture,
    format:          meta.format,
    tier:            meta.tier,
    mode:            meta.mode,
    cpu_throttle:    meta.cpuThrottle ?? 1,
    chrome_version:  meta.chrome ?? host.chrome_version ?? 'unknown',
    runner:          'local',
    host: {
      kind:           host.kind ?? 'box',
      id:             host.id ?? 'box-4467396174',
      os:             host.os ?? '',
      arch:           host.arch,
      cpu:            host.cpu_model ?? host.cpu ?? '',
      cores:          host.cores,
      memory_gb:      host.memory_gb,
      chrome_version: host.chrome_version ?? meta.chrome ?? 'unknown',
      node:           host.node,
      concurrent_runs: host.concurrent_runs,
    },
    seeded: false,
    rerun:  rerun,
    n:      samples.length,
    metrics,
  };
}

// Also produce format-tier aggregate rows to supersede the stale seeded rows
function buildTierAggregateRows(sweepRows, rerunRows) {
  // Group per-fixture rows by (format, tier, mode, cpu_throttle) across all fixtures
  const tierGroups = new Map();
  for (const row of [...sweepRows, ...rerunRows]) {
    const key = `${row.format}|${row.tier}|${row.mode}|${row.cpu_throttle}`;
    if (!tierGroups.has(key)) tierGroups.set(key, { rows: [], meta: row });
    tierGroups.get(key).rows.push(row);
  }

  const results = [];
  for (const [key, { rows, meta }] of tierGroups) {
    const metricKeys = new Set();
    for (const r of rows) {
      for (const mk of Object.keys(r.metrics ?? {})) metricKeys.add(mk);
    }

    // For tier aggregates: aggregate over fixture medians (median of medians)
    const metrics = {};
    for (const mk of metricKeys) {
      const medians = rows.map(r => r.metrics?.[mk]?.median).filter(v => v != null);
      const agg = aggregate(medians);
      if (agg) metrics[mk] = agg;
    }

    const host = meta.host ?? {};
    results.push({
      commit:         SWEEP_COMMIT,
      date:           SWEEP_DATE,
      branch:         SWEEP_BRANCH,
      engine:         meta.engine,
      fixture_id:     `${meta.format}-${meta.tier}`,  // e.g. xlsx-S
      format:         meta.format,
      tier:           meta.tier,
      mode:           meta.mode,
      cpu_throttle:   meta.cpu_throttle,
      chrome_version: meta.chrome_version,
      runner:         'local',
      host:           meta.host,
      seeded:         false,
      rerun:          false,
      n_fixtures:     rows.length,
      n_per_fixture:  rows[0]?.n ?? null,
      metrics,
    });
  }
  return results;
}

// --- Main ---
const sweepSamples = readJsonl(SWEEP_FILE);
const rerunSamples = readJsonl(RERUN_FILE);

console.log(`sweep samples: ${sweepSamples.length}, rerun samples: ${rerunSamples.length}`);

// Build per-fixture aggregate rows from sweep
const sweepGroups = groupSamples(sweepSamples);
const sweepHistoryRows = [];
for (const [key, grp] of sweepGroups) {
  sweepHistoryRows.push(buildHistoryRow(key, grp, { rerun: false }));
}
console.log(`sweep per-fixture rows: ${sweepHistoryRows.length}`);

// Build per-fixture aggregate rows from rerun
const rerunGroups = groupSamples(rerunSamples);
const rerunHistoryRows = [];
for (const [key, grp] of rerunGroups) {
  rerunHistoryRows.push(buildHistoryRow(key, grp, { rerun: true }));
}
console.log(`rerun per-fixture rows: ${rerunHistoryRows.length}`);

// Build format-tier aggregate rows (to supersede stale seeded rows in dashboard/benchmarks)
const tierRows = buildTierAggregateRows(sweepHistoryRows, rerunHistoryRows);
console.log(`tier aggregate rows: ${tierRows.length}`);

// Check existing history for idempotency
const existingRows = readJsonl(HISTORY_FILE);
const existingKeys = new Set(
  existingRows
    .filter(r => r.commit === SWEEP_COMMIT && !r.seeded)
    .map(r => `${r.fixture_id}|${r.engine}|${r.mode}|${r.cpu_throttle ?? 1}`)
);

const newPerFixture = sweepHistoryRows.filter(r =>
  !existingKeys.has(`${r.fixture_id}|${r.engine}|${r.mode}|${r.cpu_throttle ?? 1}`)
);
const newRerunRows = rerunHistoryRows.filter(r =>
  !existingKeys.has(`${r.fixture_id}|${r.engine}|${r.mode}|${r.cpu_throttle ?? 1}`)
);
const newTierRows = tierRows.filter(r =>
  !existingKeys.has(`${r.fixture_id}|${r.engine}|${r.mode}|${r.cpu_throttle ?? 1}`)
);

const allNew = [...newPerFixture, ...newRerunRows, ...newTierRows];
console.log(`rows to append: ${allNew.length} (${newPerFixture.length} sweep + ${newRerunRows.length} rerun + ${newTierRows.length} tier aggregates)`);

if (!allNew.length) {
  console.log('already up to date — nothing appended');
  process.exit(0);
}

const lines = allNew.map(r => JSON.stringify(r)).join('\n') + '\n';
appendFileSync(HISTORY_FILE, lines);
console.log(`appended ${allNew.length} rows to ${HISTORY_FILE}`);
