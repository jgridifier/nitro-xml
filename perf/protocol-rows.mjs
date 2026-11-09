// SPDX-License-Identifier: MIT
/**
 * Protocol grouping shared by the report, the inventory and the consistency test (#10). The grouping key is the gate's
 * own `protocolId` (gate.mjs PROTOCOL_FIELDS: Chrome, rerun marker, fixture sha256, host fields, node, concurrent runs);
 * nothing here re-implements it. Samples of different protocol groups are never pooled into one n, median or p95.
 */
import { createHash } from 'node:crypto';
import { readFileSync, existsSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { PROTOCOL_FIELDS, protocolOf, protocolId } from './gate.mjs';

export { PROTOCOL_FIELDS, protocolOf, protocolId };

/** A raw run row (run.mjs output) seen through the history-entry shape the gate's key function reads. */
export const rowAsEntry = (r, rerun) => ({
  chrome_version: r.chrome ?? r.host?.chrome_version ?? null,
  rerun: !!rerun,
  fixture_sha256: r.fixture_sha256 ?? null,
  host: { ...(r.host ?? {}), cpu: r.host?.cpu ?? r.host?.cpu_model },
});

/** Short stable id of a protocol group. */
export const groupId = (entryLike) => `P${createHash('sha256').update(protocolId(entryLike)).digest('hex').slice(0, 6)}`;

/** Labels from the fields that differ between the groups present (always includes rerun). */
export function groupLabels(entryLikes) {
  const by = new Map();
  for (const e of entryLikes) by.set(groupId(e), protocolOf(e));
  const keys = [...by.values()];
  const varying = PROTOCOL_FIELDS.filter((f) => f === 'rerun' || new Set(keys.map((k) => JSON.stringify(k[f]))).size > 1);
  return new Map([...by].map(([id, p]) => [id, varying.map((f) => `${f.replace(/^host\./, '')}=${p[f]}`).join(', ')]));
}

/**
 * Per-row `rerun` marker. The assembled ts-combined-<sha>.jsonl is the concatenation of the original sweep
 * (ts-4451709.jsonl), the re-run rows promoted for xlsx S/M, and the L/XL sweep; the layout is verified row by row
 * (it throws when it does not hold) so a row can never be mislabelled. Any other file: row.rerun, default false.
 */
export function rerunFlags(rows, inputPath, manifest) {
  const m = /ts-combined-([0-9a-f]+)\.jsonl$/.exec(inputPath);
  if (!m) return rows.map((r) => !!r.rerun);
  const dir = dirname(inputPath);
  const rd = (f) => readFileSync(join(dir, f), 'utf8').split('\n').filter(Boolean);
  const xlsxSM = new Set(manifest.filter((x) => x.format === 'xlsx' && ['S', 'M'].includes(x.tier)).map((x) => x.id));
  const sm = rd('ts-4451709.jsonl');
  const promoted = rd('ts-4451709.rerun.jsonl').filter((l) => xlsxSM.has(JSON.parse(l).fixture));
  const lxlFile = `ts-lxl-${m[1]}.jsonl`;
  const lxl = existsSync(join(dir, lxlFile)) ? rd(lxlFile) : [];
  const warm4xFile = `ts-warm4x-${m[1]}.jsonl`;
  const warm4x = existsSync(join(dir, warm4xFile)) ? rd(warm4xFile) : [];
  const expect = [...sm.map((l) => [l, false]), ...promoted.map((l) => [l, true]), ...warm4x.map((l) => [l, false]), ...lxl.map((l) => [l, false])];
  if (expect.length !== rows.length) throw new Error(`${basename(inputPath)} is not original + promoted re-run + L/XL (${rows.length} rows vs ${expect.length})`);
  return rows.map((r, i) => {
    if (JSON.stringify(r) !== JSON.stringify(JSON.parse(expect[i][0]))) throw new Error(`${basename(inputPath)} row ${i} differs from its source file; cannot tell which protocol produced it`);
    return expect[i][1];
  });
}

/** fixture|mode|cpu -> [{ gid, n }] sorted by gid, from run rows (+ their rerun flags). */
export function groupsFromRows(rows, flags) {
  const out = new Map();
  rows.forEach((r, i) => {
    const k = `${r.fixture}|${r.mode}|${r.cpuThrottle ?? 1}`;
    const g = groupId(rowAsEntry(r, flags[i]));
    const list = out.get(k) ?? out.set(k, new Map()).get(k);
    list.set(g, (list.get(g) ?? 0) + 1);
  });
  return out;
}

/** The same from history entries (n summed per group), tier-aggregate entries skipped. */
export function groupsFromHistory(hist, isFixture = () => true) {
  const out = new Map();
  for (const e of hist) {
    if (e.type || e.n_fixtures || !isFixture(e.fixture_id)) continue;
    const k = `${e.fixture_id}|${e.mode}|${e.cpu_throttle ?? 1}`;
    const g = groupId(e);
    const list = out.get(k) ?? out.set(k, new Map()).get(k);
    list.set(g, (list.get(g) ?? 0) + e.n);
  }
  return out;
}

/** Report-side groups: the assembled dataset plus its re-run companion, exactly the rows BASELINE.md is built from. */
export function loadReportGroups(baselinesDir, manifest, sha) {
  const rd = (f) => readFileSync(join(baselinesDir, f), 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));
  const input = join(baselinesDir, `ts-combined-${sha}.jsonl`);
  const rows = rd(`ts-combined-${sha}.jsonl`);
  const rerun = rd(`ts-combined-${sha}.rerun.jsonl`);
  return groupsFromRows([...rows, ...rerun], [...rerunFlags(rows, input, manifest), ...rerun.map(() => true)]);
}

/** Cells whose per-group n lists differ between two group maps (group ids may differ; the sorted n lists must not). */
export function nMismatches(a, b) {
  const sig = (m) => (m ? [...m.values()].sort((x, y) => x - y).join('+') : '-');
  return [...new Set([...a.keys(), ...b.keys()])].filter((k) => sig(a.get(k)) !== sig(b.get(k))).map((k) => `${k}: ${sig(a.get(k))} vs ${sig(b.get(k))}`);
}
