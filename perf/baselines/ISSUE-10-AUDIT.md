# Issue #10 audit: failure provenance, sample counts, gate test

Statuses: `perf/baselines/failure-provenance.json`. Evidence: the committed sanitized subset `perf/baselines/issue-10-evidence/` (see its README); check with `node perf/verify-issue10-evidence.mjs`, which re-derives every status and the totals below from that subset. Original raw files (gitignored `perf/results/`, operator transcript) are never edited; their sha256 are in the evidence `MANIFEST.json` and `failure-provenance.json` (`node perf/build-failure-provenance.mjs --verify`).

## 1. Status totals (recounted from the raw files)

| source | raw file | raw rows | rows with a fixture | distinct cells | statuses |
| --- | --- | ---: | ---: | ---: | --- |
| sm-sweep | `ts-4451709.failures.jsonl` | 202 | 202 | 202 | observed_timeout 194, observed_error 8, operator_aborted 0, not_run 0, unknown 0 |
| sm-rerun | `ts-4451709.rerun-failures.jsonl` | 27 | 27 | 27 | observed_timeout 21, observed_error 6, operator_aborted 0, not_run 0, unknown 0 |
| lxl-sweep | `ts-lxl-8961e71.failures.jsonl` | 44 | 42 (+1 stderr FAIL line with no fixture field, attributed to cold 4x gen-pptx-slides-200-c; +1 blank row, unattributable) | 43 | observed_timeout 12, observed_error 8, operator_aborted 4, not_run 19, unknown 0 |
| all | | 273 | 271 | 272 | observed_timeout 227, observed_error 22, operator_aborted 4, not_run 19, unknown 0 |

The report's excluded table uses `ts-combined-8961e71.failures.jsonl` (97 rows, 88 distinct cells with a fixture, all with provenance, plus the one FAIL-line cell = 89 cells over 38 fixtures: observed_timeout 50, observed_error 16, operator_aborted 4, not_run 19, unknown 0). The 164 original S/M timeout rows of the 41 promoted xlsx fixtures stay in `ts-4451709.failures.jsonl` and in this provenance file; the combined file only omits them.
Correction: the previous summary said "233 S/M cells"; the raw count is 202 + 27 = 229.

## 2. L/XL classification (43 cells; every cell without a measured sample has exactly one status)

| fixture | cold 1x | cold 4x | warm 1x | warm 4x |
| --- | --- | --- | --- | --- |
| gen-docx-pages-500 | measured | TIMEOUT | ABORTED | NOT RUN |
| gen-docx-pages-500-b | measured | TIMEOUT | ABORTED | NOT RUN |
| gen-docx-pages-500-c | measured | TIMEOUT | ABORTED | NOT RUN |
| gen-docx-xl-a | ERROR | ERROR | NOT RUN | NOT RUN |
| gen-docx-xl-b | ERROR | ERROR | ERROR | NOT RUN |
| gen-docx-xl-c | ERROR | ERROR | ERROR | NOT RUN |
| gen-pptx-slides-200 | measured | TIMEOUT | measured | NOT RUN |
| gen-pptx-slides-200-b | measured | TIMEOUT | TIMEOUT (n=1 sample then failed) | NOT RUN |
| gen-pptx-slides-200-c | measured | TIMEOUT | NOT RUN | NOT RUN |
| gen-pptx-xl-a | TIMEOUT | ABORTED | NOT RUN | NOT RUN |
| gen-pptx-xl-b | TIMEOUT | NOT RUN | NOT RUN | NOT RUN |
| gen-pptx-xl-c | TIMEOUT | NOT RUN | NOT RUN | NOT RUN |
| gen-xlsx-xl-a | measured | TIMEOUT | measured | TIMEOUT |

Measured-only L/XL fixtures (no failing cell): gen-xlsx-cells-1m, -pad-l, -pad-l2, gen-xlsx-xl-b, gen-xlsx-xl-c, real-pptx-poi-customGeo, real-pptx-python-pptx-cht-chart-type, upstream-61fc791e15c0.

### Evidence pointer per cell (file:line or file#pointer)

