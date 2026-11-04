#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Regression gate (#8). Compares current history entries against the median of the last 5 main-branch
 * entries with the same key (engine, fixture, mode, cpu_throttle) AND the same host + Chrome.
 *
 *   node perf/gate.mjs --current cur.jsonl [--history perf/history.jsonl] [--thresholds perf/thresholds.json]
 *                      [--summary out.md] [--override-out override.json]
 *   env PERF_LABELS (comma list) and PR_BODY: label `perf-regression-accepted` + a reason line
 *   (`perf-regression-accepted: <why>` in the PR body) turns failures into a logged pass.
 *
 * Exit 1 = at least one armed regression (not overridden). "incomparable" (different host or Chrome)
 * is reported as its own status; it is neither pass nor fail. PRs never append to history.
 */
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { median } from './stats.mjs';

const here = dirname(fileURLToPath(import.meta.url));

export const loadEntries = (file) =>
  existsSync(file) ? readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)).filter((e) => !e.type) : [];

const HOST_FIELDS = ['kind', 'id', 'os', 'cpu', 'cores'];

/** Same host + same Chrome (the only case where two numbers may be subtracted). */
export function sameEnv(a, b) {
  if (!a.host || !b.host) return false;
  if (!a.chrome_version || a.chrome_version !== b.chrome_version) return false;
  return HOST_FIELDS.every((f) => a.host[f] != null && a.host[f] === b.host[f]);
}

/** WASM-vs-TS: same host+Chrome, or both measured in one CI job. */
export function enginesComparable(a, b) {
  if (sameEnv(a, b)) return true;
  return a.host?.kind === 'ci' && b.host?.kind === 'ci' && !!a.ci_job && a.ci_job === b.ci_job && a.chrome_version === b.chrome_version;
}

export const keyOf = (e) => [e.engine, e.fixture_id, e.mode, e.cpu_throttle ?? 1].join('|');

export function classify(metric, th) {
  return th.classes.find((c) => new RegExp(c.match).test(metric)) ?? null;
}

/** Both conditions must hold. */
export const regressed = (base, cur, c) => base > 0 && (cur - base) / base > c.rel && cur - base > c.abs;

function disarmedReason(e, metric, th) {
  const d = (th.disarmed ?? []).find(
    (x) => (x.engine ?? e.engine) === e.engine && (x.fixture_id ?? e.fixture_id) === e.fixture_id && (x.mode ?? e.mode) === e.mode &&
      (x.cpu_throttle ?? e.cpu_throttle ?? 1) === (e.cpu_throttle ?? 1) && (x.metric ?? metric) === metric,
  );
  return d ? d.reason ?? 'disarmed' : null;
}

/** Baseline entries for `cur`: last N main entries, same key, other commits, same env. */
export function baselineFor(cur, history, n) {
  const sameKey = history.filter((h) => h.branch === 'main' && keyOf(h) === keyOf(cur) && h.commit !== cur.commit);
  const comparable = sameKey.filter((h) => sameEnv(h, cur));
  return { comparable: comparable.slice(-n), anyKey: sameKey.length };
}

/** Returns rows: {entry, metric, status: pass|fail|report-only|no-baseline|incomparable, base, cur, delta, note}. */
export function evaluate(current, history, th) {
  const rows = [];
  for (const cur of current) {
    const { comparable, anyKey } = baselineFor(cur, history, th.baseline_n ?? 5);
    for (const [metric, m] of Object.entries(cur.metrics)) {
      const c = classify(metric, th);
      if (!c) continue;
      const row = { entry: cur, metric, cur: m.median, base: null, delta: null, status: 'pass', note: '' };
      if (!comparable.length) {
        row.status = anyKey ? 'incomparable' : 'no-baseline';
        row.note = anyKey ? 'baseline from a different host or Chrome version' : '';
        rows.push(row);
        continue;
      }
      const bs = comparable.map((h) => h.metrics[metric]?.median).filter((v) => typeof v === 'number');
      if (!bs.length) {
        row.status = 'no-baseline';
        rows.push(row);
        continue;
      }
      row.base = median(bs);
      row.delta = row.base > 0 ? (row.cur - row.base) / row.base : 0;
      if (regressed(row.base, row.cur, c)) {
        const why = disarmedReason(cur, metric, th);
        row.status = why ? 'report-only' : 'fail';
        row.note = why ?? '';
      }
      rows.push(row);
    }
  }
  return rows;
}

