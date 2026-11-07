// SPDX-License-Identifier: MIT
/**
 * Failure provenance and per-cell sample-count rules for the TS baseline report (#10).
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

/** Below this many samples a cell gets no speed number, ranking or CI ("insufficient samples"). */
export const MIN_N = 5;

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

export const insufficient = (n) => (n > 0 ? `insufficient samples (n=${n})` : 'no samples');

/**
 * Text for a per-fixture cell metric: `median / p95 (n=N)`, or `insufficient samples (n=N)` / `no samples`.
 * `med` and `p95` are only evaluated when n >= MIN_N, so a suppressed cell can never leak a number.
 */
export function cellText(vals, med, p95, fmt, minN = MIN_N) {
  const n = sampleN(vals);
  if (n < minN) return insufficient(n);
  return `${fmt(med(vals))} / ${fmt(p95(vals))} (n=${n})`;
}

/** n range text for a set of per-fixture sample counts. */
export const nRangeText = (ns) => {
  if (!ns.length) return 'n=0';
  const lo = Math.min(...ns), hi = Math.max(...ns);
  return lo === hi ? `n=${lo}` : `n=${lo}–${hi}`;
};

/**
 * Tier aggregate cell: median (min..max) over the per-fixture medians of fixtures with n >= MIN_N in the cell.
 * Fixtures below MIN_N are left out and counted; with none left the cell is `insufficient samples`.
 * `items` = [{ vals }] (one per fixture of the tier).
 */
export function aggregateText(items, med, fmt, minN = MIN_N) {
  const ok = items.filter((i) => sampleN(i.vals) >= minN);
  if (!ok.length) {
    const ns = items.map((i) => sampleN(i.vals));
    return ns.some((n) => n > 0) ? `insufficient samples (${nRangeText(ns.filter((n) => n > 0))})` : '-';
  }
  const ms = ok.map((i) => med(i.vals));
  const left = items.length - ok.length;
  return `${fmt(med(ms))} (${fmt(Math.min(...ms))}..${fmt(Math.max(...ms))}) ${nRangeText(ok.map((i) => sampleN(i.vals)))}${left ? `, ${left}/${items.length} fixtures omitted (n<${minN} or no samples)` : ''}`;
}
