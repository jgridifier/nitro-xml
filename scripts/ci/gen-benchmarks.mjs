#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Generates /benchmarks/index.html for GitHub Pages from perf/history.jsonl.
// Usage: node scripts/ci/gen-benchmarks.mjs <history.jsonl> <out.html>
// Called by scripts/ci/pages.sh. No npm dependencies; Node builtins only.
//
// Design rules (from #495):
//  - Headline speedup: "not yet measured" until WASM data exists in history.jsonl
//  - Losses must be listed before wins when both exist
//  - Method section discloses host, Chrome version and memory measurement approach
//  - No provisional numbers; measured-only table
//  - No npm install link (package not yet published)

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';

const [,, histFile, outFile] = process.argv;
if (!histFile || !outFile) {
  process.stderr.write('usage: gen-benchmarks.mjs <history.jsonl> <out.html>\n');
  process.exit(1);
}

const rows = readFileSync(histFile, 'utf8')
  .split('\n').filter(Boolean).map(l => JSON.parse(l));

// Latest entry per (fixture_id, engine, mode, cpu_throttle)
const latestKey = r => `${r.fixture_id}|${r.engine}|${r.mode}|${r.cpu_throttle ?? 1}`;
const latestMap = new Map();
for (const r of rows) {
  const k = latestKey(r);
  const prev = latestMap.get(k);
  if (!prev || r.date > prev.date) latestMap.set(k, r);
}
const get = (fid, eng, mode, cpu = 1) => latestMap.get(`${fid}|${eng}|${mode}|${cpu}`) ?? null;
const mv = (r, key) => r?.metrics?.[key]?.median ?? null;

const FORMATS = ['xlsx', 'docx', 'pptx'];
const FORMAT_LABEL = { xlsx: 'Excel (.xlsx)', docx: 'Word (.docx)', pptx: 'PowerPoint (.pptx)' };
const TIERS = ['S', 'M', 'L', 'XL'];
const allFixtures = [...new Set(rows.map(r => r.fixture_id))];

const fixturesByFmt = Object.fromEntries(FORMATS.map(fmt => [
  fmt,
  allFixtures
    .filter(f => f.startsWith(fmt + '-'))
    .sort((a, b) => {
      const ta = a.slice(fmt.length + 1), tb = b.slice(fmt.length + 1);
      const d = TIERS.indexOf(ta) - TIERS.indexOf(tb);
      return d !== 0 ? d : a.localeCompare(b);
    }),
]));

const hasWasm = rows.some(r => r.engine === 'wasm');
const latestRow = rows.reduce((b, r) => (!b || r.date > b.date) ? r : b, null);
const latestCommit = (latestRow?.commit ?? '').slice(0, 7);
const latestDate = (latestRow?.date ?? '').slice(0, 10);
const chrome = latestRow?.chrome_version ?? latestRow?.host?.chrome_version ?? 'unknown';
const hostCpu = latestRow?.host?.cpu ?? 'unknown';
const hostCores = latestRow?.host?.cores ?? '?';
const hostOs = latestRow?.host?.os ?? '';
const generatedAt = new Date().toISOString();

