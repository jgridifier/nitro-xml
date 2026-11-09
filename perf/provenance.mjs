// SPDX-License-Identifier: MIT
/**
 * Failure provenance and per-cell sample-count labelling for the TS baseline report (#10).
 *
 * A "cell" is (fixture, mode, cpuThrottle). Every excluded cell has exactly one status:
 *   observed_timeout  the harness actually hit its own timeout during a run
 *   observed_error    the run finished with an error (engine limit, throw, password); not a timeout
 *   operator_aborted  the process was killed by us / OOM / manually before the cell finished
 *   not_run           never started, including cells skipped because a timeout was predicted
 *   unknown           provenance not determinable (also the fallback for a cell with no record)
 * A predicted timeout is never a measured failure. perf/baselines/failure-provenance.json holds the
 * per-cell record with evidence; raw failure files are never edited.
 */
import { readFileSync, existsSync } from 'node:fs';

export const STATUSES = ['observed_timeout', 'observed_error', 'operator_aborted', 'not_run', 'unknown'];
export const STATUS_LABEL = {
  observed_timeout: 'observed timeout',
  observed_error: 'observed error',
  operator_aborted: 'operator aborted',
  not_run: 'not run',
  unknown: 'unknown',
};

/**
 * Design sample count per cell: the harness defaults (perf/run.mjs: "Defaults: 10 iterations cold, 20 warm",
 * also perf/baseline-sweep.mjs). A cell with fewer samples is labelled "thin sample"; nothing is suppressed
 * and no new threshold is introduced.
 */
export const DESIGN_N = { cold: 10, warm: 20 };

export const cellKey = (fixture, mode, cpu) => `${fixture}|${mode}|${cpu ?? 1}`;

/** Status of a raw harness failure row, from its own error text only (used for rows the harness wrote). */
export function classifyRawRow(row) {
  const e = String(row?.error ?? '');
  if (/Timeout \d+ms exceeded|timed out waiting for/.test(e)) return 'observed_timeout';
  if (e.trim()) return 'observed_error';
  return 'unknown';
}

export function loadProvenance(file) {
  if (!file || !existsSync(file)) return { cells: [], byCell: new Map() };
  const j = JSON.parse(readFileSync(file, 'utf8'));
  return indexProvenance(j);
}

/** Later sources win for the same cell: lxl-sweep > sm-rerun > sm-sweep. */
const RANK = { 'sm-sweep': 0, 'sm-rerun': 1, 'lxl-sweep': 2 };
export function indexProvenance(j) {
  const byCell = new Map();
  for (const c of j.cells ?? []) {
    const k = cellKey(c.fixture, c.mode, c.cpu_throttle);
    const cur = byCell.get(k);
    if (!cur || (RANK[c.source] ?? 0) >= (RANK[cur.source] ?? 0)) byCell.set(k, c);
  }
  return { ...j, byCell };
}

/** Status for a failed cell; `unknown` (never a guess) when no record exists. */
export function statusFor(prov, fixture, mode, cpu) {
  return prov.byCell.get(cellKey(fixture, mode, cpu))?.status ?? 'unknown';
}

/** Number of finite samples behind one metric of one cell: `vals` is the array of numbers. */
export const sampleN = (vals) => (vals ? vals.length : 0);

export const isThin = (n, mode) => n > 0 && n < (DESIGN_N[mode] ?? DESIGN_N.cold);
/** `n=10`, or `n=3, thin sample` when below the design count; `no samples` when none. */
export const nLabel = (n, mode) => (n === 0 ? 'no samples' : isThin(n, mode) ? `n=${n}, thin sample` : `n=${n}`);

/**
 * Per-fixture cell: `median / p95 (n=N[, thin sample])`, or `no samples`. p95 is the empirical 95th
 * percentile of the samples (stats.mjs, linear interpolation); it is not a confidence interval.
 */
export function cellText(vals, med, p95, fmt, mode) {
  const n = sampleN(vals);
  if (!n) return 'no samples';
  return `${fmt(med(vals))} / ${fmt(p95(vals))} (${nLabel(n, mode)})`;
}

/** Median only, with n. */
export function medianText(vals, med, fmt, mode) {
  const n = sampleN(vals);
  return n ? `${fmt(med(vals))} (${nLabel(n, mode)})` : 'no samples';
}

/** n range text for a set of per-fixture sample counts. */
export const nRangeText = (ns) => {
  if (!ns.length) return 'n=0';
  const lo = Math.min(...ns), hi = Math.max(...ns);
  return lo === hi ? `n=${lo}` : `n=${lo}–${hi}`;
};

/**
 * Tier aggregate cell: median (min..max) over the per-fixture medians (descriptive, not a claim), with the
 * range of n over the contributing fixtures and how many of them are thin. `items` = [{ vals }].
 */
export function aggregateText(items, med, fmt, mode) {
  const have = items.filter((i) => sampleN(i.vals) > 0);
  if (!have.length) return 'no samples';
  const ms = have.map((i) => med(i.vals));
  const thin = have.filter((i) => isThin(sampleN(i.vals), mode)).length;
  const missing = items.length - have.length;
  return `${fmt(med(ms))} (${fmt(Math.min(...ms))}..${fmt(Math.max(...ms))}) ${nRangeText(have.map((i) => sampleN(i.vals)))}${thin ? `, thin sample in ${thin}/${have.length} fixtures` : ''}${missing ? `, ${missing} fixtures no samples` : ''}`;
}
