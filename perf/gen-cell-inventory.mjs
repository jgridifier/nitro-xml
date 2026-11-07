#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Generate perf/baselines/ISSUE-10-CELL-INVENTORY.md (#10): one row per format x tier x mode x CPU, from existing
 * verified data only: perf/history.jsonl (measured n per fixture and cell), perf/baselines/failure-provenance.json and
 * issue-10-evidence/cell-evidence.json (excluded cells), ts-combined-8961e71.jsonl (the aggregates behind BASELINE.md;
 * used only as a cross-check of n), and the evidence verifier's totals. No measurement, no new threshold.
 *   node perf/gen-cell-inventory.mjs [--check]     --check fails if the committed file differs from a fresh render
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { DESIGN_N, indexProvenance, cellKey, STATUSES } from './provenance.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const rd = (p) => readFileSync(join(root, p), 'utf8');
const jl = (p) => rd(p).split('\n').filter(Boolean).map((l) => JSON.parse(l));
const OUT = join(here, 'baselines/ISSUE-10-CELL-INVENTORY.md');

const manifest = new Map(JSON.parse(rd('fixtures/manifest.json')).fixtures.map((m) => [m.id, m]));
const info = (id) => manifest.get(id) ?? manifest.get(id.replace(/\.[a-z]+$/, ''));
const FORMATS = ['xlsx', 'docx', 'pptx'], TIERS = ['S', 'M', 'L', 'XL'], CELLS = [['cold', 1], ['cold', 4], ['warm', 1], ['warm', 4]];

// ---- measured n per fixture x cell, from history.jsonl --------------------------------------------------------
// The aggregates behind BASELINE.md hold the original-sweep entries plus the scroll-fixed re-run of the xlsx S/M fixtures.
// The re-run entries of other fixtures (docx/pptx S) are a different protocol key and a separate dataset
// (ts-combined-8961e71.rerun.jsonl); they are not pooled into n here.
const hist = jl('perf/history.jsonl').filter((e) => !e.type);
const fx = (e) => info(e.fixture_id) ?? (e.format ? { id: e.fixture_id, format: e.format, tier: e.tier } : null);
const inAggregates = (e) => { const m = fx(e); return m && (!e.rerun || (m.format === 'xlsx' && ['S', 'M'].includes(m.tier))); };
const nOf = new Map(); // fixture|mode|cpu -> n
const mainClasses = new Map(); // fixture|mode|cpu -> Set('orig'|'rerun') over ALL main entries
const mainSeen = new Set();
for (const e of hist) {
  const m = fx(e);
  if (!m || e.n_fixtures) continue; // tier-aggregate entries (ids xlsx-S ... pptx-M; 7ce81c1 seeds and 4451709 tier rows) are not per fixture
  const k = cellKey(e.fixture_id, e.mode, e.cpu_throttle);
  if (inAggregates(e)) nOf.set(k, (nOf.get(k) ?? 0) + e.n);
  if (e.branch === 'main') { (mainClasses.get(k) ?? mainClasses.set(k, new Set()).get(k)).add(e.rerun ? 'rerun' : 'orig'); mainSeen.add(k); }
}
// cross-check against the aggregates
const agg = new Map();
for (const r of jl('perf/baselines/ts-combined-8961e71.jsonl')) { const k = cellKey(r.fixture, r.mode, r.cpuThrottle); agg.set(k, (agg.get(k) ?? 0) + 1); }
const nDiff = [...new Set([...agg.keys(), ...nOf.keys()])].filter((k) => (agg.get(k) ?? 0) !== (nOf.get(k) ?? 0));
if (nDiff.length) throw new Error(`history n != aggregate n for ${nDiff.length} cells, e.g. ${nDiff[0]}`);

// ---- excluded cells: effective record per fixture x cell (lxl > rerun > sweep), same precedence as the report ----------
const prov = JSON.parse(rd('perf/baselines/failure-provenance.json'));
const ce = JSON.parse(rd('perf/baselines/issue-10-evidence/cell-evidence.json')).cells;
if (ce.length !== prov.cells.length) throw new Error('cell-evidence.json and failure-provenance.json differ in cell count');
const pi = indexProvenance(prov);
const recordsBySource = {};
for (const c of prov.cells) { const k = `${c.source} ${c.status}`; recordsBySource[k] = (recordsBySource[k] ?? 0) + 1; }
const effective = [...pi.byCell.values()];
const dupRecords = prov.cells.length - effective.length;

