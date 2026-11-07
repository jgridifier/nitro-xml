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
import { DESIGN_N, STATUSES, STATUS_LABEL, loadProvenance, statusFor, sampleN, isThin, cellText, medianText, aggregateText } from './provenance.mjs';

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
const COMMIT = /ts-(?:combined-)?([0-9a-f]+)\.jsonl$/.exec(input)?.[1] ?? 'unknown';
const provFile = `${root}/perf/baselines/failure-provenance.json`;
const prov = loadProvenance(provFile);
const manifest = JSON.parse(readFileSync(`${root}/fixtures/manifest.json`, 'utf8')).fixtures;

const DATE = process.env.BASELINE_DATE ?? new Date().toISOString().slice(0, 10);
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
// Per-cell sample labelling: n = number of finite samples of the metric in the cell. Medians (and p95 where shown)
// are always printed with n; a cell below the harness design count (DESIGN_N) is labelled "thin sample". No CI is
// computed here and no speed-win claim is made.
const modeOf = (cell) => cell.split(' ')[0];
const valsOf = (f, cell, k) => f.cells.get(cell)?.vals[k]?.map((v) => v / disp(k)[1]);
const nOf = (f, cell, k = 'first_paint_ms') => sampleN(valsOf(f, cell, k));
const stat = (f, cell, k, fn) => { const xs = valsOf(f, cell, k); return xs?.length ? fn(xs) : undefined; };
const mp = (f, cell, k) => cellText(valsOf(f, cell, k), median, p95, fmt, modeOf(cell));
const mOnly = (f, cell, k) => medianText(valsOf(f, cell, k), median, fmt, modeOf(cell));
const thinCell = (f, cell, k = 'first_paint_ms') => isThin(nOf(f, cell, k), modeOf(cell));
// Accept fixtures that have at least cold 1x + cold 4x so xlsx rerun-promoted rows (missing warm 4x) are included.
const complete = (f) => f.cells.has('cold 1x') && f.cells.has('cold 4x');
const fixtures = [...byFixture.values()].sort((a, b) => FORMATS.indexOf(a.format) - FORMATS.indexOf(b.format) || TIERS.indexOf(a.tier) - TIERS.indexOf(b.tier) || a.id.localeCompare(b.id));
const mrow = new Map(manifest.map((m) => [m.id, m]));
const bytesOf = (f) => mrow.get(f.id)?.bytes ?? mrow.get(f.id.replace(/\.[a-z]+$/, ''))?.bytes;

// ---- failures: one entry per fixture; the status of every failed cell comes from failure-provenance.json
// (perf/build-failure-provenance.mjs). A cell with no provenance record is `unknown`, never guessed.
const failed = new Map();
for (const x0 of failureRows) {
  // Some chunk-level rows carry the fixture only inside the error text ("FAIL <id> ts: ..."); blank rows are dropped.
  const x = x0.fixture ? x0 : { ...x0, fixture: /^FAIL (\S+) /.exec(x0.error ?? '')?.[1] };
  if (!x.fixture) continue;
  const cell = cellName([x.mode, x.cpuThrottle ?? 1]);
  const e = failed.get(x.fixture) ?? { fixture: x.fixture, cells: new Set(), status: new Map(), error: '' };
  e.cells.add(cell);
  const st = statusFor(prov, x.fixture, x.mode, x.cpuThrottle ?? 1);
  e.status.set(cell, st);
  const pe = prov.byCell.get(`${x.fixture}|${x.mode}|${x.cpuThrottle ?? 1}`)?.error;
  if (!e.error && pe) e.error = pe;
  failed.set(x.fixture, e);
}
for (const e of failed.values()) e.kind = [...e.status.values()].includes('observed_timeout') ? 'timeout' : [...e.status.values()].includes('observed_error') ? 'error' : 'not observed';
// A fixture is excluded from aggregates only when it fails in a cell required by complete()
// (cold 1x or cold 4x). Warm failures are shown in the excluded table but don't prevent
// inclusion in cold-only aggregates. This lets promoted xlsx rerun fixtures (some of which had
// warm timeouts in the original sweep) appear in the aggregate tables.
const unmeasured = new Set(
  [...failed.entries()]
    .filter(([, e]) => [...e.cells].some((c) => c === 'cold 1x' || c === 'cold 4x'))
    .map(([id]) => id),
);
const kept = fixtures.filter((f) => !unmeasured.has(f.id) && complete(f));

