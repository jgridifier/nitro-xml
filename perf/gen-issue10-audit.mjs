import { readFileSync, writeFileSync } from 'node:fs';
import { groupId } from './protocol-rows.mjs';
import { DESIGN_N } from './provenance.mjs';
process.chdir(new URL('..', import.meta.url).pathname);
const man0 = new Map(JSON.parse(readFileSync('fixtures/manifest.json', 'utf8')).fixtures.map((m) => [m.id, m]));
const info0 = (id) => man0.get(id) ?? man0.get(id.replace(/\.[a-z]+$/, ''));
const hist0 = readFileSync('perf/history.jsonl', 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)).filter((e) => !e.type && !e.n_fixtures && info0(e.fixture_id));
const gcell = new Map();
for (const e of hist0) { const k = `${e.fixture_id}|${e.mode}|${e.cpu_throttle}|${groupId(e)}`; gcell.set(k, (gcell.get(k) ?? 0) + e.n); }
const gc = [...gcell].map(([k, n]) => { const [id, mode, cpu, g] = k.split('|'); return { id, mode, cpu, g, n, tier: info0(id).tier }; });
const thinAll = gc.filter((c) => c.n < DESIGN_N[c.mode]);
const thinLXLc = thinAll.filter((c) => ['L', 'XL'].includes(c.tier));
const thinSMc = thinAll.filter((c) => ['S', 'M'].includes(c.tier)).sort((a, b) => a.id.localeCompare(b.id) || a.mode.localeCompare(b.mode));
const j = JSON.parse(readFileSync('perf/baselines/failure-provenance.json', 'utf8'));
const ce = JSON.parse(readFileSync('perf/baselines/issue-10-evidence/cell-evidence.json', 'utf8')).cells;
const man = JSON.parse(readFileSync('perf/baselines/issue-10-evidence/MANIFEST.json', 'utf8'));
const L = [];
const P = (...x) => L.push(...x);
const cellsL = j.cells.filter((c) => c.source === 'lxl-sweep');
const ST = { observed_timeout: 'observed_timeout', observed_error: 'observed_error', operator_aborted: 'operator_aborted', not_run: 'not_run', unknown: 'unknown' };
const tot = (cs) => Object.keys(ST).map((s) => `${s} ${cs.filter((c) => c.status === s).length}`).join(', ');
const short = { observed_timeout: 'TIMEOUT', observed_error: 'ERROR', operator_aborted: 'ABORTED', not_run: 'NOT RUN', unknown: 'UNKNOWN' };
P('# Issue #10 audit: failure provenance, sample counts, gate test', '',
  'Statuses: `perf/baselines/failure-provenance.json`. Evidence: the committed sanitized subset `perf/baselines/issue-10-evidence/` (see its README); check with `node perf/verify-issue10-evidence.mjs`, which re-derives every status and the totals below from that subset. Original raw files (gitignored `perf/results/`, operator transcript) are never edited; their sha256 are in the evidence `MANIFEST.json` and `failure-provenance.json` (`node perf/build-failure-provenance.mjs --verify`).', '',
  '## 1. Status totals (recounted from the raw files)', '',
  '| source | raw file | raw rows | rows with a fixture | distinct cells | statuses |', '| --- | --- | ---: | ---: | ---: | --- |',
  `| sm-sweep | \`ts-4451709.failures.jsonl\` | 202 | 202 | 202 | ${tot(j.cells.filter((c) => c.source === 'sm-sweep'))} |`,
  `| sm-rerun | \`ts-4451709.rerun-failures.jsonl\` | 27 | 27 | 27 | ${tot(j.cells.filter((c) => c.source === 'sm-rerun'))} |`,
  `| lxl-sweep | \`ts-lxl-8961e71.failures.jsonl\` | 44 | 42 (+1 stderr FAIL line with no fixture field, attributed to cold 4x gen-pptx-slides-200-c; +1 blank row, unattributable) | 43 | ${tot(cellsL)} |`,
  `| all | | 273 | 271 | 272 | ${tot(j.cells)} |`, '',
  'The report\'s excluded table uses `ts-combined-8961e71.failures.jsonl` (97 rows, 88 distinct cells with a fixture, all with provenance, plus the one FAIL-line cell = 89 cells over 38 fixtures: observed_timeout 50, observed_error 16, operator_aborted 4, not_run 19, unknown 0). The 164 original S/M timeout rows of the 41 promoted xlsx fixtures stay in `ts-4451709.failures.jsonl` and in this provenance file; the combined file only omits them.',
  'Correction: the previous summary said "233 S/M cells"; the raw count is 202 + 27 = 229.', '',
  '## 2. L/XL classification (43 cells; every cell without a measured sample has exactly one status)', '',
  '| fixture | cold 1x | cold 4x | warm 1x | warm 4x |', '| --- | --- | --- | --- | --- |');
