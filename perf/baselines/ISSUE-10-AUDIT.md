# Issue #10 audit: failure provenance, sample counts, gate test

Generated against `perf/baselines/failure-provenance.json` (rebuild: `node perf/build-failure-provenance.mjs`; hash check: `node perf/build-failure-provenance.mjs --verify`). Raw evidence is never edited.

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
| gen-docx-pages-500 | cold 4x | observed_timeout | lxl-sweep/cold-4x-000.json#failures[fixture=gen-docx-pages-500]; baselines/ts-lxl-8961e71.failures.jsonl:7 |
| gen-docx-pages-500 | warm 1x | operator_aborted | baselines/box-conditions.jsonl:56; lxl-sweep/sweep.log:14; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2307; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-docx-pages-500 | warm 4x | not_run | lxl-sweep/warm-4x-000.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-docx-pages-500-b | cold 4x | observed_timeout | lxl-sweep/cold-4x-000.json#failures[fixture=gen-docx-pages-500-b]; baselines/ts-lxl-8961e71.failures.jsonl:8 |
| gen-docx-pages-500-b | warm 1x | operator_aborted | baselines/box-conditions.jsonl:58; lxl-sweep/sweep.log:14; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2307; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-docx-pages-500-b | warm 4x | not_run | lxl-sweep/warm-4x-000.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-docx-pages-500-c | cold 4x | observed_timeout | lxl-sweep/cold-4x-000.json#failures[fixture=gen-docx-pages-500-c]; baselines/ts-lxl-8961e71.failures.jsonl:9 |
| gen-docx-pages-500-c | warm 1x | operator_aborted | baselines/box-conditions.jsonl:59; lxl-sweep/sweep.log:14; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2307; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-docx-pages-500-c | warm 4x | not_run | lxl-sweep/warm-4x-000.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-docx-xl-a | cold 1x | observed_error | lxl-sweep/cold-1x-000.json#failures[fixture=gen-docx-xl-a]; baselines/ts-lxl-8961e71.failures.jsonl:1 |
| gen-docx-xl-a | cold 4x | observed_error | lxl-sweep/cold-4x-000.json#failures[fixture=gen-docx-xl-a]; baselines/ts-lxl-8961e71.failures.jsonl:10 |
| gen-docx-xl-a | warm 1x | not_run | lxl-sweep/sweep.log:14; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2307; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-docx-xl-a | warm 4x | not_run | lxl-sweep/warm-4x-000.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-docx-xl-b | cold 1x | observed_error | lxl-sweep/cold-1x-001.json#failures[fixture=gen-docx-xl-b]; baselines/ts-lxl-8961e71.failures.jsonl:2 |
| gen-docx-xl-b | cold 4x | observed_error | lxl-sweep/cold-4x-001.json#failures[fixture=gen-docx-xl-b]; baselines/ts-lxl-8961e71.failures.jsonl:11 |
| gen-docx-xl-b | warm 1x | observed_error | lxl-sweep/warm-1x-001.json#failures[fixture=gen-docx-xl-b]; baselines/ts-lxl-8961e71.failures.jsonl:41 |
| gen-docx-xl-b | warm 4x | not_run | lxl-sweep/warm-4x-001.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-docx-xl-c | cold 1x | observed_error | lxl-sweep/cold-1x-001.json#failures[fixture=gen-docx-xl-c]; baselines/ts-lxl-8961e71.failures.jsonl:3 |
| gen-docx-xl-c | cold 4x | observed_error | lxl-sweep/cold-4x-001.json#failures[fixture=gen-docx-xl-c]; baselines/ts-lxl-8961e71.failures.jsonl:12 |
| gen-docx-xl-c | warm 1x | observed_error | lxl-sweep/warm-1x-001.json#failures[fixture=gen-docx-xl-c]; baselines/ts-lxl-8961e71.failures.jsonl:42 |
| gen-docx-xl-c | warm 4x | not_run | lxl-sweep/warm-4x-001.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-pptx-slides-200 | cold 4x | observed_timeout | lxl-sweep/cold-4x-001.json#failures[fixture=gen-pptx-slides-200]; baselines/ts-lxl-8961e71.failures.jsonl:13 |
| gen-pptx-slides-200 | warm 4x | not_run | lxl-sweep/warm-4x-001.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-pptx-slides-200-b | cold 4x | observed_timeout | lxl-sweep/cold-4x-001.json#failures[fixture=gen-pptx-slides-200-b]; baselines/ts-lxl-8961e71.failures.jsonl:14 |
| gen-pptx-slides-200-b | warm 1x | observed_timeout | lxl-sweep/warm-1x-001.json#failures[fixture=gen-pptx-slides-200-b]; baselines/ts-lxl-8961e71.failures.jsonl:43 |
| gen-pptx-slides-200-b | warm 4x | not_run | lxl-sweep/warm-4x-001.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2295 |
| gen-pptx-slides-200-c | cold 4x | observed_timeout | baselines/ts-lxl-8961e71.failures.jsonl:15; baselines/box-conditions.jsonl:41; baselines/box-conditions.jsonl:43; lxl-sweep/sweep.log:10 |
| gen-pptx-slides-200-c | warm 1x | not_run | lxl-sweep/warm-1x-002.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2089 |
| gen-pptx-slides-200-c | warm 4x | not_run | lxl-sweep/warm-4x-002.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2089 |
| gen-pptx-xl-a | cold 1x | observed_timeout | lxl-sweep/cold-1x-002.json#failures[fixture=gen-pptx-xl-a]; baselines/ts-lxl-8961e71.failures.jsonl:4 |
| gen-pptx-xl-a | cold 4x | operator_aborted | baselines/box-conditions.jsonl:43; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2104; lxl-sweep/sweep.log:10 |
| gen-pptx-xl-a | warm 1x | not_run | lxl-sweep/warm-1x-002.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2089 |
| gen-pptx-xl-a | warm 4x | not_run | lxl-sweep/warm-4x-002.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2089 |
| gen-pptx-xl-b | cold 1x | observed_timeout | lxl-sweep/cold-1x-002.json#failures[fixture=gen-pptx-xl-b]; baselines/ts-lxl-8961e71.failures.jsonl:5 |
| gen-pptx-xl-b | cold 4x | not_run | lxl-sweep/sweep.log:10; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2104 |
| gen-pptx-xl-b | warm 1x | not_run | lxl-sweep/warm-1x-002.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2089 |
| gen-pptx-xl-b | warm 4x | not_run | lxl-sweep/warm-4x-002.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2089 |
| gen-pptx-xl-c | cold 1x | observed_timeout | lxl-sweep/cold-1x-002.json#failures[fixture=gen-pptx-xl-c]; baselines/ts-lxl-8961e71.failures.jsonl:6 |
| gen-pptx-xl-c | cold 4x | not_run | lxl-sweep/sweep.log:10; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2104 |
| gen-pptx-xl-c | warm 1x | not_run | lxl-sweep/warm-1x-002.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2089 |
| gen-pptx-xl-c | warm 4x | not_run | lxl-sweep/warm-4x-002.jsonl; -workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl:2089 |
| gen-xlsx-xl-a | cold 4x | observed_timeout | lxl-sweep/cold-4x-004.json#failures[fixture=gen-xlsx-xl-a]; baselines/ts-lxl-8961e71.failures.jsonl:27 |
| gen-xlsx-xl-a | warm 4x | observed_timeout | lxl-sweep/warm-4x-004.json#failures[fixture=gen-xlsx-xl-a]; baselines/ts-lxl-8961e71.failures.jsonl:44 |