// ---- coverage
const coverage = FORMATS.flatMap((fm) => TIERS.map((t) => {
  const inManifest = manifest.filter((m) => m.format === fm && m.tier === t);
  const measured = kept.filter((f) => f.format === fm && f.tier === t).length;
  const bad = fixtures.filter((f) => f.format === fm && f.tier === t && !kept.includes(f)).length + [...failed.values()].filter((e) => !byFixture.has(e.fixture) && mrow.get(e.fixture)?.format === fm && mrow.get(e.fixture)?.tier === t).length;
  return { fm, t, manifest: inManifest.length, measured, bad };
}));

// ---- rankings (individual fixtures, cold 1x)
const rank = (k) => kept.filter((f) => !thinCell(f, 'cold 1x', k)).map((f) => [f, stat(f, 'cold 1x', k, median)]).filter(([, v]) => v !== undefined).sort((a, b) => b[1] - a[1]).slice(0, 5);
const rankRows = (xs, k) => xs.map(([f, v], i) => [i + 1, f.id, f.format, f.tier, bytesOf(f) ?? '-', fmt(v), nOf(f, 'cold 1x', k)]);
const slowest = rank('first_paint_ms');
const hungriest = rank('JSHeapUsedSize');

// ---- xlsx-M warm 1x TTI (the #10 finding): old definition vs fixed, per fixture, from the same samples
const ttiRows = kept.filter((f) => f.format === 'xlsx' && f.tier === 'M').map((f) => [f.id, mp(f, 'warm 1x', 'tti_with_interaction_ms'), mp(f, 'warm 1x', 'tti_ms'), mp(f, 'warm 1x', 'first_paint_ms')]);
const bimodal = kept.filter((f) => f.format === 'xlsx' && f.tier === 'M').filter((f) => !thinCell(f, 'warm 1x', 'tti_ms')).filter((f) => stat(f, 'warm 1x', 'tti_ms', p95) > 3 * stat(f, 'warm 1x', 'tti_ms', median)).map((f) => f.id);
const perfM = byFixture.get('gen-xlsx-cells-100k'); // = xlsx-M of perf/fixtures.json

// ---- tier aggregates: median / min..max over fixtures of the per-fixture median
const tierKeys = (fs) => {
  const all = new Set();
  for (const f of fs) for (const c of f.cells.values()) for (const k of Object.keys(c.vals)) all.add(k);
  return { prim: PRIMARY.filter((k) => all.has(k)), rest: [...all].filter((k) => !PRIMARY.includes(k)).sort() };
};
const tierBody = (fs, ks) => ks.map((k) => [disp(k)[0], ...cellNames.map((c) => aggregateText(fs.map((f) => ({ vals: valsOf(f, c, k) })), median, fmt, modeOf(c)))]);