| fixture | cell | status | evidence |
| --- | --- | --- | --- |
| gen-docx-pages-500 | cold 4x | observed_timeout | extracts/cold-4x-000.json#failures[fixture=gen-docx-pages-500]; raw/ts-lxl-8961e71.failures.jsonl:7 |
| gen-docx-pages-500 | warm 1x | operator_aborted | raw/box-conditions.jsonl:56; raw/sweep.log:14; operator-events.jsonl (transcript L2307); operator-events.jsonl (transcript L2295) |
| gen-docx-pages-500 | warm 4x | not_run | extracts/markers.json (warm-4x-000.jsonl); operator-events.jsonl (transcript L2295) |
| gen-docx-pages-500-b | cold 4x | observed_timeout | extracts/cold-4x-000.json#failures[fixture=gen-docx-pages-500-b]; raw/ts-lxl-8961e71.failures.jsonl:8 |
| gen-docx-pages-500-b | warm 1x | operator_aborted | raw/box-conditions.jsonl:58; raw/sweep.log:14; operator-events.jsonl (transcript L2307); operator-events.jsonl (transcript L2295) |
| gen-docx-pages-500-b | warm 4x | not_run | extracts/markers.json (warm-4x-000.jsonl); operator-events.jsonl (transcript L2295) |
| gen-docx-pages-500-c | cold 4x | observed_timeout | extracts/cold-4x-000.json#failures[fixture=gen-docx-pages-500-c]; raw/ts-lxl-8961e71.failures.jsonl:9 |
| gen-docx-pages-500-c | warm 1x | operator_aborted | raw/box-conditions.jsonl:59; raw/sweep.log:14; operator-events.jsonl (transcript L2307); operator-events.jsonl (transcript L2295) |
| gen-docx-pages-500-c | warm 4x | not_run | extracts/markers.json (warm-4x-000.jsonl); operator-events.jsonl (transcript L2295) |
| gen-docx-xl-a | cold 1x | observed_error | extracts/cold-1x-000.json#failures[fixture=gen-docx-xl-a]; raw/ts-lxl-8961e71.failures.jsonl:1 |
| gen-docx-xl-a | cold 4x | observed_error | extracts/cold-4x-000.json#failures[fixture=gen-docx-xl-a]; raw/ts-lxl-8961e71.failures.jsonl:10 |
| gen-docx-xl-a | warm 1x | not_run | raw/sweep.log:14; operator-events.jsonl (transcript L2307); operator-events.jsonl (transcript L2295) |
| gen-docx-xl-a | warm 4x | not_run | extracts/markers.json (warm-4x-000.jsonl); operator-events.jsonl (transcript L2295) |
| gen-docx-xl-b | cold 1x | observed_error | extracts/cold-1x-001.json#failures[fixture=gen-docx-xl-b]; raw/ts-lxl-8961e71.failures.jsonl:2 |
| gen-docx-xl-b | cold 4x | observed_error | extracts/cold-4x-001.json#failures[fixture=gen-docx-xl-b]; raw/ts-lxl-8961e71.failures.jsonl:11 |
| gen-docx-xl-b | warm 1x | observed_error | extracts/warm-1x-001.json#failures[fixture=gen-docx-xl-b]; raw/ts-lxl-8961e71.failures.jsonl:41 |
| gen-docx-xl-b | warm 4x | not_run | extracts/markers.json (warm-4x-001.jsonl); operator-events.jsonl (transcript L2295) |
| gen-docx-xl-c | cold 1x | observed_error | extracts/cold-1x-001.json#failures[fixture=gen-docx-xl-c]; raw/ts-lxl-8961e71.failures.jsonl:3 |
| gen-docx-xl-c | cold 4x | observed_error | extracts/cold-4x-001.json#failures[fixture=gen-docx-xl-c]; raw/ts-lxl-8961e71.failures.jsonl:12 |
| gen-docx-xl-c | warm 1x | observed_error | extracts/warm-1x-001.json#failures[fixture=gen-docx-xl-c]; raw/ts-lxl-8961e71.failures.jsonl:42 |
| gen-docx-xl-c | warm 4x | not_run | extracts/markers.json (warm-4x-001.jsonl); operator-events.jsonl (transcript L2295) |
| gen-pptx-slides-200 | cold 4x | observed_timeout | extracts/cold-4x-001.json#failures[fixture=gen-pptx-slides-200]; raw/ts-lxl-8961e71.failures.jsonl:13 |
| gen-pptx-slides-200 | warm 4x | not_run | extracts/markers.json (warm-4x-001.jsonl); operator-events.jsonl (transcript L2295) |
| gen-pptx-slides-200-b | cold 4x | observed_timeout | extracts/cold-4x-001.json#failures[fixture=gen-pptx-slides-200-b]; raw/ts-lxl-8961e71.failures.jsonl:14 |
| gen-pptx-slides-200-b | warm 1x | observed_timeout | extracts/warm-1x-001.json#failures[fixture=gen-pptx-slides-200-b]; raw/ts-lxl-8961e71.failures.jsonl:43 |
| gen-pptx-slides-200-b | warm 4x | not_run | extracts/markers.json (warm-4x-001.jsonl); operator-events.jsonl (transcript L2295) |
| gen-pptx-slides-200-c | cold 4x | observed_timeout | raw/ts-lxl-8961e71.failures.jsonl:15; raw/box-conditions.jsonl:41; raw/box-conditions.jsonl:43; raw/sweep.log:10 |
| gen-pptx-slides-200-c | warm 1x | not_run | extracts/markers.json (warm-1x-002.jsonl); operator-events.jsonl (transcript L2089) |
| gen-pptx-slides-200-c | warm 4x | not_run | extracts/markers.json (warm-4x-002.jsonl); operator-events.jsonl (transcript L2089) |
| gen-pptx-xl-a | cold 1x | observed_timeout | extracts/cold-1x-002.json#failures[fixture=gen-pptx-xl-a]; raw/ts-lxl-8961e71.failures.jsonl:4 |
| gen-pptx-xl-a | cold 4x | operator_aborted | raw/box-conditions.jsonl:43; operator-events.jsonl (transcript L2104); raw/sweep.log:10 |
| gen-pptx-xl-a | warm 1x | not_run | extracts/markers.json (warm-1x-002.jsonl); operator-events.jsonl (transcript L2089) |
| gen-pptx-xl-a | warm 4x | not_run | extracts/markers.json (warm-4x-002.jsonl); operator-events.jsonl (transcript L2089) |
| gen-pptx-xl-b | cold 1x | observed_timeout | extracts/cold-1x-002.json#failures[fixture=gen-pptx-xl-b]; raw/ts-lxl-8961e71.failures.jsonl:5 |
| gen-pptx-xl-b | cold 4x | not_run | raw/sweep.log:10; operator-events.jsonl (transcript L2104) |
| gen-pptx-xl-b | warm 1x | not_run | extracts/markers.json (warm-1x-002.jsonl); operator-events.jsonl (transcript L2089) |
| gen-pptx-xl-b | warm 4x | not_run | extracts/markers.json (warm-4x-002.jsonl); operator-events.jsonl (transcript L2089) |
| gen-pptx-xl-c | cold 1x | observed_timeout | extracts/cold-1x-002.json#failures[fixture=gen-pptx-xl-c]; raw/ts-lxl-8961e71.failures.jsonl:6 |
| gen-pptx-xl-c | cold 4x | not_run | raw/sweep.log:10; operator-events.jsonl (transcript L2104) |
| gen-pptx-xl-c | warm 1x | not_run | extracts/markers.json (warm-1x-002.jsonl); operator-events.jsonl (transcript L2089) |
| gen-pptx-xl-c | warm 4x | not_run | extracts/markers.json (warm-4x-002.jsonl); operator-events.jsonl (transcript L2089) |
| gen-xlsx-xl-a | cold 4x | observed_timeout | extracts/cold-4x-004.json#failures[fixture=gen-xlsx-xl-a]; raw/ts-lxl-8961e71.failures.jsonl:27 |
| gen-xlsx-xl-a | warm 4x | observed_timeout | extracts/warm-4x-004.json#failures[fixture=gen-xlsx-xl-a]; raw/ts-lxl-8961e71.failures.jsonl:44 |