// ---- rows ---------------------------------------------------------------------------------------------------------
const rows = [];
for (const fm of FORMATS) for (const t of TIERS) for (const [mode, cpu] of CELLS) {
  const inRow = (id) => { const m = info(id); return m && m.format === fm && m.tier === t; };
  const ns = [...nOf].filter(([k]) => { const [id, md, c] = k.split('|'); return inRow(id) && md === mode && Number(c) === cpu; }).map(([k, n]) => ({ k, id: k.split('|')[0], n })).filter((x) => x.n > 0);
  const design = DESIGN_N[mode];
  const full = ns.filter((x) => x.n >= design).length;
  const thin = ns.length - full;
  const st = Object.fromEntries(STATUSES.map((s) => [s, 0]));
  let alsoMeasured = 0;
  for (const c of effective) {
    if (!inRow(c.fixture) || c.mode !== mode || c.cpu_throttle !== cpu) continue;
    if ((nOf.get(cellKey(c.fixture, mode, cpu)) ?? 0) > 0) alsoMeasured++;
    else st[c.status]++;
  }
  const withMain = ns.filter((x) => mainSeen.has(x.k));
  const classes = new Set(withMain.flatMap((x) => [...mainClasses.get(x.k)]));
  const baseline = !ns.length ? '-' : !withMain.length ? 'no main entries (only branch issue-10-lxl-real)' : `main ${withMain.length}/${ns.length} fixtures; protocol: ${[...classes].sort().join('+')}`;
  const unmeasured = st.observed_timeout + st.observed_error + st.operator_aborted + st.not_run + st.unknown;
  const category = full > 0 ? 'usable descriptive (full n)' : ns.length ? 'descriptive only (thin)' : (st.observed_timeout || st.operator_aborted || st.unknown) ? 'unresolved' : 'no data';
  rows.push({ fm, t, mode, cpu, measured: ns.length, range: ns.length ? `${Math.min(...ns.map((x) => x.n))}-${Math.max(...ns.map((x) => x.n))}` : '-', full, thin, st, alsoMeasured, unmeasured, baseline, category });
}

// ---- totals / cross-checks --------------------------------------------------------------------------------------------
const sum = (f) => rows.reduce((a, r) => a + f(r), 0);
const tot = { measuredCells: sum((r) => r.measured), full: sum((r) => r.full), thin: sum((r) => r.thin), err: sum((r) => r.st.observed_error), to: sum((r) => r.st.observed_timeout), ab: sum((r) => r.st.operator_aborted), nr: sum((r) => r.st.not_run), unk: sum((r) => r.st.unknown), also: sum((r) => r.alsoMeasured) };
const cat = {};
for (const r of rows) cat[r.category] = (cat[r.category] ?? 0) + 1;
if (tot.measuredCells !== agg.size) throw new Error(`measured fixture-cells ${tot.measuredCells} != aggregates ${agg.size}`);
if (tot.err + tot.to + tot.ab + tot.nr + tot.unk + tot.also !== effective.length) throw new Error('row failure counts do not add up to the effective failure cells');
const v = spawnSync('node', [join(here, 'verify-issue10-evidence.mjs')], { encoding: 'utf8' });
if (v.status !== 0) throw new Error(`verifier failed:\n${v.stderr}`);
const vt = Object.fromEntries(STATUSES.map((s) => [s, Number(new RegExp(`${s}: (\\d+)`).exec(v.stdout.split('by status:')[1])[1]) ]));
const recByStatus = Object.fromEntries(STATUSES.map((s) => [s, prov.cells.filter((c) => c.status === s).length]));
for (const s of STATUSES) if (vt[s] !== recByStatus[s]) throw new Error(`verifier ${s} ${vt[s]} != provenance ${recByStatus[s]}`);
// records superseded by a later source for the same cell, by status (these are in the 272 but not in the effective cells)
const supersededBy = {};
for (const c of prov.cells) { const e = pi.byCell.get(cellKey(c.fixture, c.mode, c.cpu_throttle)); if (e !== c) supersededBy[c.status] = (supersededBy[c.status] ?? 0) + 1; }
const effByStatus = Object.fromEntries(STATUSES.map((s) => [s, effective.filter((c) => c.status === s).length]));

