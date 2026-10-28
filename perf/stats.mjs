// SPDX-License-Identifier: MIT
// Aggregation for perf samples (#7): median, p95 (linear interpolation, type 7) and a percentile
// bootstrap CI. Deterministic: the bootstrap uses a seeded PRNG, so a recorded dataset always
// yields the same interval. Checked against perf/reference/aggregate-reference.json.

export const sorted = (xs) => [...xs].sort((a, b) => a - b);

/** p in [0,1]. */
export function pct(xs, p) {
  const a = sorted(xs);
  const i = (a.length - 1) * p;
  const lo = Math.floor(i);
  return a[lo] + (a[Math.ceil(i)] - a[lo]) * (i - lo);
}
export const median = (xs) => pct(xs, 0.5);
export const p95 = (xs) => pct(xs, 0.95);

// mulberry32
export function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Percentile bootstrap CI of `stat` (default median). Returns [lo, hi]. */
export function bootstrapCI(xs, stat = median, { iterations = 2000, seed = 1, level = 0.95 } = {}) {
  const rand = rng(seed);
  const n = xs.length;
  const reps = new Array(iterations);
  const buf = new Array(n);
  for (let r = 0; r < iterations; r++) {
    for (let i = 0; i < n; i++) buf[i] = xs[Math.floor(rand() * n)];
    reps[r] = stat(buf);
  }
  const tail = (1 - level) / 2;
  return [pct(reps, tail), pct(reps, 1 - tail)];
}

export function summarize(xs, opts) {
  if (!xs.length) return null;
  const [lo, hi] = bootstrapCI(xs, median, opts);
  return {
    n: xs.length,
    min: Math.min(...xs),
    median: median(xs),
    p95: p95(xs),
    max: Math.max(...xs),
    mean: xs.reduce((s, x) => s + x, 0) / xs.length,
    median_ci95: [lo, hi],
  };
}