S/M cells: pointer is `ts-4451709.failures.jsonl:N` / `ts-4451709.rerun-failures.jsonl:N` in `failure-provenance.json` (`cells[].evidence`).

### Evidence files and sha256 (all verified equal to the current files)

| file | tracked in git | sha256 |
| --- | --- | --- |
| `perf/baselines/box-conditions.jsonl` | yes | `bda7c666562d991e529e27c52f45faf00dab3b0ef66003d33b4a84f4ca314ea3` |
| `perf/baselines/ts-4451709.failures.jsonl` | yes | `4f31f715b7a1cc0b5826fc4cf2f31e0b255ac9978a17224b6307d220a37a27c4` |
| `perf/baselines/ts-4451709.rerun-failures.jsonl` | yes | `9797071c7a2c6e7e5ebe85e5b28e9e739294c9329c42a82cc5274e0946fe849e` |
| `perf/baselines/ts-lxl-8961e71.failures.jsonl` | yes | `fc1cbf32325fd09d77d21d3370c082304bad23df20db9bca83d66ac3db76f56b` |
| `perf/results/lxl-sweep/cold-1x-000.json` | no (gitignored / outside repo) | `15747ec58d089f618d3502609be43871a763d9ec6e8dbe0aed4868b3665093d2` |
| `perf/results/lxl-sweep/cold-1x-001.json` | no (gitignored / outside repo) | `2c06f53b7e935f02e61f6d31db97e7b034a599ebe3e516c53d86d75dda4b8951` |
| `perf/results/lxl-sweep/cold-1x-002.json` | no (gitignored / outside repo) | `39a226b9c1e203512a0f19e90086d4f3e5e7126bc9db9ced2481fc5ae71eef5a` |
| `perf/results/lxl-sweep/cold-1x-003.json` | no (gitignored / outside repo) | `a654815f8551f00f7f385a024208e2174f648415818e923fae4a48ebc3a81586` |
| `perf/results/lxl-sweep/cold-1x-004.json` | no (gitignored / outside repo) | `6a2ff927cf0a937dbde146c9495689a427b7ab256be371a97e3bcdbf369b4d64` |
| `perf/results/lxl-sweep/cold-1x-005.json` | no (gitignored / outside repo) | `43957c4528b36040064ec61f3cf737c30db651f87d2fec8ed4604b4b206781da` |
| `perf/results/lxl-sweep/cold-4x-000.json` | no (gitignored / outside repo) | `99eb3a25af118bea041a2d8370f9ae892ea3193d25e868c9ae1421f31422ba8b` |
| `perf/results/lxl-sweep/cold-4x-001.json` | no (gitignored / outside repo) | `cbd9a66056652343cf146727a1a358b2c7e5cebb72919ab75bc855f7dc62826c` |
| `perf/results/lxl-sweep/cold-4x-002.jsonl` | no (gitignored / outside repo) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `perf/results/lxl-sweep/cold-4x-003.json` | no (gitignored / outside repo) | `15a4d57fbc88c9e03e8a24daeedd2f6ac44cfe8f8f1f5a7b8b13ae18f2d0e413` |
| `perf/results/lxl-sweep/cold-4x-004.json` | no (gitignored / outside repo) | `d5f60eb51bc6bea3719b97f4dfab068dbc607cb2af16074363959a911a2bf0c4` |
| `perf/results/lxl-sweep/cold-4x-005.json` | no (gitignored / outside repo) | `2e885a2cd06d446e68abf330cc3132bdcdd3179b516408b9122a74a04f259f26` |
| `perf/results/lxl-sweep/sweep.log` | no (gitignored / outside repo) | `3a46067a39ce6b826652c58f858c615149e82991318f9bb47aaaf5f86fc19449` |
| `perf/results/lxl-sweep/warm-1x-000.jsonl` | no (gitignored / outside repo) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `perf/results/lxl-sweep/warm-1x-001.json` | no (gitignored / outside repo) | `094a3dca213860536e8625d611db9835697e80c7c8cf11ad3e89747e31e94dde` |
| `perf/results/lxl-sweep/warm-1x-002.jsonl` | no (gitignored / outside repo) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `perf/results/lxl-sweep/warm-1x-003.json` | no (gitignored / outside repo) | `123d74ac6d67d7812914f8d8b105551caf16660096ae9818fb5108257bec5840` |
| `perf/results/lxl-sweep/warm-1x-004.json` | no (gitignored / outside repo) | `fa8e934f2b79d5256c750232b052cad2f578968483315de172757773a6ed2256` |
| `perf/results/lxl-sweep/warm-1x-005.json` | no (gitignored / outside repo) | `aba4fb67e73712738abd432cc7580bff9ced626ae4bced7b651f92be35b10caf` |
| `perf/results/lxl-sweep/warm-4x-000.jsonl` | no (gitignored / outside repo) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `perf/results/lxl-sweep/warm-4x-001.jsonl` | no (gitignored / outside repo) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `perf/results/lxl-sweep/warm-4x-002.jsonl` | no (gitignored / outside repo) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `perf/results/lxl-sweep/warm-4x-003.json` | no (gitignored / outside repo) | `fc4f54dbf20cd4d2e7717bc271250df5c662be1878ce22c39e362181f280ee08` |
| `perf/results/lxl-sweep/warm-4x-004.json` | no (gitignored / outside repo) | `4cbcc0cdb7dfa574e82b5ad265bc66e598be43dfa312e7838022356728547b70` |
| `perf/results/lxl-sweep/warm-4x-005.json` | no (gitignored / outside repo) | `4cffa1a18748a00de92c71df53cae8f0dfab9a128eee18531710d71724eb8ab5` |
| `~/.claude/projects/-workspace-nitro-xml-work-issue-10-lxl/1e83d572-c74b-4385-a8d7-e42d44880ae2.jsonl` | no (gitignored / outside repo) | `7be91da29ba57dae7747f20eb0bd21037a972d161408491d34579d12c5e44536` |

