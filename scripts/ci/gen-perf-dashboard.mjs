#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Generates the perf scoreboard page from perf/history.jsonl.
// Usage: node scripts/ci/gen-perf-dashboard.mjs <history.jsonl> <out.html>
// Called by scripts/ci/pages.sh. No npm dependencies; Node builtins only.
// Robust to new L/XL/xlsx rows appended by future measurement runs.

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';

const [,, histFile, outFile] = process.argv;
if (!histFile || !outFile) {
  process.stderr.write('usage: gen-perf-dashboard.mjs <history.jsonl> <out.html>\n');
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
const TIERS = ['S', 'M', 'L', 'XL'];

// Build format/tier lookup from explicit fields (new rows) or infer from fixture_id prefix (legacy seeded rows).
const fixtureFormatMap = new Map(); // fixture_id -> format
const fixtureTierMap   = new Map(); // fixture_id -> tier
for (const r of rows) {
  if (r.format && !fixtureFormatMap.has(r.fixture_id)) fixtureFormatMap.set(r.fixture_id, r.format);
  if (r.tier   && !fixtureTierMap.has(r.fixture_id))   fixtureTierMap.set(r.fixture_id, r.tier);
}

function fixtureFormat(fid) {
  if (fixtureFormatMap.has(fid)) return fixtureFormatMap.get(fid);
  return FORMATS.find(fmt => fid.startsWith(fmt + '-')) ?? null;
}
function fixtureTier(fid) {
  if (fixtureTierMap.has(fid)) return fixtureTierMap.get(fid);
  const fmt = fixtureFormat(fid);
  if (fmt) { const t = fid.slice(fmt.length + 1); if (TIERS.includes(t)) return t; }
  return null;
}

// Only include tier-aggregate rows (fixture_id = "fmt-TIER") and per-fixture rows with known format.
// Exclude legacy format-tier aggregate rows when per-fixture rows for the same format+tier exist.
const allFixtures = [...new Set(rows.map(r => r.fixture_id))];

// Detect which format+tier pairs have per-fixture rows (non-aggregate)
const tierHasPerFixture = new Set(); // "fmt|tier"
for (const fid of allFixtures) {
  const fmt = fixtureFormat(fid), tier = fixtureTier(fid);
  if (!fmt || !tier) continue;
  const isTierAggregate = fid === `${fmt}-${tier}`;
  if (!isTierAggregate) tierHasPerFixture.add(`${fmt}|${tier}`);
}

const fixturesByFmt = Object.fromEntries(FORMATS.map(fmt => [
  fmt,
  allFixtures
    .filter(fid => {
      const f = fixtureFormat(fid);
      if (f !== fmt) return false;
      const tier = fixtureTier(fid);
      if (!tier) return false;
      // If per-fixture rows exist for this format+tier, skip the legacy tier-aggregate row
      const isTierAggregate = fid === `${fmt}-${tier}`;
      if (isTierAggregate && tierHasPerFixture.has(`${fmt}|${tier}`)) return false;
      return true;
    })
    .sort((a, b) => {
      const ta = fixtureTier(a) ?? '', tb = fixtureTier(b) ?? '';
      const d = TIERS.indexOf(ta) - TIERS.indexOf(tb);
      return d !== 0 ? d : a.localeCompare(b);
    }),
]));

const commitsByDate = [...new Map(rows.map(r => [r.commit, r.date])).entries()]
  .sort((a, b) => a[1] < b[1] ? -1 : 1);
const commitCount = commitsByDate.length;
const latestRow = rows.reduce((b, r) => (!b || r.date > b.date) ? r : b, null);

function trendPoints(fid, eng, metricKey, mode = 'cold', cpu = 1) {
  return rows
    .filter(r => r.fixture_id === fid && r.engine === eng && r.mode === mode && (r.cpu_throttle ?? 1) === cpu)
    .sort((a, b) => a.date < b.date ? -1 : 1)
    .map(r => ({ c: r.commit.slice(0, 7), d: r.date.slice(0, 10), v: r.metrics?.[metricKey]?.median }))
    .filter(x => x.v != null)
    .slice(-10);
}

// --- HTML helpers ---
const h = s => String(s ?? '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const fmtMs = v => v == null ? '—' : v < 10 ? v.toFixed(2) : Math.round(v).toLocaleString();
const fmtMiB = v => v == null ? '—' : (v / 1_048_576).toFixed(1);

function ratioCell(wasmVal, tsVal) {
  if (wasmVal == null || tsVal == null) return '<span class="muted">not yet measured</span>';
  const r = wasmVal / tsVal;
  if (r < 1) return `<span class="win">${(1 / r).toFixed(2)}× faster</span>`;
  if (r > 1) return `<span class="loss">${r.toFixed(2)}× slower</span>`;
  return 'equal';
}

function sparkSvg(pts, label) {
  if (!pts.length) return '<span class="muted">—</span>';
  if (pts.length === 1) {
    return `<span class="spark-dot" title="${h(label + ' at ' + pts[0].d + ': ' + Math.round(pts[0].v) + ' ms')}">•</span>`;
  }
  const W = 60, H = 18;
  const vals = pts.map(p => p.v);
  const mn = Math.min(...vals), mx = Math.max(...vals), rng = mx - mn || 1;
  const xs = pts.map((_, i) => (i / (pts.length - 1)) * W);
  const ys = vals.map(v => H - 1 - ((v - mn) / rng) * (H - 2));
  const title = pts.map(p => `${p.d}: ${Math.round(p.v)} ms`).join('\n');
  const pts2 = xs.map((x, i) => `${x.toFixed(1)},${ys[i].toFixed(1)}`).join(' ');
  const lx = xs.at(-1).toFixed(1), ly = ys.at(-1).toFixed(1);
  return `<svg width="${W}" height="${H}" title="${h(title)}" aria-label="${h(label)}" role="img" style="vertical-align:middle;overflow:visible"><polyline fill="none" stroke="var(--navy)" stroke-width="1.5" points="${pts2}"/><circle cx="${lx}" cy="${ly}" r="2.5" fill="var(--navy)"/></svg>`;
}

function fmtSection(fmt) {
  const fixtures = fixturesByFmt[fmt] ?? [];
  if (!fixtures.length) return '';

  // Determine which columns to show (hide WASM column if no WASM data at all)
  const hasWasm = fixtures.some(fid => get(fid, 'wasm', 'cold', 1));

  const tableRows = fixtures.map(fid => {
    const tier = fixtureTier(fid) ?? fid.slice(fmt.length + 1);
    const ts1  = get(fid, 'ts', 'cold', 1);
    const ts4  = get(fid, 'ts', 'cold', 4);
    const tsW1 = get(fid, 'ts', 'warm', 1);
    const w1   = get(fid, 'wasm', 'cold', 1);
    const wW1  = get(fid, 'wasm', 'warm', 1);

    const tsFp    = mv(ts1, 'first_paint_ms');
    const tsFp4   = mv(ts4, 'first_paint_ms');
    const tsParse = mv(ts1, 'parse_ms');
    const tsHeap  = mv(ts1, 'JSHeapUsedSize');
    const tsTti   = mv(tsW1, 'tti_ms');
    const wasmFp  = mv(w1, 'first_paint_ms');
    const wasmTti = mv(wW1, 'tti_ms');

    // p95 for confidence tooltip
    const tsFpP95   = ts1?.metrics?.first_paint_ms?.p95 ?? null;
    const tsFp4P95  = ts4?.metrics?.first_paint_ms?.p95 ?? null;
    const tsTtiP95  = tsW1?.metrics?.tti_ms?.p95 ?? null;
    const tsN       = ts1?.n ?? null;
    const p95title  = (med, p95, n) => {
      if (med == null) return '';
      const parts = [];
      if (p95 != null) parts.push(`p95: ${Math.round(p95)} ms`);
      if (n != null) parts.push(`n=${n}`);
      return parts.length ? ` title="${parts.join(' · ')}"` : '';
    };

    const spark = sparkSvg(trendPoints(fid, 'ts', 'first_paint_ms'), `${fid} TS first_paint cold 1×`);

    // Cross-host flag: if rows for this fixture span more than one host id, mark not comparable
    const hostIds = new Set(rows.filter(r => r.fixture_id === fid).map(r => r.host?.id ?? '').filter(Boolean));
    const crossHost = hostIds.size > 1
      ? `<span class="muted" title="data from ${hostIds.size} different machines — not directly comparable">⚠&nbsp;cross-host</span> ` : '';
    // Rerun indicator: scroll capped at 200 steps — full_render_ms and scroll step metrics not comparable with hinted fixtures
    const isRerun = ts1?.rerun === true || ts4?.rerun === true || tsW1?.rerun === true;
    const rerunMark = isRerun
      ? `<span class="muted" title="post-scroll-fix rerun: scroll capped at 200 steps — full_render_ms and scroll step metrics not comparable with hinted fixtures; first_paint is comparable">⟳</span> ` : '';

    const wasmCols = hasWasm
      ? `<td class="r">${fmtMs(wasmFp)}</td><td class="r">${fmtMs(wasmTti)}</td><td>${ratioCell(wasmFp, tsFp)}</td>`
      : '';

    return `<tr>
  <td>${h(fid)}</td><td class="r">${h(tier)}</td>
  <td class="r"${p95title(tsFp, tsFpP95, tsN)}>${fmtMs(tsFp)}</td><td class="r"${p95title(tsFp4, tsFp4P95, null)}>${fmtMs(tsFp4)}</td>
  <td class="r">${fmtMs(tsParse)}</td><td class="r">${fmtMiB(tsHeap)}</td>
  <td class="r"${p95title(tsTti, tsTtiP95, null)}>${fmtMs(tsTti)}</td>
  ${wasmCols}
  <td>${rerunMark}${crossHost}${spark}</td>
</tr>`;
  }).join('\n');

  const wasmHeads = hasWasm
    ? '<th title="WASM cold 1× CPU">WASM first_paint (ms)</th><th title="WASM warm 1× CPU">WASM TTI warm (ms)</th><th>WASM vs TS</th>'
    : '';

  return `
<h2>${h(fmt)}</h2>
${!hasWasm ? '<p class="note note-sm">WASM engine not yet measured — WASM columns will appear once data exists.</p>' : ''}
<div class="scroll"><table>
<thead><tr>
  <th>Fixture</th><th>Tier</th>
  <th title="cold 1× CPU — time from navigation start to first painted frame (ms)">TS first_paint cold 1×</th>
  <th title="cold 4× CPU throttle — same metric at 4× slowdown">TS first_paint cold 4×</th>
  <th title="cold 1× CPU — engine parse time (ms)">TS parse cold 1×</th>
  <th title="cold 1× CPU — JS heap after parse (MiB)">TS heap (MiB)</th>
  <th title="warm 1× CPU — time to interactive after first paint (ms)">TS TTI warm 1×</th>
  ${wasmHeads}
  <th title="last-N cold 1× first_paint trend">Trend (TS first_paint)</th>
</tr></thead>
<tbody>
${tableRows}
</tbody>
</table></div>`;
}

const sections = FORMATS.map(fmtSection).join('\n');

const chrome = latestRow?.chrome_version ?? latestRow?.host?.chrome_version ?? 'unknown';
const hostDesc = latestRow?.host
  ? `${h(latestRow.host.cpu ?? 'unknown')}, ${latestRow.host.cores ?? '?'} cores, ${h(latestRow.host.os ?? '')}`
  : 'unknown';
const generatedAt = new Date().toISOString();

const page = `<!doctype html>
<!-- SPDX-License-Identifier: MIT; generated ${generatedAt} by scripts/ci/gen-perf-dashboard.mjs -->
<html lang="en"><head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Nitro XML · Perf Scoreboard</title>
<style>
:root{--navy:#0b1f3a;--ink:#000;--paper:#fff;--muted:#555;--rule:#c9d3e3;--tint:#e8eef7}
*{box-sizing:border-box}
body{margin:0;font:15px/1.5 system-ui,sans-serif;color:var(--ink);background:var(--paper)}
header{background:var(--navy);color:#fff;padding:1rem 1.25rem}
header h1{margin:0;font-size:1.3rem}
header nav{margin-top:.35rem;font-size:.88rem}
header nav a{color:#fff;margin-right:1rem}
main{max-width:1120px;margin:0 auto;padding:.75rem 1rem 3rem}
h2{color:var(--navy);border-bottom:2px solid var(--navy);padding-bottom:.2rem;margin-top:2rem}
.scroll{overflow-x:auto;border:1px solid var(--navy);margin:.6rem 0}
table{border-collapse:collapse;width:100%;font-size:.82rem}
th{background:var(--navy);color:#fff;padding:5px 8px;text-align:left;white-space:nowrap}
td{border-top:1px solid var(--rule);padding:4px 8px;vertical-align:middle;white-space:nowrap}
.r{text-align:right;font-variant-numeric:tabular-nums}
.win{color:#1a7f37;font-weight:600}
.loss{color:#c0392b;font-weight:600}
.muted{color:var(--muted)}
.note{background:var(--tint);border-left:4px solid var(--navy);padding:8px 12px;margin:.6rem 0;font-size:.9rem}
.note-sm{padding:5px 10px;font-size:.85rem;margin:.35rem 0}
.spark-dot{color:var(--navy);font-size:1.1rem;line-height:1}
.gen{font-size:.78rem;color:var(--muted);margin-top:2rem}
@media(prefers-color-scheme:dark){
  :root{--paper:#06101f;--ink:#e8eef7;--rule:#1e3050;--tint:#0d1e35;--muted:#9db6dc}
  tr:nth-child(even) td{background:#0a1829}
  .win{color:#4caf50}
}
</style>
</head><body>
<header>
  <h1>Nitro XML &middot; Perf Scoreboard</h1>
  <nav>
    <a href="../">Plan</a>
    <a href="../scoreboard/">Scoreboard vs alternatives</a>
    <a href="baseline/">TS baseline detail</a>
    <a href="../benchmarks/">Public benchmarks</a>
    <a href="https://github.com/jgridifier/nitro-xml">Repo</a>
  </nav>
</header>
<main>
<div class="note">
  <strong>${commitCount} commit${commitCount !== 1 ? 's' : ''} tracked</strong> &middot;
  ${allFixtures.length} fixture${allFixtures.length !== 1 ? 's' : ''} &middot;
  latest commit <code>${h((latestRow?.commit ?? '—').slice(0, 7))}</code> (${h((latestRow?.date ?? '').slice(0, 10))}) &middot;
  Chrome ${h(chrome)} &middot; ${h(hostDesc)} &middot;
  Engine: <strong>ts</strong> = upstream ChristopherVR/ooxml @ e66aaec; <strong>wasm</strong> = Nitro XML Rust/WASM (not yet measured) &middot;
  Losses vs TS shown as prominently as wins. Cross-host rows ⚠ are not directly comparable. ⟳ = post-scroll-fix rerun (scroll capped 200 steps; <code>full_render_ms</code> and scroll step metrics not comparable with hinted fixtures). &middot;
  <a href="baseline/">Full TS baseline report</a>
</div>
${sections}
<p class="gen">Generated ${generatedAt} &middot; source: <code>perf/history.jsonl</code> &rarr; <code>scripts/ci/gen-perf-dashboard.mjs</code> &rarr; assembled by <code>scripts/ci/pages.sh</code></p>
</main>
</body></html>
`;

mkdirSync(dirname(outFile), { recursive: true });
writeFileSync(outFile, page);
console.log(`gen-perf-dashboard: wrote ${outFile} (${rows.length} rows, ${commitCount} commit${commitCount !== 1 ? 's' : ''}, ${allFixtures.length} fixtures)`);