Paths are relative to `perf/baselines/issue-10-evidence/`. S/M cells (229) are in `cell-evidence.json` and point at `raw/ts-4451709.failures.jsonl:N` or `raw/ts-4451709.rerun-failures.jsonl:N`.

### Committed evidence files (subset) and the originals they come from

| file | bytes | sha256 | source | source sha256 |
| --- | ---: | --- | --- | --- |
| `cell-evidence.json` | 73097 | `023704a13a23d1637d5c04215aa2fcbd973101026edb64f0de814d6b0450f1a4` | `perf/baselines/failure-provenance.json` | `91e1bbc49adda68156a5fd55c3da20eeea98e864b3c4ce59a60595302c712c90` |
| `extracts/cold-1x-000.json` | 1730 | `4478a7973ec3149632df61aa5a37bfed542811d8656c90ee0783cfd5fbee4eb1` | `perf/results/lxl-sweep/cold-1x-000.json` | `15747ec58d089f618d3502609be43871a763d9ec6e8dbe0aed4868b3665093d2` |
| `extracts/cold-1x-001.json` | 2222 | `075e94631bea46a515533fc9c2448577c5d82daed49b314ad7a61b2eebead2d5` | `perf/results/lxl-sweep/cold-1x-001.json` | `2c06f53b7e935f02e61f6d31db97e7b034a599ebe3e516c53d86d75dda4b8951` |
| `extracts/cold-1x-002.json` | 1360 | `d60860dddf27cbd2d73688a5cc94c529d5d8d4f663ade60902c77eb984f42dd8` | `perf/results/lxl-sweep/cold-1x-002.json` | `39a226b9c1e203512a0f19e90086d4f3e5e7126bc9db9ced2481fc5ae71eef5a` |
| `extracts/cold-4x-000.json` | 1867 | `7ebafd3f89decdf4f1ae9eb9cffd7ef1a7c8f0b796a754d18c5ebf86067c65cc` | `perf/results/lxl-sweep/cold-4x-000.json` | `99eb3a25af118bea041a2d8370f9ae892ea3193d25e868c9ae1421f31422ba8b` |
| `extracts/cold-4x-001.json` | 2313 | `d10c861b9a503666e57ce9fd19fca5f5e772e139b41d446652e79f85b6efcefb` | `perf/results/lxl-sweep/cold-4x-001.json` | `cbd9a66056652343cf146727a1a358b2c7e5cebb72919ab75bc855f7dc62826c` |
| `extracts/cold-4x-004.json` | 1282 | `3d95674bc0a923d31f75a8ffa9b806bd9b8982d578a6aa17bc3f1a265c51612c` | `perf/results/lxl-sweep/cold-4x-004.json` | `d5f60eb51bc6bea3719b97f4dfab068dbc607cb2af16074363959a911a2bf0c4` |
| `extracts/markers.json` | 1810 | `88677e86d7b7512cec57b792c3ed19ba8e387936ac7226f28e0b0cbd189e8d12` | `perf/results/lxl-sweep/sweep.log` | `3a46067a39ce6b826652c58f858c615149e82991318f9bb47aaaf5f86fc19449` |
| `extracts/warm-1x-001.gen-pptx-slides-200-b.row.jsonl` | 47270 | `293c713356b41224b6cf4bc2264dfc06a2665ea32dd32eb7dba2f540236186f8` | `perf/results/lxl-sweep/warm-1x-001.jsonl` | `ae9a255f800c7ae863fefdf8a2830998b6e7858cbe8cc6888ab059061e40cfe8` |
| `extracts/warm-1x-001.json` | 2268 | `e4fba7f766f5e5e2a3508756692092a33f8f878529724d8b4290f7977e5c9692` | `perf/results/lxl-sweep/warm-1x-001.json` | `094a3dca213860536e8625d611db9835697e80c7c8cf11ad3e89747e31e94dde` |
| `extracts/warm-4x-004.json` | 1282 | `628eb17eb72642ecba9e0475115aad4e74da5a9fc32c7ef11c31aa9e3296b3f8` | `perf/results/lxl-sweep/warm-4x-004.json` | `4cbcc0cdb7dfa574e82b5ad265bc66e598be43dfa312e7838022356728547b70` |
| `operator-events.jsonl` | 12284 | `04af8c7a1d0ea639683d0e952cb8ae9d1ff67b6ca8d158cd1cea992617965107` | `~/.claude/projects/-workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl` | `7be91da29ba57dae7747f20eb0bd21037a972d161408491d34579d12c5e44536` |
| `raw/box-conditions.jsonl` | 16718 | `bda7c666562d991e529e27c52f45faf00dab3b0ef66003d33b4a84f4ca314ea3` | `perf/baselines/box-conditions.jsonl` | `bda7c666562d991e529e27c52f45faf00dab3b0ef66003d33b4a84f4ca314ea3` |
| `raw/sweep.log` | 1521 | `3a46067a39ce6b826652c58f858c615149e82991318f9bb47aaaf5f86fc19449` | `perf/results/lxl-sweep/sweep.log` | `3a46067a39ce6b826652c58f858c615149e82991318f9bb47aaaf5f86fc19449` |
| `raw/ts-4451709.failures.jsonl` | 38858 | `4f31f715b7a1cc0b5826fc4cf2f31e0b255ac9978a17224b6307d220a37a27c4` | `perf/baselines/ts-4451709.failures.jsonl` | `4f31f715b7a1cc0b5826fc4cf2f31e0b255ac9978a17224b6307d220a37a27c4` |
| `raw/ts-4451709.rerun-failures.jsonl` | 10806 | `9797071c7a2c6e7e5ebe85e5b28e9e739294c9329c42a82cc5274e0946fe849e` | `perf/baselines/ts-4451709.rerun-failures.jsonl` | `9797071c7a2c6e7e5ebe85e5b28e9e739294c9329c42a82cc5274e0946fe849e` |
| `raw/ts-lxl-8961e71.failures.jsonl` | 9995 | `fc1cbf32325fd09d77d21d3370c082304bad23df20db9bca83d66ac3db76f56b` | `perf/baselines/ts-lxl-8961e71.failures.jsonl` | `fc1cbf32325fd09d77d21d3370c082304bad23df20db9bca83d66ac3db76f56b` |