The 0-byte marker files (`cold-4x-002.jsonl`, `warm-*-00[0-2].jsonl`, `warm-4x-001.jsonl`) share the empty-file sha256.

Operator events used as evidence (quoted in the provenance file): transcript L2089 (empty markers for chunk 2), L2104 (`kill -9 4066898`, cold 4x chunk 2), L2295 (empty markers for warm chunks), L2307 (`kill -9 4088154`, warm 1x chunk 0).

## 3. Sample counts per cell (n)

Every number in `BASELINE.md` carries its n. A cell is labelled `thin sample` when n is below the harness design count (cold 10, warm 20; `perf/run.mjs` defaults). Nothing is suppressed; no CI and no speed-win claim is made; p95 is the empirical 95th percentile, not a confidence interval.

- Measured cells in the combined dataset: 912. Thin: 59 (17 S/M, 42 L/XL).
- L/XL: all 42 measured cells are thin (41 at n=3, `gen-pptx-slides-200-b` warm 1x at n=1).
- Thin S/M cells: gen-docx-pages-100 warm 4x n=19; gen-pptx-equation cold 4x n=8; gen-pptx-slides-10-c cold 4x n=5; gen-xlsx-cells-100k-c cold 4x n=3; real-docx-poi-60316.dotx warm 4x n=1; real-docx-poi-Bug54849 cold 1x n=4; real-xlsx-poi-45540_classic_Footer cold 1x n=6; real-xlsx-poi-46535 cold 1x n=3; real-xlsx-poi-47813 cold 4x n=3; real-xlsx-poi-50096 cold 1x n=7; real-xlsx-poi-57893-many-merges cold 1x n=1; real-xlsx-poi-ConditionalFormattingSamples warm 4x n=1; real-xlsx-poi-DataTableCities cold 1x n=9; real-xlsx-poi-WithChartSheet cold 1x n=9; upstream-38aa694fb4a5 warm 4x n=2; upstream-88e14cba7830 warm 4x n=17; upstream-a344b2f3f55d warm 4x n=12.