const fx = [...new Set(cellsL.map((c) => c.fixture))];
for (const f of fx) {
  const g = (m, t) => { const c = cellsL.find((x) => x.fixture === f && x.mode === m && x.cpu_throttle === t); return c ? short[c.status] + (c.measured_n ? ` (n=${c.measured_n} sample then failed)` : '') : 'measured'; };
  P(`| ${f} | ${g('cold', 1)} | ${g('cold', 4)} | ${g('warm', 1)} | ${g('warm', 4)} |`);
}
P('', 'Measured-only L/XL fixtures (no failing cell): gen-xlsx-cells-1m, -pad-l, -pad-l2, gen-xlsx-xl-b, gen-xlsx-xl-c, real-pptx-poi-customGeo, real-pptx-python-pptx-cht-chart-type, upstream-61fc791e15c0.', '',
  '### Evidence pointer per cell (file:line or file#pointer)', '', '| fixture | cell | status | evidence |', '| --- | --- | --- | --- |');
const ev = (e) => `${e.file}${e.line ? ':' + e.line : ''}${e.pointer ? '#' + e.pointer : ''}${e.transcript_line ? ' (transcript L' + e.transcript_line + ')' : ''}${e.marker ? ' (' + e.marker + ')' : ''}`;
for (const c of ce.filter((x) => x.source === 'lxl-sweep')) P(`| ${c.fixture} | ${c.mode} ${c.cpu_throttle}x | ${c.status} | ${c.evidence.map(ev).join('; ')} |`);
P('', 'Paths are relative to `perf/baselines/issue-10-evidence/`. S/M cells (229) are in `cell-evidence.json` and point at `raw/ts-4451709.failures.jsonl:N` or `raw/ts-4451709.rerun-failures.jsonl:N`.', '',
  '### Committed evidence files (subset) and the originals they come from', '', '| file | bytes | sha256 | source | source sha256 |', '| --- | ---: | --- | --- | --- |');