The six 0-byte marker files are listed in `extracts/markers.json` (all share the empty-file sha256 `e3b0c442…b855`). Redactions: none were needed (0 e-mail, home path, token, bearer, JWT).

Operator events (in `operator-events.jsonl`, with transcript line numbers): L299/L707/L732 sweep launch, kill of the first launch, relaunch with `--iterations 3`; L2089 markers for chunk 2; L2101/L2104 `kill -9 4066898` (cold 4x chunk 2); L2286/L2295/L2298/L2307 warm 1x chunk 0 command line, markers, annotation rows, `kill -9 4088154`; L2121 annotation rows for the pptx XL cells.

## 3. Sample counts per cell (n)

Every number in `BASELINE.md` carries its n. A cell is labelled `thin sample` when n is below the harness design count (cold 10, warm 20; `perf/run.mjs` defaults). Nothing is suppressed; no CI and no speed-win claim is made; p95 is the empirical 95th percentile, not a confidence interval.

- Measured fixture x cell x protocol-group cells (main dataset + re-run companion, from history via the gate's key): 946 = full-n 883 + thin 63 (21 S/M, 42 L/XL). Samples of different protocol groups are never pooled (before this correction: 912 cells, 853 full, 59 thin, with four xlsx cells that added n=1 + n=10 = 11).
- L/XL: all 42 measured cells are thin (41 at n=3, gen-pptx-slides-200-b warm 1x at n=1).
- Thin S/M cells: gen-docx-pages-100 warm 4x P22e9d3 n=19; gen-pptx-equation cold 4x P22e9d3 n=8; gen-pptx-slides-10-c cold 4x P22e9d3 n=5; gen-xlsx-cells-100k-b cold 4x P22e9d3 n=1; gen-xlsx-cells-100k-c cold 4x Pb8f044 n=3; real-docx-poi-60316.dotx warm 4x P22e9d3 n=1; real-docx-poi-Bug54849 cold 1x P22e9d3 n=4; real-xlsx-poi-45540_classic_Footer cold 4x P22e9d3 n=1; real-xlsx-poi-45540_classic_Footer cold 1x Pb8f044 n=6; real-xlsx-poi-46535 cold 1x Pb8f044 n=3; real-xlsx-poi-47813 cold 4x Pb8f044 n=3; real-xlsx-poi-50096 cold 1x Pb8f044 n=7; real-xlsx-poi-57893-many-merges cold 1x Pb8f044 n=1; real-xlsx-poi-ConditionalFormattingSamples warm 4x P22e9d3 n=1; real-xlsx-poi-DataTableCities cold 1x Pb8f044 n=9; real-xlsx-poi-WithChartSheet cold 1x Pb8f044 n=9; real-xlsx-poi-WithEmbeded cold 1x P22e9d3 n=1; real-xlsx-usgov-census-NST-EST2024-POP cold 1x P22e9d3 n=1; upstream-38aa694fb4a5 warm 4x P22e9d3 n=2; upstream-88e14cba7830 warm 4x P22e9d3 n=17; upstream-a344b2f3f55d warm 4x P22e9d3 n=12.

## 4. Gate: comparability and the self-gate test

- Original failure (base 36ebc43): `gate.test.mjs` "noise: gating the same data twice passes" failed with 38 metric rows over 8 keys. Cause: 57 keys in `history.jsonl` have two entries of commit 4451709 (original sweep and scroll-fix re-run); `baselineFor` pooled both (median = mean) and each "regressed" against it.
- Revision 2 added a same-class pool with a cross-class fallback. That fallback is **removed**: a baseline must have the identical protocol key or the row is `no-comparable-baseline` (not pass, not regression, not counted as a failure, shown in the summary).
- Protocol key = keyOf (engine, fixture_id, mode, cpu_throttle) + `PROTOCOL_FIELDS` in `perf/gate.mjs`: `chrome_version`, `rerun` (explicit harness-run marker; absent = false), `fixture_sha256`, `host.kind/id/os/arch/cpu/cores/memory_gb/node/concurrent_runs`. Audit of `history.jsonl` (994 entries): chrome one value; rerun undefined 66 / false 799 / true 129; fixture_sha256 absent on the 928 entries of 4451709, set on 24 seeded 7ce81c1, null on the 42 L/XL; host.id 2 values (66 vs 928); host.arch/memory_gb/node/concurrent_runs missing on the 66 older entries, concurrent_runs=2 on the 928; commit/date/branch/n differ (not part of the key: commit is what is compared across, `date` is a placeholder 2026-11-04 for all 4451709 entries, n varies with timeouts).
- Not recorded anywhere, so not usable: harness version/commit, scroll cap per run, iterations design, playwright version, viewport. `rerun` is the only recorded proxy for the harness definition (necessary, not shown sufficient). Recommendation: record a harness id and the run protocol in future history entries.
- Counts over the real `history.jsonl`: self-gate (each entry vs history under a new commit id): 994 entries, 18792 metric rows = pass 17964, fail 0, no-baseline 828 (the 42 L/XL entries are not main), no-comparable-baseline 0 (a self copy always has an identical-protocol twin). Leave-one-out (each entry vs all other entries): 18792 rows = pass 0, fail 0, no-comparable-baseline 2158 (114 entries, the original/re-run twins), no-baseline 16634.
- Tests in `perf/test/gate.test.mjs`: matching protocol is compared (and a real regression still fails, thresholds unchanged); mismatched-only history (rerun, fixture hash, concurrent_runs, node, Chrome) gives `no-comparable-baseline` with the differing field in the note and the summary line; mixed history never pools and never falls back.
- Effect on live gating: PR entries (CI host, fixture hashes) have no identical-protocol main entry in today's box-only history, so they report `no comparable baseline` until main CI entries accumulate. The 42 L/XL entries are branch `issue-10-lxl-real` and record no hash, so there is no main-branch L/XL regression baseline yet.

## 5. Remaining blockers for closing #10

- xlsx XL has 2 measured fixtures (`gen-xlsx-xl-a` timed out at 4x); the 3-fixture target is not met. xlsx S 28 and M 7 are measured (the HoP >=3 addendum holds).
- docx L (3) and docx XL (3) and pptx XL (3): 0 fixtures in any aggregate (docx L has cold 1x rows only). pptx L: 3 of 6 in aggregates.
- All L/XL cells are thin (n=3). Re-measuring with the design iteration counts is the only way to remove that label; not done here (no new measurements).
- Warm 4x for every excluded L/XL fixture, and several warm 1x cells, are `not_run`, not measured; the pages-500 warm 1x cells are `operator_aborted`.
- Cause of the pptx XL cold 1x timeouts (suspected synchronous slide loop) and of the 4x timeouts (TS slowness vs box pressure; load average reached 241) is not determined.
- No main-branch L/XL regression baseline (L/XL history entries are not main and record no fixture hash).
- Head of Product sign-off is pending; #10 stays open.