## 4. gate.test.mjs "noise: gating the same data twice passes"

- Fails on base 36ebc43 (and 700dd58): 38 failing metric rows over 8 keys: real-xlsx-poi-WithEmbeded cold 1x, real-xlsx-usgov-census-NST-EST2024-POP cold 1x, real-docx-poi-Bug54849 cold 4x, real-docx-poi-drawing cold 4x, gen-pptx-slides-10-c cold 4x and warm 1x, gen-xlsx-cells-100k-b cold 4x, real-xlsx-poi-45540_classic_Footer cold 4x.
- Cause: `history.jsonl` has 57 keys (engine, fixture, mode, cpu) with two entries of the same commit 4451709: the original sweep (`rerun:false`, original harness, e.g. unbounded xlsx scroll, often n=1) and the supplemental scroll-fix re-run (`rerun:true`, n=10). `gate.baselineFor` took the last 5 main entries of the key regardless of that marker, so the baseline was the median of both (their mean), and each of the two entries "regressed" against it by more than 10% and the absolute floor.
- Fix (`perf/gate.mjs` `baselineFor`, no threshold or budget change, no history line touched): the baseline pool is the entries with the same `rerun` marker as the current entry; only if there are none is the other class used, still unpooled, so keys that exist only as re-run entries (the promoted xlsx fixtures) keep a baseline. Deterministic: depends only on the existing explicit field.
- Regression tests in `perf/test/gate.test.mjs`: original and fixed-harness entries are not pooled; a re-run-only key is still baselined and still fails a real regression. After the fix the self-gate over `history.jsonl` has 0 fails and 17964 passes.
- Not changed: the 42 L/XL history entries were appended with branch `issue-10-lxl-real`, so the gate (main entries only) does not baseline against them until they are appended from main.

## 5. Remaining blockers for closing #10

- xlsx XL has 2 measured fixtures (`gen-xlsx-xl-a` timed out at 4x); the 3-fixture target is not met. xlsx S 28 and M 7 are measured (the HoP >=3 addendum holds).
- docx L (3) and docx XL (3) and pptx XL (3): 0 fixtures in any aggregate (docx L has cold 1x rows only). pptx L: 3 of 6 in aggregates.
- All L/XL cells are thin (n=3). Re-measuring with the design iteration counts is the only way to remove that label; not done here (no new measurements).
- Warm 4x for every excluded L/XL fixture, and several warm 1x cells, are `not_run`, not measured; the pages-500 warm 1x cells are `operator_aborted`.
- Cause of the pptx XL cold 1x timeouts (suspected synchronous slide loop) and of the 4x timeouts (TS slowness vs box pressure; load average reached 241) is not determined.
- Head of Product sign-off is pending; #10 stays open.
