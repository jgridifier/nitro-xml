#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * TS baseline report (#10): reads perf/baselines/ts-<commit>.jsonl (rows from `node perf/run.mjs --corpus
 * manifest`, merged by baseline-sweep.mjs --merge) and writes perf/BASELINE.md and the static page
 * docs/perf/baseline/index.html. Node ESM, no dependencies beyond ./stats.mjs.
 * Usage: node perf/baseline-report.mjs <input.jsonl> [failures.jsonl]
 *   failures default to <input>.failures.jsonl when present. BASELINE_DATE=YYYY-MM-DD pins the date.
 */
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { median, p95 } from './stats.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
if (!process.argv[2]) throw new Error('usage: node perf/baseline-report.mjs perf/baselines/ts-<commit>.jsonl [failures.jsonl]');
const input = resolve(process.argv[2]);
const lines = (f) => readFileSync(f, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));
const rows = lines(input);
const failFile = process.argv[3] ? resolve(process.argv[3]) : input.replace(/\.jsonl$/, '.failures.jsonl');
const failureRows = existsSync(failFile) ? lines(failFile) : [];
const rerunFile = input.replace(/\.jsonl$/, '.rerun.jsonl');
const rerunRows = existsSync(rerunFile) ? lines(rerunFile) : [];
const rerunFailFile = input.replace(/\.jsonl$/, '.rerun-failures.jsonl');
const rerunFailRows = existsSync(rerunFailFile) ? lines(rerunFailFile) : [];
const COMMIT = /ts-([0-9a-f]+)\.jsonl$/.exec(input)?.[1] ?? 'unknown';
const manifest = JSON.parse(readFileSync(`${root}/fixtures/manifest.json`, 'utf8')).fixtures;

const DATE = process.env.BASELINE_DATE ?? new Date().toISOString().slice(0, 10);
const GAP = 'GAP - provisional, pending #117 (L/XL fixtures not generated; pad.bin to be replaced)';
const PRIMARY = ['first_paint_ms', 'parse_ms', 'first_render_ms', 'full_render_ms', 'tbt_ms', 'JSHeapUsedSize', 'all_slides_ms'];
const FORMATS = ['xlsx', 'docx', 'pptx'];
const TIERS = ['S', 'M', 'L', 'XL'];
const CELLS = [['cold', 1], ['cold', 4], ['warm', 1], ['warm', 4]];
const cellName = ([m, t]) => `${m} ${t}x`;
const cellNames = CELLS.map(cellName);
const MIB = 1024 * 1024;
const disp = (k) => {
  if (k.endsWith('_bytes')) return [k.replace(/_bytes$/, '_MiB'), MIB];
  if (k === 'JSHeapUsedSize' || k === 'JSHeapTotalSize') return [`${k} (MiB)`, MIB];
  return [k, 1];
};
const fmt = (v) => (v === undefined || v === null || Number.isNaN(v) ? '-' : Math.abs(v) >= 100 ? v.toFixed(0) : v.toFixed(1).replace(/\.0$/, ''));

// ---- group rows: fixture -> cell -> metric -> values
const group = (rs) => {
  const byFixture = new Map();
  for (const r of rs) {
    if (!byFixture.has(r.fixture)) byFixture.set(r.fixture, { id: r.fixture, format: r.format, tier: r.tier, cells: new Map() });
    const f = byFixture.get(r.fixture);
    const ck = cellName([r.mode, r.cpuThrottle]);
    if (!f.cells.has(ck)) f.cells.set(ck, { n: 0, vals: {} });
    const c = f.cells.get(ck);
    c.n++;
    for (const [k, v] of Object.entries(r.metrics)) if (typeof v === 'number') (c.vals[k] ??= []).push(v);
  }
  return byFixture;
};
const byFixture = group(rows);
const rerunBy = group(rerunRows);
const chromes = [...new Set(rows.map((r) => r.chrome))].sort();
const hostKey = (h) => JSON.stringify(h ?? null);
const hosts = [...new Map(rows.map((r) => [hostKey(r.host), r.host])).values()].filter(Boolean);
const stat = (f, cell, k, fn) => {
  const xs = f.cells.get(cell)?.vals[k];
  return xs?.length ? fn(xs) / disp(k)[1] : undefined;
};
const complete = (f) => cellNames.every((c) => f.cells.has(c));
const fixtures = [...byFixture.values()].sort((a, b) => FORMATS.indexOf(a.format) - FORMATS.indexOf(b.format) || TIERS.indexOf(a.tier) - TIERS.indexOf(b.tier) || a.id.localeCompare(b.id));
const mrow = new Map(manifest.map((m) => [m.id, m]));
const bytesOf = (f) => mrow.get(f.id)?.bytes ?? mrow.get(f.id.replace(/\.[a-z]+$/, ''))?.bytes;

// ---- failures: one entry per fixture
const failed = new Map();
for (const x of failureRows) {
  const e = failed.get(x.fixture) ?? { fixture: x.fixture, cells: new Set(), error: String(x.error ?? '').split('\n')[0].slice(0, 160), kind: /Timeout|timed out/.test(x.error ?? '') ? 'timeout' : 'error' };
  e.cells.add(cellName([x.mode, x.cpuThrottle]));
  failed.set(x.fixture, e);
}
const unmeasured = new Set([...failed.keys()]); // failed in at least one cell: left out of the aggregates below
const kept = fixtures.filter((f) => !unmeasured.has(f.id) && complete(f));

// ---- coverage
const coverage = FORMATS.flatMap((fm) => TIERS.map((t) => {
  const inManifest = manifest.filter((m) => m.format === fm && m.tier === t);
  const measured = kept.filter((f) => f.format === fm && f.tier === t).length;
  const bad = fixtures.filter((f) => f.format === fm && f.tier === t && !kept.includes(f)).length + [...failed.values()].filter((e) => !byFixture.has(e.fixture) && mrow.get(e.fixture)?.format === fm && mrow.get(e.fixture)?.tier === t).length;
  return { fm, t, manifest: inManifest.length, measured, bad };
}));

// ---- rankings (individual fixtures, cold 1x)
const rank = (k) => kept.map((f) => [f, stat(f, 'cold 1x', k, median)]).filter(([, v]) => v !== undefined).sort((a, b) => b[1] - a[1]).slice(0, 5);
const rankRows = (xs) => xs.map(([f, v], i) => [i + 1, f.id, f.format, f.tier, bytesOf(f) ?? '-', fmt(v)]);
const slowest = rank('first_paint_ms');
const hungriest = rank('JSHeapUsedSize');

// ---- xlsx-M warm 1x TTI (the #10 finding): old definition vs fixed, per fixture, from the same samples
const ttiRows = kept.filter((f) => f.format === 'xlsx' && f.tier === 'M').map((f) => {
  const ok = stat(f, 'warm 1x', 'tti_ms', median) !== undefined;
  return [f.id, fmt(stat(f, 'warm 1x', 'tti_with_interaction_ms', median)), fmt(stat(f, 'warm 1x', 'tti_with_interaction_ms', p95)), ok ? fmt(stat(f, 'warm 1x', 'tti_ms', median)) : '-', ok ? fmt(stat(f, 'warm 1x', 'tti_ms', p95)) : '-', fmt(stat(f, 'warm 1x', 'first_paint_ms', median)), fmt(stat(f, 'warm 1x', 'first_paint_ms', p95))];
});
const bimodal = kept.filter((f) => f.format === 'xlsx' && f.tier === 'M').filter((f) => stat(f, 'warm 1x', 'tti_ms', p95) > 3 * stat(f, 'warm 1x', 'tti_ms', median)).map((f) => f.id);
const perfM = byFixture.get('gen-xlsx-cells-100k'); // = xlsx-M of perf/fixtures.json

// ---- tier aggregates: median / min..max over fixtures of the per-fixture median
const tierKeys = (fs) => {
  const all = new Set();
  for (const f of fs) for (const c of f.cells.values()) for (const k of Object.keys(c.vals)) all.add(k);
  return { prim: PRIMARY.filter((k) => all.has(k)), rest: [...all].filter((k) => !PRIMARY.includes(k)).sort() };
};
const tierBody = (fs, ks) => ks.map((k) => [disp(k)[0], ...cellNames.map((c) => {
  const ms = fs.map((f) => stat(f, c, k, median)).filter((v) => v !== undefined);
  return ms.length ? `${fmt(median(ms))} (${fmt(Math.min(...ms))}..${fmt(Math.max(...ms))})` : '-';
})]);

// ---- document model: blocks rendered to Markdown and HTML
const hostRows = hosts.flatMap((h, i) => Object.entries(h).map(([k, v]) => [hosts.length > 1 ? `host ${i + 1}: ${k}` : `host: ${k}`, v]));
const header = [
  ['TS engine', 'upstream ChristopherVR/ooxml @ e66aaec'],
  ['Measured at', `nitro-xml commit ${COMMIT} plus the harness changes of the PR that added this file`],
  ['Chrome', chromes.join(', ')],
  ['Runner', 'local Linux box via `node perf/baseline-sweep.mjs` -> `node perf/run.mjs --corpus manifest --engine ts` (cold n=10; warm n=20 + 1 discarded warm-up; CPU 1x and 4x)'],
  ['Date', DATE],
  ['Source data', `perf/baselines/${basename(input)} (${rows.length} samples, ${kept.length} fixtures x 4 cells)`],
  ...hostRows,
];
const NOTE1 = 'WASM-beats-TS comparison applies only to S/M until #117 lands; L/XL are provisional.';
const NOTE2 = 'Appending to `history.jsonl` and dashboard visibility are pending #8/#9, so #10 stays open.';
const gapRows = FORMATS.flatMap((fm) => ['L', 'XL'].map((t) => [fm, t, coverage.find((c) => c.fm === fm && c.t === t).manifest, GAP]));
const short = coverage.filter((c) => ['S', 'M'].includes(c.t) && c.measured < 3);

const doc = [];
const H = (n, t) => doc.push({ h: n, t });
const P = (t) => doc.push({ p: t });
const T = (head, body) => doc.push({ head, body });

H(1, 'TypeScript engine baseline (#10)');
T(['Item', 'Value'], header);
P('Times are ms unless noted; heap is MiB. Per-fixture cells are `median / p95` over the iterations of one fixture; per-tier cells are `median (min..max)` over the fixtures of the tier of each fixture\'s median. Cells: mode (cold/warm) x CPU throttle (1x/4x).');
H(2, 'Status');
P(`- **${NOTE1}**`);
P(`- ${NOTE2}`);
P(`- Corpus: every S and M xlsx/docx/pptx row of \`fixtures/manifest.json\` that exists on disk under \`fixtures/\` (the demo/ copies of the same bytes are not re-measured). ${short.length ? `**Fewer than 3 measured fixtures for: ${short.map((c) => `${c.fm} ${c.t} (${c.measured})`).join(', ')}.**` : 'Every S and M tier of every format has at least 3 measured fixtures.'}`);
P(`- Fixture ids are the manifest ids; the six ids of the first baseline map to \`gen-xlsx-cells-1k\` (xlsx-S), \`gen-xlsx-cells-100k\` (xlsx-M), \`gen-docx-pages-10\` (docx-S), \`gen-docx-pages-100\` (docx-M), \`gen-pptx-slides-10\` (pptx-S), \`gen-pptx-slides-50\` (pptx-M).`);
P(`- Two runs shared the machine at a time (\`concurrent_runs\` in the host fingerprint), on disjoint fixtures of the same cell, to fit the sweep in time; a 3-way concurrency check on this box moved the median first_paint of a small xlsx by about 7%, so absolute numbers sit slightly above a serial run. Compare like with like (same host, same \`concurrent_runs\`).`);
H(2, 'Corpus coverage');
T(['format', 'tier', 'in manifest', 'measured', 'excluded (failed or incomplete)'], coverage.filter((c) => ['S', 'M'].includes(c.t)).map((c) => [c.fm, c.t, c.manifest, c.measured, c.bad]));
P('"in manifest" includes rows that are not redistributable and so not on disk (`upstream-noredist-*`), which cannot be measured.');
H(2, 'Headline: first_paint median per tier (ms)');
T(['format', 'tier', 'fixtures', ...cellNames], FORMATS.flatMap((fm) => ['S', 'M'].map((t) => {
  const fs = kept.filter((f) => f.format === fm && f.tier === t);
  return [fm, t, fs.length, ...cellNames.map((c) => { const v = fs.map((f) => stat(f, c, 'first_paint_ms', median)).filter((x) => x !== undefined); return v.length ? `${fmt(median(v))} (${fmt(Math.min(...v))}..${fmt(Math.max(...v))})` : '-'; })];
})));
H(2, 'Rankings (individual fixtures)');
P('**5 slowest fixtures** (cold 1x first_paint median):');
T(['#', 'fixture', 'format', 'tier', 'bytes', 'first_paint_ms'], rankRows(slowest));
P('**5 most memory-hungry fixtures** (cold 1x JSHeapUsedSize median):');
T(['#', 'fixture', 'format', 'tier', 'bytes', 'JSHeapUsedSize (MiB)'], rankRows(hungriest));