// ---- document model: blocks rendered to Markdown and HTML
// Compute actual iteration range per mode so the runner line is accurate for mixed-n datasets.
const iterRange = (mode) => {
  const fxMaxIter = new Map();
  for (const r of rows) if (r.mode === mode) {
    const key = `${r.fixture}|${r.engine}`;
    fxMaxIter.set(key, Math.max(fxMaxIter.get(key) ?? 0, r.iteration + 1));
  }
  const counts = [...fxMaxIter.values()];
  return counts.length ? [Math.min(...counts), Math.max(...counts)] : [0, 0];
};
const [coldLo, coldHi] = iterRange('cold');
const [warmLo, warmHi] = iterRange('warm');
const nDesc = (lo, hi, label) => lo === hi ? `${label} n=${lo}` : `${label} n=${lo}–${hi}`;
const runnerLine = `local Linux box via \`node perf/baseline-sweep.mjs\` -> \`node perf/run.mjs --corpus manifest --engine ts\` (${nDesc(coldLo, coldHi, 'cold')}; ${nDesc(warmLo, warmHi, 'warm')} + 1 discarded warm-up; CPU 1x and 4x)`;
// #117 merged (#494); L/XL measured on real post-#117 content.
const NOTE2 = 'L/XL baseline measured on real #117 content (pad.bin replaced). `history.jsonl` updated. #10 stays open until HoP sign-off.';
const lxlMeasured = FORMATS.some((fm) => kept.some((f) => f.format === fm && ['L', 'XL'].includes(f.tier)));
const hostRows = hosts.flatMap((h, i) => Object.entries(h).map(([k, v]) => [hosts.length > 1 ? `host ${i + 1}: ${k}` : `host: ${k}`, v]));
const header = [
  ['TS engine', 'upstream ChristopherVR/ooxml @ e66aaec'],
  ['Measured at', `nitro-xml commit ${COMMIT} plus the harness changes of the PR that added this file`],
  ['Chrome', chromes.join(', ')],
  ['Runner', runnerLine],
  ['Date', DATE],
  ['Source data', `perf/baselines/${basename(input)} (${rows.length} samples, ${kept.length} fixtures x 4 cells)`],
  ...hostRows,
];
const short = coverage.filter((c) => ['S', 'M', 'L', 'XL'].includes(c.t) && c.measured < 3);

const doc = [];
const H = (n, t) => doc.push({ h: n, t });
const P = (t) => doc.push({ p: t });
const T = (head, body) => doc.push({ head, body });