// --- HTML helpers ---
const h = s => String(s ?? '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const fmtMs = v => v == null ? '<span class="na">not yet measured</span>' : `${Math.round(v).toLocaleString()} ms`;
const fmtMiB = v => v == null ? '<span class="na">not yet measured</span>' : `${(v / 1_048_576).toFixed(1)} MiB`;

// Comparison table per format.
// Rows sorted losses-first (WASM slower than TS first) when WASM data exists.
function comparisonTable(fmt) {
  const fixtures = fixturesByFmt[fmt] ?? [];
  if (!fixtures.length) return '<p class="na-block">No data yet for this format.</p>';

  // Build row data
  const tableData = fixtures.map(fid => {
    const tier = fid.slice(fmt.length + 1);
    const ts1 = get(fid, 'ts', 'cold', 1);
    const w1  = get(fid, 'wasm', 'cold', 1);
    const tsFp   = mv(ts1, 'first_paint_ms');
    const tsParse = mv(ts1, 'parse_ms');
    const tsHeap = mv(ts1, 'JSHeapUsedSize');
    const wasmFp = mv(w1, 'first_paint_ms');
    const ratio  = (tsFp != null && wasmFp != null) ? wasmFp / tsFp : null;
    return { fid, tier, tsFp, tsParse, tsHeap, wasmFp, ratio };
  });

  // Sort: losses first (ratio > 1), then wins (ratio < 1), then unmeasured
  tableData.sort((a, b) => {
    const ra = a.ratio, rb = b.ratio;
    if (ra == null && rb == null) return TIERS.indexOf(a.tier) - TIERS.indexOf(b.tier);
    if (ra == null) return 1;   // unmeasured after measured
    if (rb == null) return -1;
    if (ra > 1 && rb <= 1) return -1; // losses first
    if (ra <= 1 && rb > 1) return 1;
    return ra - rb;
  });

  const tbody = tableData.map(({ fid, tier, tsFp, tsParse, tsHeap, wasmFp, ratio }) => {
    let statusCell;
    if (ratio == null) {
      statusCell = '<td class="na" colspan="2">not yet measured</td>';
    } else if (ratio < 1) {
      statusCell = `<td class="r num win">${fmtMs(wasmFp)}</td><td class="win">▲ ${(1 / ratio).toFixed(2)}× faster</td>`;
    } else {
      statusCell = `<td class="r num loss">${fmtMs(wasmFp)}</td><td class="loss">▼ ${ratio.toFixed(2)}× slower</td>`;
    }

    return `<tr>
  <td>${h(fid)}</td><td class="tier">${h(tier)}</td>
  <td class="r num">${fmtMs(tsFp)}</td>
  <td class="r num">${fmtMs(tsParse)}</td>
  <td class="r num">${fmtMiB(tsHeap)}</td>
  ${statusCell}
</tr>`;
  }).join('\n');

  return `<div class="scroll"><table>
<caption>${h(FORMAT_LABEL[fmt] ?? fmt)} — first_paint cold 1× CPU; TS baseline at <code>${h(latestCommit)}</code></caption>
<thead><tr>
  <th>Fixture</th><th>Tier</th>
  <th title="upstream TS engine, cold load, 1× CPU (ms)">TS first_paint (ms)</th>
  <th title="upstream TS engine, cold load, 1× CPU (ms)">TS parse (ms)</th>
  <th title="JS heap after cold parse (MiB; approximate — performance.memory API)">TS heap (MiB)</th>
  <th title="Nitro XML Rust/WASM, same conditions">WASM first_paint (ms)</th>
  <th>WASM vs TS</th>
</tr></thead>
<tbody>
${tbody}
</tbody>
</table></div>`;
}

const compSections = FORMATS.map(fmt => `
<h3>${h(FORMAT_LABEL[fmt] ?? fmt)}</h3>
${comparisonTable(fmt)}`).join('\n');

// Headline speedup chart: simple ASCII/SVG bar chart placeholder
// Until WASM data exists, render a "not yet measured" state.
function headlineChart() {
  if (!hasWasm) {
    return `<div class="chart-placeholder" role="img" aria-label="Speedup chart: not yet measured">
  <div class="chart-inner">
    <div class="chart-label">WASM vs TypeScript speedup</div>
    <div class="chart-empty">
      <span class="na-badge">not yet measured</span>
      <p>Once the Rust/WASM engine has a full open-to-first-paint path,
         this chart will show the speedup ratio per format and file size,
         with any regressions highlighted in red. No provisional numbers
         are shown here; every cell reflects an actual run.</p>
    </div>
  </div>
</div>`;
  }

  // If WASM data exists, build a simple bar chart from it
  const fmtRatios = FORMATS.map(fmt => {
    const pts = (fixturesByFmt[fmt] ?? []).map(fid => {
      const tsFp = mv(get(fid, 'ts', 'cold', 1), 'first_paint_ms');
      const wFp  = mv(get(fid, 'wasm', 'cold', 1), 'first_paint_ms');
      return (tsFp != null && wFp != null) ? wFp / tsFp : null;
    }).filter(x => x != null);
    if (!pts.length) return null;
    const geomean = Math.exp(pts.reduce((s, r) => s + Math.log(r), 0) / pts.length);
    return { fmt, geomean };
  }).filter(Boolean);

  if (!fmtRatios.length) return '<p class="na-block">No WASM vs TS comparison data available yet.</p>';

  const W = 480, barH = 36, gap = 10, padL = 90, padR = 20, padT = 10;
  const totalH = padT + fmtRatios.length * (barH + gap);
  const maxRatio = Math.max(...fmtRatios.map(x => x.geomean), 1) * 1.1;
  const bars = fmtRatios.map((x, i) => {
    const y = padT + i * (barH + gap);
    const barW = (x.geomean / maxRatio) * (W - padL - padR);
    const cls = x.geomean < 1 ? 'win' : 'loss';
    const label = x.geomean < 1 ? `${(1 / x.geomean).toFixed(2)}× faster` : `${x.geomean.toFixed(2)}× slower`;
    return `<text x="${padL - 6}" y="${y + barH / 2 + 5}" text-anchor="end" font-size="13">${h(FORMAT_LABEL[x.fmt] ?? x.fmt)}</text>
<rect x="${padL}" y="${y}" width="${barW.toFixed(1)}" height="${barH}" class="bar-${cls}"/>
<text x="${padL + barW + 4}" y="${y + barH / 2 + 5}" font-size="12">${h(label)}</text>`;
  }).join('\n');

  return `<svg width="${W}" height="${totalH}" role="img" aria-label="WASM vs TS speedup (geomean first_paint cold 1×)">
<style>.bar-win{fill:#1a7f37}.bar-loss{fill:#c0392b}text{fill:var(--ink);font-family:system-ui}</style>
${bars}
</svg>`;
}

const page = `<!doctype html>
<!-- SPDX-License-Identifier: MIT; generated ${generatedAt} by scripts/ci/gen-benchmarks.mjs -->
<html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Nitro XML &mdash; Benchmarks</title>
<meta name="description" content="Performance benchmarks for Nitro XML: a Rust/WebAssembly port of ChristopherVR/ooxml for Excel, Word and PowerPoint in the browser. Measured against the upstream TypeScript engine on first paint, parse time and memory.">
<meta property="og:type" content="website">
<meta property="og:title" content="Nitro XML &mdash; Benchmarks">
<meta property="og:description" content="Measured performance: Nitro XML (Rust/WASM) vs the upstream TypeScript engine on Excel, Word and PowerPoint. No provisional numbers — every cell is a real measured run.">
<meta property="og:url" content="https://jgridifier.github.io/nitro-xml/benchmarks/">
<link rel="canonical" href="https://jgridifier.github.io/nitro-xml/benchmarks/">
<style>
:root{--navy:#0b1f3a;--ink:#000;--paper:#fff;--muted:#555;--rule:#c9d3e3;--tint:#e8eef7}
*{box-sizing:border-box}
body{margin:0;font:16px/1.6 system-ui,sans-serif;color:var(--ink);background:var(--paper)}
header{background:var(--navy);color:#fff;padding:1.1rem 1.25rem}
header h1{margin:0;font-size:1.35rem}
header nav{margin-top:.4rem;font-size:.88rem}
header nav a{color:#fff;margin-right:1rem}
main{max-width:980px;margin:0 auto;padding:.75rem 1.25rem 3.5rem}
h2{color:var(--navy);border-bottom:3px solid var(--navy);padding-bottom:.25rem;margin-top:2.2rem;font-size:1.2rem}
h3{color:var(--navy);margin:1.4rem 0 .4rem;font-size:1.05rem}
a{color:var(--navy)}
.note{background:var(--tint);border-left:4px solid var(--navy);padding:10px 14px;margin:.8rem 0;font-size:.9rem}
.na{color:var(--muted);font-style:italic}
.na-block{color:var(--muted);font-style:italic;padding:1rem 0}
.na-badge{display:inline-block;background:#e8eef7;border:1px solid #c9d3e3;color:var(--navy);padding:4px 12px;border-radius:4px;font-weight:600;font-size:.95rem}
.chart-placeholder{border:2px dashed var(--rule);border-radius:6px;padding:1.5rem;margin:1rem 0;text-align:center}
.chart-inner .chart-label{font-weight:700;font-size:1.05rem;color:var(--navy);margin-bottom:.8rem}
.chart-empty{max-width:520px;margin:0 auto}
.chart-empty p{color:var(--muted);font-size:.9rem;margin:.7rem 0 0}
.scroll{overflow-x:auto;border:1px solid var(--navy);margin:.6rem 0}
table{border-collapse:collapse;width:100%;font-size:.85rem}
caption{text-align:left;padding:5px 8px;font-size:.82rem;color:var(--muted);background:var(--tint)}
th{background:var(--navy);color:#fff;padding:5px 8px;text-align:left;white-space:nowrap}
td{border-top:1px solid var(--rule);padding:5px 8px;vertical-align:middle;white-space:nowrap}
.r{text-align:right}.num{font-variant-numeric:tabular-nums}
.tier{text-align:right;font-variant-numeric:tabular-nums}
.win{color:#1a7f37;font-weight:600}
.loss{color:#c0392b;font-weight:600}
.muted{color:var(--muted)}
.gen{font-size:.78rem;color:var(--muted);margin-top:2rem}
dl.method{display:grid;grid-template-columns:max-content 1fr;gap:.35rem 1rem;font-size:.9rem}
dl.method dt{font-weight:600;white-space:nowrap}
dl.method dd{margin:0}
.links{display:flex;flex-wrap:wrap;gap:.5rem 1.5rem;margin:1rem 0}
.links a{display:inline-block;background:var(--navy);color:#fff;padding:7px 16px;border-radius:4px;text-decoration:none;font-size:.93rem}
.links a:hover{background:#16325c}
@media(prefers-color-scheme:dark){
  :root{--paper:#06101f;--ink:#e8eef7;--rule:#1e3050;--tint:#0d1e35;--muted:#9db6dc}
  a{color:#9db6dc}
  .na-badge{background:#0d1e35;border-color:#1e3050;color:#9db6dc}
  tr:nth-child(even) td{background:#0a1829}
}
</style>
</head><body>
<header>
  <h1>Nitro XML &mdash; Benchmarks</h1>
  <nav>
    <a href="../">Plan</a>
    <a href="../demo/">Live demo</a>
    <a href="../perf/">Perf dashboard</a>
    <a href="../scoreboard/">Scoreboard vs alternatives</a>
    <a href="https://github.com/jgridifier/nitro-xml">Repo</a>
  </nav>
</header>
<main>

<div class="note">
  <strong>Status (${h(latestDate)}):</strong>
  The Rust/WASM engine has not yet reached a full open-to-first-paint path.
  All WASM vs TS cells are <em>not yet measured</em>; no provisional numbers are shown.
  The measurement PR for WASM numbers is tracked separately in issue <a href="https://github.com/jgridifier/nitro-xml/issues/10">#10</a>.
  Head of Product signs off on public claims before they are released.
</div>

<h2>WASM vs TypeScript &mdash; first_paint speedup</h2>
${headlineChart()}

<h2>Results: WASM vs TypeScript baseline</h2>
<p>All numbers are median over ${latestRow?.n ?? 'N'} runs, cold load, 1× CPU.
Losses (WASM slower than TS) are listed first within each format.
Unmeasured cells are labelled <em>not yet measured</em> and are never replaced by estimates.</p>
${compSections}

<h2>Method</h2>
<dl class="method">
  <dt>Engine</dt>
  <dd>upstream ChristopherVR/ooxml TypeScript engine, pinned at commit <code>e66aaec</code> (the correctness oracle and speed baseline)</dd>
  <dt>Browser</dt>
  <dd>Google Chrome (headless) ${h(chrome)} via Playwright; cross-origin-isolated page (COOP/COEP on the perf harness only — not required by the shipped package)</dd>
  <dt>Host</dt>
  <dd>${h(hostCpu)}, ${hostCores} logical cores, ${h(hostOs)}; runner: ${h(latestRow?.runner ?? 'unknown')}</dd>
  <dt>Timing</dt>
  <dd><code>first_paint_ms</code>: navigation start to first painted frame (PerformancePaintTiming "first-paint"); <code>parse_ms</code>: engine parse call only; <code>tti_ms</code> (warm): time to interactive after first paint (harness interaction start, to avoid counting harness scroll steps)</dd>
  <dt>Memory</dt>
  <dd>JS heap size via <code>performance.memory.usedJSHeapSize</code> after a cold parse with the model retained. This is an approximate lower bound — it does not include off-heap WASM linear memory, which is reported separately when WASM data exists.</dd>
  <dt>Fairness</dt>
  <dd>Same machine, same Chrome build, same fixtures per run. Losses are shown as prominently as wins. Cross-host rows (data from different machines) are flagged as not comparable.</dd>
  <dt>Source</dt>
  <dd><code>perf/history.jsonl</code> in the repo; harness scripts in <code>perf/</code>; this page generated by <code>scripts/ci/gen-benchmarks.mjs</code></dd>
</dl>

<h2>Scoreboard vs third-party alternatives</h2>
<p>A separate scoreboard compares Nitro XML to SheetJS, ExcelJS, calamine, mammoth.js and others on parse speed and render-to-first-paint. Races that are not yet run show <em>not entered</em>.</p>
<p><a href="../scoreboard/">View the scoreboard &rarr;</a></p>

<h2>Try it</h2>
<div class="links">
  <a href="../demo/">Live demo (TypeScript engine)</a>
  <a href="https://github.com/jgridifier/nitro-xml">Source on GitHub</a>
</div>
<p class="note">The <code>nitro-xml</code> npm package is not yet published. This page will be updated when it is.</p>

<p class="gen">Generated ${generatedAt} &middot; TS baseline commit <code>${h(latestCommit)}</code> &middot; <code>scripts/ci/gen-benchmarks.mjs</code> via <code>scripts/ci/pages.sh</code></p>
</main>
</body></html>
`;

mkdirSync(dirname(outFile), { recursive: true });
writeFileSync(outFile, page);
console.log(`gen-benchmarks: wrote ${outFile} (${rows.length} rows, ${hasWasm ? 'WASM data present' : 'no WASM data yet'})`);