H(2, 'xlsx-M warm 1x TTI: bimodal p95 (investigated)');
P(`The first baseline (7ce81c1) had xlsx-M warm 1x \`tti_ms\` median 946 ms but p95 8228 ms. **Cause: a harness metric bug, not the engine.** \`tti_ms\` was "first_render, or the end of the last long task up to full_render". For xlsx the harness itself scrolls the sheet from first_render to full_render (about 8.4 s, 166 steps of 600 px), and each step is about 50-70 ms of main-thread work, i.e. right on the 50 ms long-task line. In about a quarter of the loads one scroll step crossed it, and TTI jumped to the end of that task, anywhere between 2 s and 9 s into the scroll. Evidence (n=30 and n=24 warm 1x loads of the 100k-cell xlsx on this box):`);
P('- Every load with a high TTI had a long task of 50-120 ms starting well after first_render (e.g. at 7947, 6355, 7350 ms) while first_paint was normal (about 900 ms); loads with a low TTI had none after first_render. Step latencies of the high-TTI loads contained the 100-160 ms outliers (see the old jsonl: runs 10, 13, 14).');
P('- Not GC: a Chromium trace (v8, devtools.timeline) of 24 loads overlapped only 1 of about 25 late long tasks with a MajorGC. Not host contention: /proc/stat steal time was 0.00-0.27% on every load. Not cache eviction or font load: warm loads are served from the same disk cache, the late task is in the scroll phase after all fetches finished (fetch_ms about 3 ms), and the page loads no fonts.');
P('- Fix: `tti_ms` now ends at `interact_start` (the moment the scripted interaction begins, right after first_paint) for all three hosts, so tasks caused by the harness\'s own scroll / page jumps / slide walk no longer count (they remain in `tbt_ms`, `long_task_*` and the step latencies). The old definition is kept as `tti_with_interaction_ms`.');
T(['xlsx-M fixture (warm 1x)', 'old TTI median', 'old TTI p95', 'fixed TTI median', 'fixed TTI p95', 'first_paint median', 'first_paint p95'], ttiRows);
P(bimodal.length
  ? `**UNEXPLAINED / DO-NOT-ARM:** after the fix, ${bimodal.join(', ')} still has a warm 1x TTI p95 more than 3x its median. Do not arm a TTI gate on those cells until it is explained.`
  : `After the fix no measured xlsx-M fixture (${ttiRows.length} in the sweep aggregates) has a warm 1x TTI p95 above 3x its median; no cell is flagged unexplained/do-not-arm, but this rests on ${ttiRows.length} fixture(s) because the other xlsx-M timeouts are excluded. Remaining spread is first_paint noise on this shared box. Note that TTI is now close to first_render by construction (the harness starts interacting right after first_paint); a quiet-window TTI is still TODO(#8).`);