// thin measured L/XL fixture-cells and S/M fixture-cells for the next-work list
const thinLXL = rows.filter((r) => ['L', 'XL'].includes(r.t)).reduce((a, r) => a + r.thin, 0);
const thinSM = rows.filter((r) => ['S', 'M'].includes(r.t)).reduce((a, r) => a + r.thin, 0);
const lxlRows = rows.filter((r) => ['L', 'XL'].includes(r.t));
const unresolvedLXL = lxlRows.reduce((a, r) => a + r.st.observed_timeout, 0);
const detErrLXL = lxlRows.reduce((a, r) => a + r.st.observed_error, 0);
const smTimeouts = rows.filter((r) => ['S', 'M'].includes(r.t)).reduce((a, r) => a + r.st.observed_timeout, 0);
const smErrors = rows.filter((r) => ['S', 'M'].includes(r.t)).reduce((a, r) => a + r.st.observed_error, 0);
const lxlAborted = lxlRows.reduce((a, r) => a + r.st.operator_aborted, 0);
const lxlNotRun = lxlRows.reduce((a, r) => a + r.st.not_run, 0);
const unmeasuredTimeouts = effective.filter((c) => c.status === 'observed_timeout' && !((nOf.get(cellKey(c.fixture, c.mode, c.cpu_throttle)) ?? 0) > 0));
const smTo = unmeasuredTimeouts.filter((c) => c.source !== 'lxl-sweep');
const smToXlsx = smTo.filter((c) => info(c.fixture).format === 'xlsx').length;
const smToOther = smTo.length - smToXlsx;
const xlsxWhere = {};
for (const c of smTo.filter((c) => info(c.fixture).format === 'xlsx')) { const k = `${c.mode} ${c.cpu_throttle}x`; xlsxWhere[k] = (xlsxWhere[k] ?? 0) + 1; }
const thinCaveat = rows.filter((r) => r.category === 'usable descriptive (full n)' && r.unmeasured > r.measured);
const lxlHistoryEntries = hist.filter((e) => e.branch !== 'main' && e.commit === '8961e71').length;

// ---- render -------------------------------------------------------------------------------------------------------------
const L = [];
const P = (...x) => L.push(...x);
P('# Issue #10 cell inventory: what exists, per format x tier x mode x CPU', '',
  'Generated by `node perf/gen-cell-inventory.mjs` from existing verified data only: `perf/history.jsonl` (measured n), `perf/baselines/failure-provenance.json` and `perf/baselines/issue-10-evidence/cell-evidence.json` (excluded cells), `perf/baselines/ts-combined-8961e71.jsonl` (the aggregates behind `BASELINE.md`, cross-checked cell by cell against history), and the totals of `node perf/verify-issue10-evidence.mjs`. No measurement, no new threshold.', '',
  '**Usable descriptive is not an eligible speed claim, and this document approves no speed claim.** Every number in `BASELINE.md` is a descriptive TS-engine baseline from one box under the recorded protocol. Nothing here approves a WASM-vs-TS win, a regression gate, a budget or a sign-off; Head of Product approval is not given or implied.', '',
  '## Definitions', '',
  `- **Cell** = fixture x mode x CPU throttle. **n** = samples of a fixture in a cell (history \`n\`, summed over the entries that are in the aggregates; 4 xlsx S/M cells pool an original and a re-run entry, so n can exceed the design count).`,
  `- **thin** = n below the harness default iterations (cold ${DESIGN_N.cold}, warm ${DESIGN_N.warm}; \`perf/run.mjs\`), descriptive-only. **full-n** = n at or above it. No other sample threshold is used or invented.`,
  '- **Excluded cell columns** count fixture-cells with **no samples** by their status in `failure-provenance.json` (a cell with a failure record that also has samples, e.g. a timeout later measured by the re-run, is counted in "also measured", not as a status). observed_error is deterministic (engine limit or throw, e.g. the 25 MiB docx limit): a rerun would not change it. observed_timeout is the harness hitting its timeout; **why** (TS slowness vs box pressure vs harness) is unresolved. operator_aborted = killed before finishing, nothing saved. not_run = never started.',
  '- **Baseline** = `main M/N fixtures; protocol: ...`: how many measured fixtures have entries on branch main, and which protocol classes (`orig` = original sweep, `rerun` = the scroll-fixed re-run) those entries are. The gate needs an *identical* protocol key (`PROTOCOL_FIELDS` in `perf/gate.mjs`); existing entries record no fixture hash, so a new entry that records one will report `no comparable baseline` until like-for-like main entries exist. `orig+rerun` means two separate protocol keys, never pooled by the gate.',
  '- **Category** (structural, one per row): `usable descriptive (full n)` = at least one fixture is full-n; `descriptive only (thin)` = measured fixtures exist but all are thin; `unresolved` = nothing measured and at least one observed_timeout / operator_aborted / unknown cell; `no data` = nothing measured and only deterministic errors or never-run cells (or nothing recorded).', '',
  '## Inventory', '',
  '| format | tier | mode | cpu | measured fixtures | fixture n (min-max) | full-n | thin | observed_error | observed_timeout | operator_aborted | not_run | also measured (failure record + samples) | baseline (main, protocol) | category |',
  '| --- | --- | --- | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |');