/** Override needs the label AND a written reason in the PR body. */
export function overrideFrom({ labels = [], body = '' } = {}) {
  if (!labels.includes('perf-regression-accepted')) return null;
  const m = /perf[- ]regression[- ]accepted\s*:\s*(\S[^\n]{9,})/i.exec(body);
  return m ? { reason: m[1].trim() } : { missingReason: true };
}

export function decide(rows, override) {
  const fails = rows.filter((r) => r.status === 'fail');
  if (!fails.length) return { ok: true, overridden: false };
  if (override?.reason) return { ok: true, overridden: true, reason: override.reason };
  return { ok: false, overridden: false, missingReason: !!override?.missingReason };
}

const fmt = (metric, v) => {
  if (v == null) return '—';
  if (/bytes|Size$/.test(metric)) return `${(v / 1048576).toFixed(2)} MiB`;
  return v.toFixed(1);
};

export function summaryMarkdown(rows, th, verdict) {
  const head = new Set(th.headline ?? []);
  const shown = rows.filter((r) => head.has(r.metric) || !['pass', 'no-baseline'].includes(r.status));
  const count = (s) => rows.filter((r) => r.status === s).length;
  const L = [`### perf-gate: ${verdict.ok ? (verdict.overridden ? 'PASS (override)' : 'PASS') : '**FAIL**'}`, ''];
  L.push(`${count('fail')} regression(s) · ${count('report-only')} report-only · ${count('incomparable')} incomparable · ${count('no-baseline')} no baseline · ${count('pass')} pass`, '');
  if (verdict.overridden) L.push(`> \`perf-regression-accepted\` honoured. Reason: ${verdict.reason}`, '');
  if (verdict.missingReason) L.push('> `perf-regression-accepted` label present but the PR body has no `perf-regression-accepted: <reason>` line; not honoured.', '');
  L.push('| fixture | engine | mode | cpu | metric | baseline | current | delta | status |', '|---|---|---|---:|---|---:|---:|---:|---|');
  for (const r of shown) {
    const e = r.entry;
    const d = r.delta == null ? '—' : `${r.delta >= 0 ? '+' : ''}${(r.delta * 100).toFixed(1)}%`;
    const st = r.status === 'fail' ? '**FAIL**' : r.status === 'report-only' ? `report-only (${r.note})` : r.status === 'incomparable' ? `incomparable (${r.note})` : r.status;
    L.push(`| ${e.fixture_id} | ${e.engine} | ${e.mode} | ${e.cpu_throttle}x | ${r.metric} | ${fmt(r.metric, r.base)} | ${fmt(r.metric, r.cur)} | ${d} | ${st} |`);
  }
  return L.join('\n') + '\n';
}

function parseArgs(argv) {
  const o = {};
  for (let i = 0; i < argv.length; i += 2) o[argv[i].replace(/^--/, '')] = argv[i + 1];
  return o;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const a = parseArgs(process.argv.slice(2));
  if (!a.current) throw new Error('usage: node perf/gate.mjs --current cur.jsonl [--history h.jsonl] [--summary out.md]');
  const th = JSON.parse(readFileSync(a.thresholds ?? join(here, 'thresholds.json'), 'utf8'));
  const rows = evaluate(loadEntries(a.current), loadEntries(a.history ?? join(here, 'history.jsonl')), th);
  const override = overrideFrom({ labels: (process.env.PERF_LABELS ?? '').split(',').map((s) => s.trim()), body: process.env.PR_BODY ?? '' });
  const verdict = decide(rows, override);
  const md = summaryMarkdown(rows, th, verdict);
  console.log(md);
  if (a.summary) writeFileSync(a.summary, md);
  if (verdict.overridden && a['override-out']) {
    const cur = loadEntries(a.current)[0];
    writeFileSync(a['override-out'], JSON.stringify({ type: 'override', commit: cur?.commit, date: new Date().toISOString(), reason: verdict.reason, regressions: rows.filter((r) => r.status === 'fail').map((r) => `${keyOf(r.entry)}:${r.metric}`) }) + '\n');
  }
  process.exit(verdict.ok ? 0 : 1);
}