H(2, 'Excluded fixtures');
P(failed.size ? 'The TS engine (or the harness page) failed these fixtures; they are not part of any number above. Failures are TS-engine behaviour recorded as data, not harness faults, unless noted.' : 'No fixture failed.');
if (failed.size) {
  const nTimeout = [...failed.values()].filter((e) => e.kind === 'timeout').length;
  P(`${nTimeout} of ${failed.size} excluded fixtures are **timeouts** (the page did not finish within 120 s, or a host wait expired); the rest are TS-engine errors. Timed-out rows stay in this history marked as timeouts (\`${basename(failFile)}\` has every failed chunk row, one per fixture and cell); they are never replaced by re-run numbers.`);
  T(['fixture', 'kind', 'cells failed', 'first error line'], [...failed.values()].map((e) => [e.fixture, e.kind, [...e.cells].join(', '), e.error]));
  P('**Were the timeouts memory pressure?** No evidence of it. The same fixtures timed out in every cell, cold and warm, with about 8 GiB free, and re-running alone on an idle machine reproduced the xlsx timeouts; the cause was the unbounded scroll loop in the xlsx host page (see the re-run section below). The docx/pptx rows are different: `real-docx-poi-Bug54849` and `real-docx-poi-drawing` timed out in the sweep and, where they were re-measured, finished, so their cause is **unexplained** (contention or flakiness is possible, not shown). `gen-pptx-placeholder` and `real-pptx-poi-bug59273` never render slide 1, `real-docx-poi-bug65649` throws in the TS engine, and `upstream-fb3af3f7b5f7` is password protected.');
}
if (rerunRows.length || rerunFailRows.length) {
  const rf = [...rerunBy.values()].filter((f) => f.cells.has('cold 1x')).sort((a, b) => FORMATS.indexOf(a.format) - FORMATS.indexOf(b.format) || TIERS.indexOf(a.tier) - TIERS.indexOf(b.tier) || a.id.localeCompare(b.id));
  const stillFailed = new Set(rerunFailRows.map((x) => x.fixture));
  H(2, 'Re-run of the excluded fixtures after the xlsx scroll fix (provisional, separate from the sweep)');
  P('**Cause of the xlsx timeouts: a harness bug, not the engine and not memory pressure.** The xlsx page scrolls the sheet 600 px per step until the grid stops moving or `lastRow` is reached. Fixtures from `fixtures/manifest.json` carry no `lastRow` hint and the TS grid is far taller than any sheet, so the scroll ran for minutes and the page never set `done`; the runner gave up after 120 s. Evidence: `real-xlsx-poi-test` (8.5 KB) timed out again when run alone on an idle machine (about 8 GiB free), and finished in about 10 s once the loop was bounded to 200 steps when no hint is given (`perf/pages/xlsx.html`).');
  P('**The sweep rows stay as recorded: they are timeouts.** The rows above and `' + basename(failFile) + '` are not edited, and the numbers below are not substituted into the aggregates, rankings or the table of every fixture; they are a second dataset (`' + basename(rerunFile) + '`, same protocol and host, 2 concurrent workers, fixed harness) and are **provisional**. The scripted scroll is capped at 200 steps for these fixtures, so their full_render and step metrics are not comparable with the sweep rows of fixtures that have a `lastRow` hint.');
  const perCell = cellNames.map((c) => `${c}: ${[...rerunBy.values()].filter((f) => f.cells.has(c)).length}`).join(', ');
  P(`${rf.length} of ${failed.size} excluded fixtures were measured in the re-run${rerunFailRows.length ? `; ${stillFailed.size} still fail in at least one re-run cell (${[...stillFailed].join(', ')})` : ''}. **The re-run is incomplete: it was stopped after about 24 h** (warm chunks took hours each). Fixtures measured per cell: ${perCell}. Cold 1x and cold 4x are complete; warm 1x covers chunks 0-3 of 8 only and warm 4x was not run, so "-" in a cell below means not re-run, not zero.`);
  for (const fm of FORMATS) for (const t of ['S', 'M']) {
    const fs = rf.filter((f) => f.format === fm && f.tier === t);
    if (!fs.length) continue;
    H(3, `re-run ${fm} ${t}: ${fs.length} fixtures`);
    T(['fixture', 'bytes', ...cellNames, 'heap MiB'], fs.map((f) => [f.id, bytesOf(f) ?? '-', ...cellNames.map((c) => `${fmt(stat(f, c, 'first_paint_ms', median))} / ${fmt(stat(f, c, 'first_paint_ms', p95))}`), fmt(stat(f, 'cold 1x', 'JSHeapUsedSize', median))]));
  }
}