H(1, 'TypeScript engine baseline (#10)');
T(['Item', 'Value'], header);
P('Times are ms unless noted; heap is MiB. Per-fixture cells are `median / p95 (n=samples)` over the iterations of one fixture, p95 being the empirical 95th percentile (not a confidence interval); per-tier cells are `median (min..max)` over the fixtures of the tier of each fixture\'s median. Cells: mode (cold/warm) x CPU throttle (1x/4x).');
H(2, 'Status');
P(`- ${NOTE2}`);
P(`- Corpus: every S/M/L/XL xlsx/docx/pptx row of \`fixtures/manifest.json\` that exists on disk under \`fixtures/\` (the demo/ copies of the same bytes are not re-measured). ${short.length ? `**Fewer than 3 measured fixtures for: ${short.map((c) => `${c.fm} ${c.t} (${c.measured})`).join(', ')}.**` : 'Every tier of every format has at least 3 measured fixtures.'}`);
P(`- Fixture ids are the manifest ids; the six ids of the first baseline map to \`gen-xlsx-cells-1k\` (xlsx-S), \`gen-xlsx-cells-100k\` (xlsx-M), \`gen-docx-pages-10\` (docx-S), \`gen-docx-pages-100\` (docx-M), \`gen-pptx-slides-10\` (pptx-S), \`gen-pptx-slides-50\` (pptx-M).`);
P(`- Two runs shared the machine at a time (\`concurrent_runs\` in the host fingerprint), on disjoint fixtures of the same cell, to fit the sweep in time; a 3-way concurrency check on this box moved the median first_paint of a small xlsx by about 7%, so absolute numbers sit slightly above a serial run. Compare like with like (same host, same \`concurrent_runs\`).`);
H(2, 'Sample counts, p95 and what is not claimed');
P(`Every number in a cell is printed with its sample count n (finite samples of that metric in that cell, warm-up excluded). **Measured medians are always shown.** A cell with fewer samples than the harness design count (cold ${DESIGN_N.cold}, warm ${DESIGN_N.warm}; \`perf/run.mjs\` defaults) is labelled \`thin sample\`; a cell with none shows \`no samples\`. No existing code defines a smaller cut-off, so none is invented here.`);
P('**p95** in this report is the empirical 95th percentile of the cell\'s samples (linear interpolation, `perf/stats.mjs`); with a thin sample it is close to the maximum. It is **not** a 95% confidence interval. This report computes **no 95% confidence interval** (the seeded bootstrap in `stats.mjs` is not used here) and makes **no speed-win or WASM-vs-TS claim**: it is a descriptive TS baseline. Rankings only include fixtures whose cell is not thin. The raw samples are in the jsonl and history; this changes no perf gate, budget or threshold.');
P('The L/XL sweep ran 3 iterations per cell (cold n=3, warm n=3 after one discarded warm-up), so every measured L/XL cell below is a thin sample.');
H(2, 'Corpus coverage');
T(['format', 'tier', 'in manifest', 'measured', 'excluded (failed or incomplete)'], coverage.map((c) => [c.fm, c.t, c.manifest, c.measured, c.bad]));
P('"in manifest" includes rows that are not redistributable and so not on disk (`upstream-noredist-*`), which cannot be measured.');
H(2, 'Headline: first_paint median per tier (ms)');
T(['format', 'tier', 'fixtures', ...cellNames], FORMATS.flatMap((fm) => TIERS.map((t) => {
  const fs = kept.filter((f) => f.format === fm && f.tier === t);
  if (!fs.length) return null;
  return [fm, t, fs.length, ...cellNames.map((c) => aggregateText(fs.map((f) => ({ vals: valsOf(f, c, 'first_paint_ms') })), median, fmt, modeOf(c)))];
}).filter(Boolean)));
H(2, 'Rankings (individual fixtures)');
P(`**5 slowest fixtures** (cold 1x first_paint median, descriptive ordering of this run only). Fixtures whose cold 1x cell is a thin sample (n < ${DESIGN_N.cold}) or has no samples are not ranked, so the n=3 L/XL fixtures are absent from these two lists; their medians are in the per-fixture tables with n:`);
T(['#', 'fixture', 'format', 'tier', 'bytes', 'first_paint_ms', 'n'], rankRows(slowest, 'first_paint_ms'));
P('**5 most memory-hungry fixtures** (cold 1x JSHeapUsedSize median):');
T(['#', 'fixture', 'format', 'tier', 'bytes', 'JSHeapUsedSize (MiB)', 'n'], rankRows(hungriest, 'JSHeapUsedSize'));

H(2, 'xlsx-M warm 1x TTI: bimodal p95 (investigated)');
P(`The first baseline (7ce81c1) had xlsx-M warm 1x \`tti_ms\` median 946 ms but p95 8228 ms. **Cause: a harness metric bug, not the engine.** \`tti_ms\` was "first_render, or the end of the last long task up to full_render". For xlsx the harness itself scrolls the sheet from first_render to full_render (about 8.4 s, 166 steps of 600 px), and each step is about 50-70 ms of main-thread work, i.e. right on the 50 ms long-task line. In about a quarter of the loads one scroll step crossed it, and TTI jumped to the end of that task, anywhere between 2 s and 9 s into the scroll. Evidence (n=30 and n=24 warm 1x loads of the 100k-cell xlsx on this box):`);
P('- Every load with a high TTI had a long task of 50-120 ms starting well after first_render (e.g. at 7947, 6355, 7350 ms) while first_paint was normal (about 900 ms); loads with a low TTI had none after first_render. Step latencies of the high-TTI loads contained the 100-160 ms outliers (see the old jsonl: runs 10, 13, 14).');
P('- Not GC: a Chromium trace (v8, devtools.timeline) of 24 loads overlapped only 1 of about 25 late long tasks with a MajorGC. Not host contention: /proc/stat steal time was 0.00-0.27% on every load. Not cache eviction or font load: warm loads are served from the same disk cache, the late task is in the scroll phase after all fetches finished (fetch_ms about 3 ms), and the page loads no fonts.');
P('- Fix: `tti_ms` now ends at `interact_start` (the moment the scripted interaction begins, right after first_paint) for all three hosts, so tasks caused by the harness\'s own scroll / page jumps / slide walk no longer count (they remain in `tbt_ms`, `long_task_*` and the step latencies). The old definition is kept as `tti_with_interaction_ms`.');
T(['xlsx-M fixture (warm 1x)', 'old TTI median / p95', 'fixed TTI median / p95', 'first_paint median / p95'], ttiRows);
P(bimodal.length
  ? `**UNEXPLAINED / DO-NOT-ARM:** after the fix, ${bimodal.join(', ')} still has a warm 1x TTI p95 more than 3x its median. Do not arm a TTI gate on those cells until it is explained.`
  : `After the fix no measured xlsx-M fixture (${ttiRows.length} in the sweep aggregates) has a warm 1x TTI p95 above 3x its median; no cell is flagged unexplained/do-not-arm, but this rests on ${ttiRows.length} fixture(s) because the other xlsx-M timeouts are excluded. Remaining spread is first_paint noise on this shared box. Note that TTI is now close to first_render by construction (the harness starts interacting right after first_paint); a quiet-window TTI is still TODO(#8).`);