for (const f of man.files) P(`| \`${f.file}\` | ${f.bytes} | \`${f.sha256}\` | \`${f.source}\` | \`${f.source_sha256}\` |`);
P('', 'The six 0-byte marker files are listed in `extracts/markers.json` (all share the empty-file sha256 `e3b0c442…b855`). Redactions: none were needed (0 e-mail, home path, token, bearer, JWT).', '',
  'Operator events (in `operator-events.jsonl`, with transcript line numbers): L299/L707/L732 sweep launch, kill of the first launch, relaunch with `--iterations 3`; L2089 markers for chunk 2; L2101/L2104 `kill -9 4066898` (cold 4x chunk 2); L2286/L2295/L2298/L2307 warm 1x chunk 0 command line, markers, annotation rows, `kill -9 4088154`; L2121 annotation rows for the pptx XL cells.', '',
  '## 3. Sample counts per cell (n)', '',
  'Every number in `BASELINE.md` carries its n. A cell is labelled `thin sample` when n is below the harness design count (cold 10, warm 20; `perf/run.mjs` defaults). Nothing is suppressed; no CI and no speed-win claim is made; p95 is the empirical 95th percentile, not a confidence interval.', '',
  `- Measured fixture x cell x protocol-group cells (main dataset + re-run companion, from history via the gate's key): ${gc.length} = full-n ${gc.length - thinAll.length} + thin ${thinAll.length} (${thinSMc.length} S/M, ${thinLXLc.length} L/XL). Samples of different protocol groups are never pooled (before this correction: 912 cells, 853 full, 59 thin, with four xlsx cells that added n=1 + n=10 = 11).`,
  `- L/XL: all ${thinLXLc.length} measured cells are thin (${thinLXLc.filter((c) => c.n === 3).length} at n=3, ${thinLXLc.filter((c) => c.n !== 3).map((c) => `${c.id} ${c.mode} ${c.cpu}x at n=${c.n}`).join(', ')}).`,
  `- Thin S/M cells: ${thinSMc.map((c) => `${c.id} ${c.mode} ${c.cpu}x ${c.g} n=${c.n}`).join('; ')}.`, '',
  '## 4. Gate: comparability and the self-gate test', '',
  '- Original failure (base 36ebc43): `gate.test.mjs` "noise: gating the same data twice passes" failed with 38 metric rows over 8 keys. Cause: 57 keys in `history.jsonl` have two entries of commit 4451709 (original sweep and scroll-fix re-run); `baselineFor` pooled both (median = mean) and each "regressed" against it.',
  '- Revision 2 added a same-class pool with a cross-class fallback. That fallback is **removed**: a baseline must have the identical protocol key or the row is `no-comparable-baseline` (not pass, not regression, not counted as a failure, shown in the summary).',
  '- Protocol key = keyOf (engine, fixture_id, mode, cpu_throttle) + `PROTOCOL_FIELDS` in `perf/gate.mjs`: `chrome_version`, `rerun` (explicit harness-run marker; absent = false), `fixture_sha256`, `host.kind/id/os/arch/cpu/cores/memory_gb/node/concurrent_runs`. Audit of `history.jsonl` (994 entries): chrome one value; rerun undefined 66 / false 799 / true 129; fixture_sha256 absent on the 928 entries of 4451709, set on 24 seeded 7ce81c1, null on the 42 L/XL; host.id: grok-bot-vm-427796959 on the 24 seeded 7ce81c1 entries, box-4467396174 on the other 970 (the 42 L/XL entries were corrected, see section 5); host.arch/memory_gb/node/concurrent_runs missing on the 24 seeded entries; concurrent_runs=2 on 928, 1 on the 42 L/XL; commit/date/branch/n differ (not part of the key: commit is what is compared across, `date` is a placeholder 2026-11-04 for all 4451709 entries, n varies with timeouts).',
  '- Not recorded anywhere, so not usable: harness version/commit, scroll cap per run, iterations design, playwright version, viewport. `rerun` is the only recorded proxy for the harness definition (necessary, not shown sufficient). Recommendation: record a harness id and the run protocol in future history entries.',
  '- Counts over the real `history.jsonl`: self-gate (each entry vs history under a new commit id): 994 entries, 18792 metric rows = pass 17964, fail 0, no-baseline 828 (the 42 L/XL entries are not main), no-comparable-baseline 0 (a self copy always has an identical-protocol twin). Leave-one-out (each entry vs all other entries): 18792 rows = pass 0, fail 0, no-comparable-baseline 2158 (114 entries, the original/re-run twins), no-baseline 16634.',
  '- Tests in `perf/test/gate.test.mjs`: matching protocol is compared (and a real regression still fails, thresholds unchanged); mismatched-only history (rerun, fixture hash, concurrent_runs, node, Chrome) gives `no-comparable-baseline` with the differing field in the note and the summary line; mixed history never pools and never falls back.',
  '- Effect on live gating: PR entries (CI host, fixture hashes) have no identical-protocol main entry in today\'s box-only history, so they report `no comparable baseline` until main CI entries accumulate. The 42 L/XL entries are branch `issue-10-lxl-real` and record no hash, so there is no main-branch L/XL regression baseline yet.', '',
  '## 5. L/XL history host correction (revision 6)', '',
  '- The 42 L/XL entries (`history.jsonl` lines 953-994, commit 8961e71, branch `issue-10-lxl-real`) had been appended on another machine and recorded that machine\'s host (`grok-bot-vm-427796959`, `linux 6.12.94+`, no arch/memory/node/concurrent_runs). The authoritative host is in the raw rows they were built from: every row of `perf/baselines/ts-lxl-8961e71.jsonl` (124 rows) has the identical `host`, and the 18 chunk result files in the gitignored `perf/results/lxl-sweep/` carry the same host (nothing was read from the current machine).',
  '- Field by field (all 42 entries): kind `box`, id `box-4467396174`, os `Debian GNU/Linux 13 (trixie) (linux 6.12.94+)`, arch `x64`, cpu `Intel(R) Xeon(R) Processor` (the rows\' `cpu_model`, same naming as the S/M entries), cores 8, memory_gb 16, chrome_version `156.0.8078.4`, node `v20.19.2`, concurrent_runs 1: **recovered**, each entry\'s `provenance.host` lists the source row lines (`ts-lxl-8961e71.jsonl:N`), the file sha256 `a0eec008...e998` and per-field status. **Unknown: none** (every field was recorded). `fixture_sha256` stays null (not a host field; the rows do not record it).',
  '- Unknown handling: a protocol-key field a source run does not record is the explicit UNKNOWN marker (`perf/unknown.mjs`); an entry with one has a unique protocol id, is never a baseline and never gets one (`no comparable baseline`), even against another unknown.',
  '- Preserved: the original recorded host is in each entry\'s `host_import_recorded`, and the 42 pre-correction lines are committed as `issue-10-evidence/history-lxl-pre-host-fix.jsonl` (sha256 in `MANIFEST.json`). The verifier puts those lines back and checks that `history.jsonl` then hashes to the recorded pre-correction file (`3e30e7a2...edefd`, 994 lines), i.e. no other line changed (lines 1-952 are byte-identical), and that every corrected host equals what its source rows record.',
  '- Writer fixed: `toHistoryEntries` (used by `history-append.mjs`) now takes the host from the source row (`sourceHost`), never from the importing machine; the importer\'s host is kept only as `host_import_recorded`. Tests: `perf/test/history-host.test.mjs`.',
  '- Effect: the L/XL report, gate and inventory group ids now agree (`P4faa41`), counts are unchanged (946 cells, 883 full, 63 thin). `date` of these entries is still the importing commit\'s date and `runner` the importer\'s `local`; neither is a protocol field and neither was changed.', '',
  '## 6. Remaining blockers for closing #10',
  '',
  '- xlsx XL has 2 measured fixtures (`gen-xlsx-xl-a` timed out at 4x); the 3-fixture target is not met. xlsx S 28 and M 7 are measured (the HoP >=3 addendum holds).',
  '- docx L (3) and docx XL (3) and pptx XL (3): 0 fixtures in any aggregate (docx L has cold 1x rows only). pptx L: 3 of 6 in aggregates.',
  '- All L/XL cells are thin (n=3). Re-measuring with the design iteration counts is the only way to remove that label; not done here (no new measurements).',
  '- Warm 4x for every excluded L/XL fixture, and several warm 1x cells, are `not_run`, not measured; the pages-500 warm 1x cells are `operator_aborted`.',
  '- Cause of the pptx XL cold 1x timeouts (suspected synchronous slide loop) and of the 4x timeouts (TS slowness vs box pressure; load average reached 241) is not determined.',
  '- No main-branch L/XL regression baseline (L/XL history entries are not main and record no fixture hash).',
  '- Head of Product sign-off is pending; #10 stays open.', '');
writeFileSync('perf/baselines/ISSUE-10-AUDIT.md', L.join('\n'));