H(2, 'L/XL');
T(['format', 'tier', 'in manifest', 'status'], gapRows);
P('The sweep measured S and M only. Any L/XL rows from earlier runs used the pre-#117 padded fixtures (pad.bin); they are provisional and superseded, not final, and are not in the aggregates above.');
P(NOTE1);

H(2, 'Per format and tier (aggregate over fixtures)');
for (const fm of FORMATS) for (const t of ['S', 'M']) {
  const fs = kept.filter((f) => f.format === fm && f.tier === t);
  const { prim, rest } = tierKeys(fs);
  H(3, `${fm} ${t}: ${fs.length} fixtures`);
  T(['metric', ...cellNames.map((c) => `${c} (n=${CELLS[cellNames.indexOf(c)][0] === 'cold' ? 10 : 20}/fixture, Chrome ${chromes.join('/')})`)], tierBody(fs, prim));
  if (rest.length) { P('Other metrics:'); T(['metric', ...cellNames], tierBody(fs, rest)); }
}
H(2, 'Every fixture');
P('first_paint `median / p95` per cell, then cold 1x JSHeapUsedSize median (MiB) and warm 1x tti_ms median. The full metric set per iteration is in the jsonl.');
for (const fm of FORMATS) for (const t of ['S', 'M']) {
  const fs = kept.filter((f) => f.format === fm && f.tier === t);
  H(3, `${fm} ${t}`);
  T(['fixture', 'bytes', ...cellNames, 'heap MiB', 'tti warm 1x'], fs.map((f) => [f.id, bytesOf(f) ?? '-', ...cellNames.map((c) => `${fmt(stat(f, c, 'first_paint_ms', median))} / ${fmt(stat(f, c, 'first_paint_ms', p95))}`), fmt(stat(f, 'cold 1x', 'JSHeapUsedSize', median)), fmt(stat(f, 'warm 1x', 'tti_ms', median))]));
}