H(2, 'Excluded fixtures');
P(failed.size ? 'The TS engine (or the harness page) failed these fixtures, or a cell was never run; they are not part of any number above. Every excluded cell has exactly one status from `perf/baselines/failure-provenance.json` (evidence pointer per cell; raw failure files are never edited):' : 'No fixture failed.');
if (failed.size) {
  P(`- **${STATUS_LABEL.observed_timeout}**: the harness actually hit its own timeout during a run (120 s for S/M, 300 s for L/XL).`);
  P(`- **${STATUS_LABEL.observed_error}**: the run finished with an error (engine limit, throw, password); not a timeout.`);
  P(`- **${STATUS_LABEL.operator_aborted}**: the process was killed by the operator before the cell finished; no result was saved and no timeout was observed.`);
  P(`- **${STATUS_LABEL.not_run}**: the cell was never started. This includes cells skipped because a timeout was *predicted*; a predicted timeout is not a measured failure.`);
  P(`- **${STATUS_LABEL.unknown}**: provenance not determinable (also the fallback for a cell with no provenance record). Never guessed.`);
  const cellStatusCount = Object.fromEntries(STATUSES.map((st) => [st, 0]));
  for (const e of failed.values()) for (const st of e.status.values()) cellStatusCount[st]++;
  P(`Excluded cells by status (${[...failed.values()].reduce((n, e) => n + e.status.size, 0)} cells of ${failed.size} fixtures): ${STATUSES.map((st) => `${STATUS_LABEL[st]} ${cellStatusCount[st]}`).join(', ')}. ${failed.size} excluded fixtures; ${[...failed.values()].filter((e) => e.kind === 'timeout').length} have at least one observed timeout. Timed-out rows stay in the history as timeouts (\`${basename(failFile)}\` has every failed chunk row); they are never replaced by re-run numbers. In the table, \`ok (n=N)\` is a measured cell and \`-\` a cell that has neither samples nor a failure record.`);
  const cellCol = (e, c) => (e.status.has(c) ? STATUS_LABEL[e.status.get(c)] : byFixture.get(e.fixture)?.cells.has(c) ? `ok (n=${byFixture.get(e.fixture).cells.get(c).n})` : '-');
  T(['fixture', ...cellNames, 'first error line (observed cells only)'], [...failed.values()].map((e) => [e.fixture, ...cellNames.map((c) => cellCol(e, c)), e.error]));
  P('**S/M rows: were the timeouts memory pressure?** No evidence of it. The same fixtures timed out in every cell, cold and warm, with about 8 GiB free, and re-running alone on an idle machine reproduced the xlsx timeouts; the cause was the unbounded scroll loop in the xlsx host page (see the re-run section below). The docx/pptx rows are different: `real-docx-poi-Bug54849` and `real-docx-poi-drawing` timed out in the sweep and, where they were re-measured, finished, so their cause is **unexplained** (contention or flakiness is possible, not shown). `gen-pptx-placeholder` and `real-pptx-poi-bug59273` never render slide 1, `real-docx-poi-bug65649` throws in the TS engine, and `upstream-fb3af3f7b5f7` is password protected. For the S/M rows the status comes from the raw harness failure rows only (written from completed chunk results); the S/M operator transcript was not audited for kills.');
}
if (rerunRows.length || rerunFailRows.length) {
  const rf = [...rerunBy.values()].filter((f) => f.cells.has('cold 1x')).sort((a, b) => FORMATS.indexOf(a.format) - FORMATS.indexOf(b.format) || TIERS.indexOf(a.tier) - TIERS.indexOf(b.tier) || a.id.localeCompare(b.id));
  const stillFailed = new Set(rerunFailRows.map((x) => x.fixture));
  H(2, 'Re-run of the originally-excluded xlsx fixtures after the scroll fix');
  P('**Cause of the xlsx timeouts: a harness bug, not the engine and not memory pressure.** The xlsx page scrolls the sheet 600 px per step until the grid stops moving or `lastRow` is reached. Fixtures from `fixtures/manifest.json` carry no `lastRow` hint and the TS grid is far taller than any sheet, so the scroll ran for minutes and the page never set `done`; the runner gave up after 120 s. Evidence: `real-xlsx-poi-test` (8.5 KB) timed out again when run alone on an idle machine (about 8 GiB free), and finished in about 10 s once the loop was bounded to 200 steps when no hint is given (`perf/pages/xlsx.html`).');
  P('**The original sweep timeout rows are kept as-is in `' + basename(failFile) + '` and are never edited.** xlsx fixtures that the re-run measured adequately (cold 1x + cold 4x minimum) are **promoted to the main aggregates** (required by HoP 2026-11-05: ≥3 measured fixtures per xlsx S/M tier). Their re-run data is included via `' + basename(input) + '`; the scripted scroll is capped at 200 steps for these fixtures, so full_render and step metrics are not comparable with fixtures that have a `lastRow` hint.');
  const perCell = cellNames.map((c) => `${c}: ${[...rerunBy.values()].filter((f) => f.cells.has(c)).length}`).join(', ');
  P(`${rf.length} re-run fixture(s) shown below${rerunFailRows.length ? `; ${stillFailed.size} still fail in at least one re-run cell (${[...stillFailed].join(', ')})` : ''}. Fixtures measured per cell: ${perCell}.`);
  for (const fm of FORMATS) for (const t of ['S', 'M']) {
    const fs = rf.filter((f) => f.format === fm && f.tier === t);
    if (!fs.length) continue;
    H(3, `re-run ${fm} ${t}: ${fs.length} fixtures`);
    T(['fixture', 'bytes', ...cellNames, 'heap MiB (cold 1x)'], fs.map((f) => [f.id, bytesOf(f) ?? '-', ...cellNames.map((c) => mp(f, c, 'first_paint_ms')), mOnly(f, 'cold 1x', 'JSHeapUsedSize')]));
  }
}