for (const r of rows) P(`| ${r.fm} | ${r.t} | ${r.mode} | ${r.cpu}x | ${r.measured} | ${r.range} | ${r.full} | ${r.thin} | ${r.st.observed_error} | ${r.st.observed_timeout} | ${r.st.operator_aborted} | ${r.st.not_run} | ${r.alsoMeasured} | ${r.baseline} | ${r.category} |`);
P('', '## Totals and cross-checks', '',
  `- Measured fixture-cells: **${tot.measuredCells}** = full-n **${tot.full}** + thin **${tot.thin}**. Equal to the ${agg.size} fixture-cells of \`ts-combined-8961e71.jsonl\`, and n equals the aggregate sample count in every one of them (checked cell by cell).`,
  `- Excluded fixture-cells with no samples: observed_error **${tot.err}**, observed_timeout **${tot.to}**, operator_aborted **${tot.ab}**, not_run **${tot.nr}**, unknown **${tot.unk}**; plus **${tot.also}** failure-record cells that also have samples. Together ${tot.err + tot.to + tot.ab + tot.nr + tot.unk + tot.also} = the ${effective.length} distinct failure cells.`,
  `- Verifier totals (\`node perf/verify-issue10-evidence.mjs\`, one count per provenance **record**): observed_timeout ${vt.observed_timeout}, observed_error ${vt.observed_error}, operator_aborted ${vt.operator_aborted}, not_run ${vt.not_run}, unknown ${vt.unknown}; ${prov.cells.length} records. They equal \`failure-provenance.json\` record counts.`,
  `- Reconciliation records -> inventory: ${prov.cells.length} records - ${dupRecords} records superseded by a later source for the same cell (${Object.entries(supersededBy).map(([s, n]) => `${s} ${n}`).join(', ')}) = ${effective.length} distinct cells (${STATUSES.map((s) => `${s} ${effByStatus[s]}`).join(', ')}); of those ${tot.also} also have samples, which leaves the ${tot.err + tot.to + tot.ab + tot.nr + tot.unk} status counts above.`,
  `- Rows by category (${rows.length} rows): ${Object.entries(cat).sort().map(([k, n]) => `${k} ${n}`).join(', ')}.`,
  '- Not counted: manifest fixtures never attempted or not on disk (e.g. `upstream-noredist-*`), and the 48 tier-aggregate history entries (ids `xlsx-S` ... `pptx-M`: 24 seeded 7ce81c1 and 24 of 4451709, not per fixture).', '',
  '## Evidence', '',
  '- Excluded-cell evidence: `perf/baselines/issue-10-evidence/cell-evidence.json` (per cell), `.../raw/*` (verbatim failure rows, `sweep.log`, `box-conditions.jsonl`), `.../extracts/*` (chunk failures, markers), `.../operator-events.jsonl` (launches, kills, markers); statuses re-derived by `node perf/verify-issue10-evidence.mjs`. Overview: `perf/baselines/ISSUE-10-AUDIT.md`; evidence manifest and limits: `perf/baselines/issue-10-evidence/MANIFEST.json`, `.../README.md`.',
  '- Measured cells: `perf/history.jsonl` and `perf/baselines/ts-combined-8961e71.jsonl` (rendered in `perf/BASELINE.md`).', '',
  '## Coverage caveats for the "usable descriptive" rows', '',
  `The category says some fixture in the row is full-n; it says nothing about how many fixtures of the row have no samples. Rows where more fixture-cells have no samples than have samples: ${thinCaveat.length ? thinCaveat.map((r) => `${r.fm} ${r.t} ${r.mode} ${r.cpu}x (${r.measured} measured, ${r.unmeasured} without samples)`).join('; ') : 'none'}. These rows describe only the few measured fixtures.`, '',
  '## Historical harness identity limitation', '',
  'History entries record no harness version or commit, no scroll cap or hint mode, no iteration design, no Playwright version and no viewport (the chunk result files hold iterations, Playwright and viewport, but they were never copied into history). The explicit `rerun` flag is the only recorded proxy for the harness definition: necessary for separating the original sweep from the scroll-fixed re-run, not shown to be sufficient. L/XL entries were appended from a non-main branch with no fixture hash. Do not infer a harness version for old entries.', '',
  '## Smallest next engineering work (targeted; nothing here is a blanket rerun)', '',
  `1. **Record the protocol in history entries** (\`history-append.mjs\` / \`toHistoryEntries\`): a harness id plus iterations, scroll cap/hint mode, Playwright version, viewport, and always \`fixture_sha256\`, for all entries from now on; add them to \`PROTOCOL_FIELDS\` once present. No backfill of old entries with guessed values.`,
  `2. **Append the existing L/XL results from main** (\`ts-lxl-8961e71.jsonl\`, ${lxlHistoryEntries} entries are currently on branch \`issue-10-lxl-real\`) with fixture hashes checked against the bytes measured at 8961e71. This moves existing data; it does not measure. It gives L/XL a main-branch baseline row in the table above (every L/XL baseline cell is now "no main entries").`,
  `3. **Persist per-fixture results as they finish** in \`run.mjs\`/\`baseline-sweep.mjs\` (the result file is only written at the end of a chunk, so a kill discards iterations that had finished; for the ${lxlAborted} operator_aborted L/XL cells whether any had finished is not determinable).`,
  `4. **Attribute the observed timeouts, only the cells that matter**: ${unresolvedLXL} L/XL observed_timeout fixture-cells without samples (docx L cold 4x x3; pptx L cold 4x x3; pptx XL cold 1x x3; xlsx XL -a cold 4x and warm 4x), plus the partial pptx L \`gen-pptx-slides-200-b\` warm 1x cell (n=1 then timeout), have unresolved attribution. One quiet-box run with \`box-conditions\` sampling on just those fixture-cells at the CPU throttle that timed out would separate TS slowness from box pressure. Of the ${smTo.length} S/M observed_timeout fixture-cells without samples, ${smToXlsx} are xlsx (the unbounded scroll loop of the original sweep harness, per \`BASELINE.md\`; the re-run left them without samples: ${Object.entries(xlsxWhere).map(([k, n]) => `${k} ${n}`).join(', ')}) and ${smToOther} are docx/pptx whose cause this document does not attribute; none is re-run here.`,
  `5. **Do not re-run deterministic errors.** ${detErrLXL} L/XL observed_error cells (docx XL: 25 MiB limit) and ${smErrors} S/M observed_error cells (password-protected file, TS tableRow throw) are engine behaviour and are not re-run; the docx XL warm cells that were never started stay not_run (${lxlNotRun} L/XL not_run cells in total) and are not measured.`,
  `6. **Optional, only if full-n descriptive L/XL numbers are wanted:** top up the ${thinLXL} thin L/XL fixture-cells that already have samples (xlsx L/XL, pptx L, docx L cold 1x) to the design count (cold ${DESIGN_N.cold}, warm ${DESIGN_N.warm}); not approved or scheduled by this document. The ${thinSM} thin S/M fixture-cells come from mid-run timeouts and are left as they are.`, '');
const md = L.join('\n');
if (process.argv.includes('--check')) {
  if (readFileSync(OUT, 'utf8') !== md) { console.error('ISSUE-10-CELL-INVENTORY.md is stale; run node perf/gen-cell-inventory.mjs'); process.exit(1); }
  console.log('inventory up to date');
} else {
  writeFileSync(OUT, md);
  console.log(`wrote ${OUT}`);
  console.log('totals', JSON.stringify(tot), 'categories', JSON.stringify(cat), 'verifier', JSON.stringify(vt), 'effective', JSON.stringify(effByStatus), 'superseded', JSON.stringify(supersededBy));
}