// ---- Markdown
const esc = (s) => String(s).replace(/\|/g, '\\|').replace(/\n/g, ' ');
const mdTable = ({ head, body }) => [`| ${head.map(esc).join(' | ')} |`, `| ${head.map((_, i) => (i ? '---:' : '---')).join(' | ')} |`, ...body.map((r) => `| ${r.map(esc).join(' | ')} |`), ''];
const md = ['<!-- generated by perf/baseline-report.mjs; do not edit -->', ''];
for (const b of doc) {
  if (b.h) md.push(`${'#'.repeat(b.h)} ${b.t}`, '');
  else if (b.p) md.push(b.p, '');
  else md.push(...mdTable(b));
}
writeFileSync(`${root}/perf/BASELINE.md`, md.join('\n').replace(/^(# .*)\n\n(<!--.*-->)$/m, '$1'));

// ---- HTML
const h = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const inline = (s) => h(s).replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>').replace(/`(.+?)`/g, '<code>$1</code>');
const htable = ({ head, body }) => `<div class="scroll"><table><thead><tr>${head.map((x) => `<th>${h(x)}</th>`).join('')}</tr></thead><tbody>${body.map((r) => `<tr>${r.map((x, i) => (i ? `<td>${h(x)}</td>` : `<th scope="row">${h(x)}</th>`)).join('')}</tr>`).join('')}</tbody></table></div>`;
const body = doc.map((b) => (b.h ? (b.h === 1 ? '' : `<h${b.h}>${inline(b.t)}</h${b.h}>`) : b.p ? (b.p.startsWith('- ') ? `<ul><li>${inline(b.p.slice(2))}</li></ul>` : `<p>${inline(b.p)}</p>`) : htable(b))).join('\n');
const page = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<!-- SPDX-License-Identifier: MIT; generated by perf/baseline-report.mjs -->
<title>TS engine baseline - nitro-xml perf</title>
<style>
:root{--navy:#0b1f3a;--ink:#000;--paper:#fff}
*{box-sizing:border-box}
body{margin:0;font:16px/1.5 system-ui,sans-serif;color:var(--ink);background:var(--paper)}
header{background:var(--navy);color:var(--paper);padding:1rem}
header a{color:var(--paper)}
main{padding:1rem;max-width:80rem;margin:0 auto}
h1{margin:.5rem 0;font-size:1.5rem}
h2{color:var(--navy);border-bottom:2px solid var(--navy);padding-bottom:.2rem;margin-top:2rem}
h3{color:var(--navy);margin:1.5rem 0 .5rem}
.scroll{overflow-x:auto}
table{border-collapse:collapse;width:100%;font-size:.85rem}
th,td{border:1px solid var(--navy);padding:.3rem .5rem;text-align:right;white-space:nowrap}
th:first-child,td:first-child{text-align:left}
thead th{background:var(--navy);color:var(--paper)}
tbody th{text-align:left;font-weight:600}
ul{margin:.25rem 0}
</style></head><body>
<header><a href="../../">&larr; nitro-xml</a><h1>TypeScript engine baseline (#10)</h1></header>
<main>
${body}
</main></body></html>
`;
mkdirSync(`${root}/docs/perf/baseline`, { recursive: true });
writeFileSync(`${root}/docs/perf/baseline/index.html`, page);
console.log(`wrote perf/BASELINE.md and docs/perf/baseline/index.html (${rows.length} samples, ${kept.length} fixtures, ${failed.size} excluded)`);