H(2, 'L/XL');
if (lxlMeasured) {
  P('L/XL measured on real post-#117 content (pad.bin replaced by #494). Pre-#117 padded rows are superseded and not in any aggregate.');
  T(['format', 'tier', 'in manifest', 'measured', 'excluded'], coverage.filter((c) => ['L', 'XL'].includes(c.t)).map((c) => [c.fm, c.t, c.manifest, c.measured, c.bad]));
} else {
  P('L/XL not yet measured in this dataset.');
}
if (lxlMeasured) {
  H(3, 'L/XL hard limits and unmeasured cells');
  P('Status per cell is in `perf/baselines/failure-provenance.json` (one evidence pointer per cell). Cells with no row have no measurement; nothing below is an estimate.');
  P('- **docx XL (3 fixtures): 0 measured.** **Observed error** at cold 1x, cold 4x and (b, c) warm 1x: `word/document.xml exceeds the 25 MiB limit`, a hard TS-engine limit. The warm 4x cells (a, b, c) and warm 1x for `-a` were **not run**; the limit is observed only in the cells that ran.');
  P('- **docx L `gen-docx-pages-500` (a/b/c): in no aggregate.** Cold 1x has 3 measured rows each. Cold 4x: **observed timeout** (300 s). Warm 1x: **operator aborted**: the chunk ran about 6.6 h and was killed with `kill -9`; the result file is written only at the end, so nothing was saved and no timeout was observed (whether any iteration finished is not determinable). Warm 4x: **not run**. The "render avg 226 s" style notes in the raw failures file are predictions with no raw source.');
  P('- **pptx XL (3 fixtures): 0 measured.** Cold 1x: **observed timeout** (300 s) for a, b, c. Cold 4x: `-a` **operator aborted** (killed about 23 s after it started), `-b` and `-c` **not run** (the chunk was killed first). Warm 1x and warm 4x: **not run** (predicted hang). That the TS engine blocks the page in a synchronous slide loop is an operator inference from the cold 1x timeouts, not something the data shows.');
  P('- **pptx L `gen-pptx-slides-200` (a/b/c): in no aggregate.** Cold 1x measured (3 rows each). Cold 4x: **observed timeout** for all three (for `-c` from the harness FAIL line and the 301 s gap between per-fixture start samples). Warm 1x: `-a` measured, `-b` one sample then an **observed timeout** (n=1), `-c` **not run**. Warm 4x: **not run** for all three.');
  P('- **xlsx XL `gen-xlsx-xl-a`:** cold 1x and warm 1x measured (n=3); cold 4x and warm 4x **observed timeout** (300 s), so it is excluded from aggregates. xl-b and xl-c are measured (2 fixtures, below the 3-fixture target).');
  P('- **Box pressure:** the sweep ran for roughly 40 hours on a shared 8-core, 16 GB box (`perf/baselines/box-conditions.jsonl` records load and free memory at each fixture start; load average reached 241 when the pptx XL cold 4x run started). The 4x observed timeouts therefore mix real TS slowness with box pressure and are **not separable** from this data. They are recorded as timeouts, never as numbers.');
}

H(2, 'Per format and tier (aggregate over fixtures)');
for (const fm of FORMATS) for (const t of TIERS) {
  const fs = kept.filter((f) => f.format === fm && f.tier === t);
  if (!fs.length) continue;
  const { prim, rest } = tierKeys(fs);
  H(3, `${fm} ${t}: ${fs.length} fixtures`);
  T(['metric', ...cellNames.map((c) => `${c} (Chrome ${chromes.join('/')})`)], tierBody(fs, prim));
  if (rest.length) { P('Other metrics:'); T(['metric', ...cellNames], tierBody(fs, rest)); }
}
H(2, 'Every fixture');
P(`first_paint \`median / p95 (n=samples)\` per cell, then cold 1x JSHeapUsedSize median (MiB) and warm 1x tti_ms median, each with its n. A cell below the design count is labelled thin sample; a cell with none shows no samples. The full metric set per iteration is in the jsonl.`);
for (const fm of FORMATS) for (const t of TIERS) {
  const fs = kept.filter((f) => f.format === fm && f.tier === t);
  if (!fs.length) continue;
  H(3, `${fm} ${t}`);
  T(['fixture', 'bytes', ...cellNames, 'heap MiB (cold 1x)', 'tti warm 1x'], fs.map((f) => [f.id, bytesOf(f) ?? '-', ...cellNames.map((c) => mp(f, c, 'first_paint_ms')), mOnly(f, 'cold 1x', 'JSHeapUsedSize'), mOnly(f, 'warm 1x', 'tti_ms')]));
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
