<!-- SPDX-License-Identifier: MIT -->
# M1 report — Stage 7 gate (#30)

Status: **metrics + TS-vs-WASM tables + live report-mode CI gates** (2026-10-21). Milestone 1
viewer gate record for Nitro XML. Native/WASM/bundle baselines on **`164ac7d`** (#279/#283);
TS-vs-WASM / peak-mem refreshed on **`b48e0e2`** after peak-mem cut [#352](https://github.com/jgridifier/nitro-xml/pull/352)
(+ open warmup/runs [#356](https://github.com/jgridifier/nitro-xml/pull/356); prior paint-intern
[#333](https://github.com/jgridifier/nitro-xml/pull/333)). CI gate scripts in
[#342](https://github.com/jgridifier/nitro-xml/pull/342) (§5.4) — default
`PERF_GATE_MODE=report` (measures and posts LOSSes; does not fail the job). Does **not**
close #23 (docx perf-gate arm deferred). Enforce mode (`PERF_GATE_MODE=enforce`) waits on
Jared.

Upstream pin: ChristopherVR/ooxml @ `e66aaec`. Shipped surface: Rust/WASM only; TS is
test-oracle / pixel reference / perf baseline and never ships.

Reproduce the tables in §5–§6:

```bash
bash perf/large/collect-m1-report.sh
# or stepwise — see commands under each section
```

---

## 1. Visual parity (hard corpus)

Thresholds from `docs/PRODUCT_SPEC.md` §5 and `parity/visual/thresholds.mjs`:

| Format | Hard fixtures | Bound | Result |
|--------|--------------:|-------|--------|
| **xlsx** | 30/30 | ≤0.1% per viewport | **PASS** |
| **docx** | 30/30 | ≤0.5% per page (same page count) | **PASS** |
| **pptx** | 114/114 | ≤0.5% per slide **plus** tile guard | **PASS** |

### pptx tile guard

On top of the 0.5% slide budget, hard pptx cases must pass a local window guard
(`PPTX_TILE_GUARD` in `parity/visual/thresholds.mjs`):

| DPR | Window | Worst-tile mismatch limit |
|-----|--------|---------------------------:|
| 1 | 32 CSS px, stride 16 | ≤20% |
| 2 | 32 CSS px, stride 16 | ≤12% |

### Unsupported-content rate

Unsupported-content rate at the format stage gates: **0** for **xlsx**, **docx**,
and **pptx** (any unsupported kind counts as a failing parity item).

---

## 2. Crash recovery (#30 / #62)

| Acceptance criterion | Status | Notes |
|----------------------|--------|-------|
| Viewer: WASM crash → reload once in a fresh worker; else clear error; CI crash-injection | **Met** | `WorkerDocument.reloadOnce` + `ViewerElement` readonly path; `tests/crash/viewer_reload.mjs` |
| Editor: mid-edit kill → restore unsaved edits + undo stack (xlsx / docx / pptx); graceful UI | **Met** | Op journal (`loader/lazy/journal.js`); `recoverEditOnce`; `EDITOR_RECOVERY_*` messages; harness in `tests/crash/run.mjs` (#276 / #277). Post-#445: journal deferred until first edit; `ViewerElement` edit-recover gates on retained `_originalBytes` (not pre-created `_journal`) so a crash before the first edit still reloads |
| CI `crash-recovery` job gates on every PR (no path filters) | **Met** | `.github/workflows/ci.yml` job has no `paths:` / `paths-ignore:`; default **10** deterministic flake iterations (`NITRO_CRASH_FLAKE_RUNS`) |
| Stub `NOT IMPLEMENTED` removed from `scripts/ci/crash_recovery.sh` | **Met** | Job is live; ask Jared / Chief of Staff to keep it in branch-protection required checks |

Local flakiness proof (harness tip **`94b5523`**, post-#445/#448/#449): **10/10** flake iterations × 3 formats = **30/30** happy-path+undo and **30/30** negative controls (`bash scripts/ci/crash_recovery.sh`).

---

## 3. WASM / package size-limit graceful errors (#268)

`MAX_UNCOMPRESSED_BYTES` (300 MiB) and related zip limits reject oversize packages
with stable machine codes. The loader maps those codes to a single user-facing
string instead of a blank/hung viewer or a raw `ZipError`:

| Code | User message |
|------|----------------|
| `total_too_large` | Package too large. |
| `input_too_large` | Package too large. |
| `part_too_large` | Package too large. |
| `too_many_parts` | Package too large. |
| `compression_ratio` | Package too large. |

Covered for **xlsx, docx and pptx** open paths (`viewerOpenErrorMessage` /
`packages/nitro-xml/loader/test/package-limits.test.mjs`). Loader first-load
brotli budget unchanged at **17 KB** (`perf/budgets.json`).

---

## 4. Fuzz campaign (#30)

≤1 CPU-hour per cargo-fuzz target, **0 crashes / panics** (only `slow-unit-*`
timeout artifacts). Recorded on #30 after #268:

| target | duration_s | execs | notes |
|--------|------------:|------:|-------|
| xml_parse | 3603 | 6,870,696 | 20 min + 40 min top-up |
| opc_open | 3603 | 5,184,795 | 20 min + 40 min top-up |
| zip_open | 3603 | 848,862 | 20 min + 40 min top-up |
| xlsx_load | 3605 | 391,855 | 20 min + 40 min top-up |
| docx_load | 3707 | 973,706 | 20 min + 40 min top-up |
| pptx_load | 3707 | 400,491 | 20 min + 40 min top-up |
| safe_href | 3707 | 129,288,316 | 20 min + 40 min top-up |
| edit_xlsx | 3610 | 13,546 | single ~1 h run |
| edit_docx | 3609 | 112,480 | single ~1 h run |
| edit_pptx | 3609 | 119,000 | single ~1 h run |
| edit_ci | 15 | — | seeded harness (not a long libFuzzer loop) |

Smoke replay: `scripts/ci/fuzz-smoke.sh`.

---

## 5. Performance baseline (existing harnesses)

**Commit:** `164ac7d40f72d68d549bd8218e955a76584e9c7c`  
**Host:** Linux x86_64 (cloud agent VM), 2026-10-19 UTC  
**Collector:** `bash perf/large/collect-m1-report.sh` (wraps the commands below)

Size-tier fixtures (from `fixtures/gen/tiers.py`; L/XL generated on demand, not committed):

| Tier | xlsx | docx | pptx |
|------|------|------|------|
| **S** | `cells_1k.xlsx` (~8 KB) | `pages_10.docx` (~3 KB) | `slides_10.pptx` (~12 KB) |
| **M** | `cells_100k.xlsx` (~492 KB) | `pages_100.docx` (~394 KB) | `slides_50.pptx` (~433 KB) |
| **L** | `cells_1m.xlsx` (~4.9 MB) | `pages_500.docx` (~2.4 MB) | `slides_200.pptx` (~2.5 MB) |
| **XL** | `xl_a.xlsx` (~16.3 MB) | `xl_a.docx` (~11.5 MB) | `xl_a.pptx` (~11.6 MB) |

### 5.1 Native Rust — `perf/large` (#275)

Harness metrics: **median**, **p95**, and **RSS HWM** (process-lifetime; one fixture
per process). This harness does **not** emit TS timings or bootstrap CIs of a
WASM/TS ratio.

```bash
python3 fixtures/gen/generate.py \
  --id gen-xlsx-cells-1k --id gen-xlsx-cells-100k --id gen-xlsx-cells-1m --id gen-xlsx-xl-a \
  --id gen-docx-pages-10 --id gen-docx-pages-100 --id gen-docx-pages-500 --id gen-docx-xl-a \
  --id gen-pptx-slides-10 --id gen-pptx-slides-50 --id gen-pptx-slides-200 --id gen-pptx-xl-a
cargo run --release --manifest-path perf/large/Cargo.toml -- \
  --xlsx-small <fixture> --runs 7   # or --xlsx / --docx / --pptx
```

#### xlsx (native)

| Tier | Fixture | `open_first_view` median / p95 (ms) | scroll `ensure_viewport` (ms) | `load_workbook` median / p95 (ms) | open RSS HWM med (MiB) | full-load RSS HWM med (MiB) |
|------|---------|------------------------------------:|------------------------------:|----------------------------------:|-----------------------:|----------------------------:|
| S | `cells_1k.xlsx` | 1.53 / 1.61 | 0.28 | 1.36 / 1.41 | 5.8 | 6.1 |
| M | `cells_100k.xlsx` | 18.86 / 18.99 | 23.71 | 173.77 / 183.14 | 6.8 | 197.4 |
| L | `cells_1m.xlsx` | **2.27 / 3.64** | 10.86 | 2233.82 / 2323.20 | 15.2 | 1904.9 |
| XL | `xl_a.xlsx` | 4.02 / 10.67 | 11.93 | 2224.67 / 2291.78 | 38.3 | 1917.6 |

Notes (honest):

- **L/XL `open_first_view` is faster than M** after #283 (O(viewport) first paint when
  the sheet `dimension` ref is trustworthy). Full `load_workbook` still scales with
  cell count (~2.2 s / ~1.9 GiB for 1M cells).
- Previous #275 headline on an earlier tip was `open_first_view` **170.8 ms** for
  `cells_1m.xlsx`; tip `164ac7d` measures **2.27 ms** for the same fixture.

#### docx (native) — `PrintSession::open`

| Tier | Fixture | open median / p95 (ms) | RSS HWM med (MiB) |
|------|---------|-----------------------:|------------------:|
| S | `pages_10.docx` | 0.73 / 0.76 | 4.3 |
| M | `pages_100.docx` | 1.42 / 1.48 | 5.5 |
| L | `pages_500.docx` | 4.52 / 4.81 | 11.2 |
| XL | `xl_a.docx` | 5.18 / 5.72 | 20.3 |

Harness prints `docx_pages=0` for these generators (pagination deferred until paint);
timing is still wall-clock for `PrintSession::open`.

#### pptx (native) — `Presentation::open`

| Tier | Fixture | open median / p95 (ms) | RSS HWM med (MiB) | slides |
|------|---------|-----------------------:|------------------:|-------:|
| S | `slides_10.pptx` | 0.33 / 0.41 | 4.5 | 10 |
| M | `slides_50.pptx` | 0.62 / 0.67 | 5.1 | 50 |
| L | `slides_200.pptx` | 1.39 / 1.58 | 7.7 | 200 |
| XL | `xl_a.pptx` | 2.02 / 2.18 | 16.7 | 200 |

### 5.2 WASM — `perf/large/wasm-baseline.mjs`

Metrics the harness already prints: **`openDocument` median**, raw **samples**,
`prepareFrame` median. (Not a browser first-paint; Node `Handle` + `openDocument`.)

```bash
bash scripts/build-wasm.sh   # must match tip SHA
node perf/large/wasm-baseline.mjs <fixture> 7
```

| Format | Tier | Fixture | `openDocument` median (ms) | samples (ms) | `prepareFrame` median (ms) |
|--------|------|---------|---------------------------:|-------------:|---------------------------:|
| xlsx | S | `cells_1k.xlsx` | 8.83 | 22.22, 16.07, 11.30, 8.28, 8.83, 8.11, 8.40 | 6.06 |
| xlsx | M | `cells_100k.xlsx` | 31.35 | 32.04, 31.35, 31.30, 44.96, 30.20, 31.92, 30.40 | 6.53 |
| xlsx | L | `cells_1m.xlsx` | 240.61 | 241.59, 242.19, 238.15, 237.60, 240.61, 239.84, 241.38 | 6.07 |
| xlsx | XL | `xl_a.xlsx` | 268.07 | 268.07, 268.66, 266.91, 266.18, 269.64 | 6.02 |
| docx | S | `pages_10.docx` | 1.67 | 1.82, 1.75, 1.65, 1.75, 1.63, 1.67, 1.56 | ~0 |
| docx | M | `pages_100.docx` | 4.96 | 5.33, 4.89, 5.63, 4.98, 4.95, 4.82, 4.96 | ~0 |
| docx | L | `pages_500.docx` | 16.04 | 23.99, 22.24, 15.89, 15.86, 15.35, 16.04, 16.87 | ~0 |
| docx | XL | `xl_a.docx` | 24.01 | 23.09, 23.59, 24.93, 24.01, 24.29 | ~0 |
| pptx | S | `slides_10.pptx` | 1.47 | 1.82, 1.65, 1.46, 1.47, 1.35, 1.42, 1.51 | ~0.02 |
| pptx | M | `slides_50.pptx` | 4.93 | 5.37, 5.15, 4.93, 5.12, 4.84, 4.81, 4.83 | ~0.02 |
| pptx | L | `slides_200.pptx` | 17.36 | 19.41, 19.23, 19.72, 17.36, 16.24, 16.27, 16.61 | ~0.03 |
| pptx | XL | `xl_a.pptx` | 25.42 | 24.45, 25.10, 25.42, 26.67, 26.08 | ~0.03 |

### 5.3 TS vs WASM cold open — medians + 95% bootstrap CIs

**Report-only** measurement (does **not** arm `perf_beats_ts.sh` / `perf.sh`).

| | |
|--|--|
| **Commit** | `368b311` (xlsx S CF paint cache rebased on `origin/main` incl. #290 CellStore) |
| **When** | 2026-10-20T00:41:05Z |
| **Host** | linux x64, Node v22.14.0 (cloud agent VM) |
| **Command** | `node --expose-gc perf/large/ts-vs-wasm.mjs --runs=11 --warmup=2 --bootstrap=2000` |
| **TS entry** | scoreboard adapters: `loadWorkbook` / `loadDocument` / `PptxHandler.load` |
| **WASM entry** | `Handle.openDocument` (Node `initSync`; not browser first-paint) |
| **Ratio** | `median(wasm) / median(ts)`; 95% CI = unpaired bootstrap of that median ratio (B=2000, seed 20261019) |
| **Verdict** | **WIN** iff median ratio &lt; 1 **and** CI upper &lt; 1 (PRODUCT_SPEC §5); else **LOSS** / **TIE/LOSS** — losses kept |

```bash
# also via the collector:
bash perf/large/collect-m1-report.sh --skip-generate --skip-build-wasm
# or only the compare:
node --expose-gc perf/large/ts-vs-wasm.mjs
# native open/paint breakdown (S only):
cargo run -p nitro-xml-xlsx --release --example xlsx_paint_bench
```

#### xlsx S root cause (measured)

Fixture `cells_1k.xlsx` (~8 KB, 1000 cells, colorScale on `B1:T10`). Metric =
`open_first_view` + first `process_frame` (default 800×600). TS times full `loadWorkbook`
only (no paint).

| Stage (native release, `xlsx_paint_bench`) | Before CF cache (~#294 era) | After rebase on main+#290 (`368b311`) |
|--------------------------------------------|----------------------------:|--------------------------------------:|
| `open_first_view` | ~1.5 ms (§5.1) | **1.29 ms** |
| workbook+sheet clone into `XlsxState` | — | **0.03 ms** (negligible) |
| first-frame `paint_grid_frame` | ~1.85 ms (~97% `cells::paint`) | **0.89 ms** |
| **Native open+paint total** | ~3.4 ms | **~2.2 ms** |

Where the time went before the fix:

1. **CF `range_numbers` per viewport cell** — colorScale / dataBar / icon / top10 re-scanned
   and re-sorted the full CF `sqref` on every cell → O(cells × range) on first paint.
2. **Per-cell CF prep** — colorScale cuts + resolved CSS rebuilt every cell; rule list
   rebuilt/sorted every cell.
3. **`cell_view` alloc churn** — `indexed_palette` allocated a `Vec` per cell; extra
   `DumpCellValue` / `Font` / `Border` clones on the hot path.

Honest fix landed: frame-local `CfPaintCache` (range nums, colorScale prep, rule order,
cover bbox) threaded through `build_items`, plus the `cell_view` alloc cuts. **Did not**
move `process_frame` out of `openDocument` or shrink the default viewport. Rebased cleanly
onto #290 (living CellStore); no CF API conflict.

**Peak memory (native `perf/large`, S):** `open_first_view` RSS HWM median **~5.7 MiB**
(was 5.8 MiB in §5.1) — no material change. #290 targets large-file RSS, not this S fixture.

#### Before → after (xlsx S primary)

| | TS median (ms) | WASM median (ms) | WASM/TS | 95% CI | Notes |
|--|---------------:|-----------------:|--------:|-------:|-------|
| **#294 / `80bbf59`** | 4.35 | 8.46 | **1.945 LOSS** | [1.227, 2.830] | Prior tip |
| **This PR on main+#290 / `368b311`** | 4.35 | 7.23 | **1.663 LOSS** | [1.495, 2.894] | Full xlsx matrix run |
| 8× fresh-process probe (same SHA) | 4.35 | ~7.5–11.7 | sample ratios 1.49–2.59 | — | S is noisy; median-of-ratios ~2.26 |

Still a **LOSS** after the CF paint cache (~1.66× on `368b311`). Residual gap was
**WASM encode / command-buffer + StringTable tax** on open+paint.

#### Encode cut (this slice — gate metric unchanged)

Same cold-open metric (`Handle.openDocument` = open_first_view + `process_frame`). No
parse/paint split, no threshold/budget/fixture changes.

| Change | Detail |
|--------|--------|
| `StringTable` | FNV-1a hash→id index (single owned copy per string); wasm-safe, no `RandomState` |
| `CommandBuffer` | `reserve(128 KiB)` on xlsx paint frame; tighter `put_*` writes |
| Cell paint | `PaintKeys` (prop ids once/frame); stacked geometry intern; text styles batched into one `style` attr (same pattern as geometry) |
| `cmd_bytes` (`cells_1k`) | **106744 → 54354** (~49%) |

Fair same-VM remeasure after rebase onto `main` (#302/#303 tip `8fc6a6f`):

| | TS median (ms) | WASM median (ms) | WASM/TS | 95% CI | openDocument median (ms) |
|--|---------------:|-----------------:|--------:|-------:|-------------------------:|
| **Before** (`main` `8fc6a6f`) | 4.24 | 7.26 | **1.711 LOSS** | [1.188, 2.686] | 7.22 |
| **After** (encode `402fb2e`) | 4.96 | 5.64 | **1.137 LOSS** | [1.050, 1.452] | 3.88 |

| Peak memory (xlsx S / `cells_1k`) | Before | After |
|----------------------------------|-------:|------:|
| WASM linear peak | 3.25 MiB | 3.44 MiB |
| JS heap peak | 4.20 MiB | 4.20 MiB |
| Native `open_first_view` RSS HWM median | 5.68 MiB | 5.69 MiB |

Still a **LOSS** at the encode tip (CI lower still ≥1.05). Absolute WASM cold-open median
**7.26 → 5.64 ms** (~22%); openDocument-only median **7.22 → 3.88 ms**. Peak memory **flat**
(encode shrinks cmdbuf bytes, not retained package residency). Gate metric unchanged.

#### Paint-intern cut ([#333](https://github.com/jgridifier/nitro-xml/pull/333) — merged)

Same cold-open metric. StylePaintCache / `Rc<FontView>` / FontCssMemo / General fast-path /
StringTable sticky intern / FNV MeasureBatch. Parity fix: blank-cell skip used emptied
`view.text` after `mem::take` — switched to `CellItem::is_blank()`. No threshold / budget /
fixture demotion. Loader brotli still ≤17408 B.

| | TS median (ms) | WASM median (ms) | WASM/TS | 95% CI | Verdict |
|--|---------------:|-----------------:|--------:|-------:|---------|
| **Before** (encode tip) | 4.96 | 5.64 | **1.137 LOSS** | [1.050, 1.452] | LOSS |
| **After** (`main` `f2b68fa`, local) | 4.13 | 2.56 | **0.621** | [0.543, 0.803] | **WIN** |

Local full S/M/L matrix on `f2b68fa` (`ts-vs-wasm.mjs --runs=11`, 2026-10-21T00:52Z):

| Format | Tier | TS median (ms) | WASM median (ms) | WASM/TS | 95% CI | Verdict |
|--------|------|---------------:|-----------------:|--------:|-------:|---------|
| xlsx | S | 4.13 | 2.56 | 0.621 | [0.543, 0.803] | **WIN** |
| xlsx | M | 177.39 | 14.18 | 0.080 | [0.079, 0.083] | **WIN** |
| xlsx | L | 2032.97 | 2.96 | 0.001 | [0.001, 0.002] | **WIN** |
| docx | S | 2.41 | 0.81 | 0.338 | [0.288, 0.382] | **WIN** |
| docx | M | 7.06 | 1.63 | 0.230 | [0.170, 0.272] | **WIN** |
| docx | L | 26.69 | 3.27 | 0.123 | [0.119, 0.126] | **WIN** |
| pptx | S | 13.11 | 0.37 | 0.029 | [0.027, 0.033] | **WIN** |
| pptx | M | 50.98 | 0.72 | 0.014 | [0.013, 0.016] | **WIN** |
| pptx | L | 182.58 | 2.21 | 0.012 | [0.012, 0.013] | **WIN** |

Local peak-mem (`peak-mem.mjs --runs=5`, same SHA) — losses kept:

| Format | Tier | TS peak MiB | WASM peak MiB | WASM/TS | 95% CI | Verdict |
|--------|------|------------:|--------------:|--------:|-------:|---------|
| xlsx | S | 5.7 | 30.4 | 5.348 | [4.953, 6.803] | **LOSS** |
| xlsx | M | 131.2 | 38.3 | 0.292 | [0.281, 0.310] | **WIN** |
| xlsx | L | 1008.3 | 44.7 | 0.044 | [0.044, 0.048] | **WIN** |
| docx | S | 2.3 | 27.3 | 11.935 | [9.882, 12.856] | **LOSS** |
| docx | M | 6.7 | 27.8 | 4.173 | [3.855, 4.419] | **LOSS** |
| docx | L | 22.6 | 39.1 | 1.733 | [1.663, 1.805] | **LOSS** |
| pptx | S | 10.1 | 21.3 | 2.104 | [2.054, 2.265] | **LOSS** |
| pptx | M | 34.6 | 27.3 | 0.789 | [0.737, 0.814] | **WIN** |
| pptx | L | 41.6 | 33.2 | 0.799 | [0.782, 0.867] | **WIN** |

**Wins / losses summary (openDocument, `f2b68fa`):** WASM wins **9 / 9** cells locally
(xlsx S flipped LOSS → WIN: **5.64 → 2.56 ms** WASM median). Peak-mem LOSSes remained
(wasm compile floor) — not demoted. Pre-#352 CI sometimes posted xlsx S openDocument as
**TIE/LOSS** when the 95% CI upper spanned 1.0 — see §5.4 post-#352 refresh (now a clear WIN).

#### Post-#352 refresh (`b48e0e2` + measure sticky size/bold)

[#352](https://github.com/jgridifier/nitro-xml/pull/352) (wasm-opt one-caller inline cap +
sheet-clone drops; **Cargo.toml wasm-opt flags owned by that stream — not retouched here**),
[#351](https://github.com/jgridifier/nitro-xml/pull/351) (mut-frame), [#356](https://github.com/jgridifier/nitro-xml/pull/356)
(open warmup 8 / runs 21). This slice adds a **real** xlsx S paint speedup: sticky
`estimate_width` size/bold on `CanvasTextMeasure` (skip per-cell CSS parse) + `intern_px`
for border/editor geometry (no `format!("{}px")`). No extra warmup/runs; pass rule /
budgets / fixtures unchanged.

| | TS median (ms) | WASM median (ms) | WASM/TS | 95% CI | Verdict |
|--|---------------:|-----------------:|--------:|-------:|---------|
| **Before** (CI tip pre-#352, `928926d` era) | — | — | **0.945** | [0.610, **1.187**] | **TIE/LOSS** |
| **After #352** (CI `perf-beats-ts (xlsx)` on `b48e0e2`) | 5.86 | 1.58 | **0.269** | [0.228, 0.321] | **WIN** |
| **+ measure cut** (local, same gate knobs) | ~5.4 | ~1.66 | **~0.31** | [~0.22, ~0.41] | **WIN** |
| Noisy probe (warmup=2 / runs=11, local) | 6.00 | 1.78 | **0.297** | [0.266, 0.354] | **WIN** |

CI upper stays **well below 1** without relying on warmup/runs alone (noisy probe CI upper
0.354). Native `xlsx_paint_bench` paint **0.387 → 0.370 ms**; `cmd_bytes` flat at 56254.

**Honest scope notes:**

- Metric in §5.3 is **cold open only**. Peak memory is measured in CI (§5.4). Scroll /
  browser first-paint (incl. wasm fetch+compile+instantiate) and 4× CPU / XL remain TODO(#8).
- **xlsx asymmetry:** TS `loadWorkbook` is a full workbook parse; WASM `openDocument` uses
  `open_first_view` (viewport) + first `process_frame`, and after #289 does **not** SeekTail
  on that first frame. That is the viewer TTFV path, not identical work — especially visible
  at L/XL.
- Gate scripts are **live in report mode** — see §5.4 ([#342](https://github.com/jgridifier/nitro-xml/pull/342)).

**Docx perf-gate arm stays deferred under #23; this slice does not close #23.**

### 5.4 WASM-beats-TS CI gate — report mode ([#342](https://github.com/jgridifier/nitro-xml/pull/342))

`scripts/ci/perf_beats_ts.sh` and `scripts/ci/perf.sh` no longer print `NOT IMPLEMENTED`.
They generate S/M/L fixtures, build wasm, run `perf/large/ts-vs-wasm.mjs` (openDocument) and
`perf/large/peak-mem.mjs` (VmHWM growth + 95% bootstrap CI), and write a per-test table to
`$GITHUB_STEP_SUMMARY`.

| | |
|--|--|
| **PR** | [#342](https://github.com/jgridifier/nitro-xml/pull/342) (merged) |
| **Pass rule** | median WASM/TS &lt; 1 **and** 95% CI upper &lt; 1 (PRODUCT_SPEC §5); a tie fails |
| **Mode** | default `PERF_GATE_MODE=report` — LOSS/TIE posted, job exits 0. Jared: set `enforce` to arm |
| **Not demoted** | budgets, fixtures, loader ceiling (≤17408 B brotli) unchanged |
| **Open knobs** | warmup=8 / runs=21 ([#356](https://github.com/jgridifier/nitro-xml/pull/356)); peak-mem knobs untouched |

#### Post-#352 matrix (`b48e0e2`)

Local `PERF_GATE_MODE=report bash scripts/ci/perf.sh gate` on tip `b48e0e2` —
**14/18 WIN · 4 LOSS/TIE**. CI `perf.yml` `perf-beats-ts (*)` on the same SHA (ubuntu-latest)
corroborates; peak-mem LOSSes kept (bar not lowered):

| Format | Tier | Metric | Before (#348 era) | After #352 (local / CI) | Verdict |
|--------|------|--------|-------------------:|------------------------:|---------|
| xlsx | S | openDocument | 0.945 [0.610, **1.187**] | **0.333** [0.255, 0.377] local · **0.269** [0.228, 0.321] CI | **WIN** |
| xlsx | S | peak_mem | 6.841 [6.501, 7.186] | **3.355** [3.186, 3.535] local · 3.515 CI | **LOSS** |
| xlsx | M | peak_mem | WIN | 0.153 / 0.164 | **WIN** |
| xlsx | L | peak_mem | WIN | 0.026 / 0.026 | **WIN** |
| docx | S | peak_mem | 15.664 | **5.979** local · 7.040 CI | **LOSS** |
| docx | M | peak_mem | 4.425 | **2.554** local · 2.932 CI | **LOSS** |
| docx | L | peak_mem | 1.765 | 1.011 local · **0.932** CI | local TIE/LOSS · **CI WIN** |
| pptx | S | peak_mem | 1.803 | **0.548** local · 0.476 CI | **WIN** (was LOSS) |
| pptx | M | peak_mem | 1.104 | **0.362** local · 0.531 CI | **WIN** (was LOSS) |
| pptx | L | peak_mem | WIN | 0.463 / 0.390 | **WIN** |

All S/M/L **openDocument** cells: **WIN** (xlsx/docx/pptx). Absolute before→after peak
(WASM MiB, local gate): xlsx S **30.4 → 19.0**; docx S **27.3 → 14.3**; pptx S **21.3 → 5.4**.
Remaining LOSSes are the wasm compile floor on small TS footprints (see
`docs/perf-rust-hotpaths.md`). Peak-mem work continues on another stream — this slice does
not touch `rs/crates/nitro-xml-wasm/Cargo.toml` wasm-opt flags.

#### Tip enforce-readiness refresh (`a5a58d6`, 2026-10-22)

`PERF_GATE_MODE=report bash scripts/ci/perf_beats_ts.sh <fmt>` on tip **`a5a58d6`**
(#407 readiness + #406 parse publish + #405 xlsx S outline; open warmup=8 / runs=21;
peak runs=7). Bar unchanged. Each format writes `enforce-readiness.json` + a summary
footer (`scripts/ci/perf_beats_ts.sh`); `perf.sh` rollup aggregates READY / NOT READY.
**Does not** flip enforce (Jared / #8).

| Format | openDocument S/M/L | peak_mem S/M/L | Enforce ready? | Blockers |
|--------|--------------------|----------------|:--------------:|----------|
| **pptx** | WIN / WIN / WIN | WIN / WIN / WIN | **READY** (6/6) | — |
| **xlsx** | WIN / WIN / WIN | **LOSS** / WIN / WIN | NOT READY (5/6) | xlsx S peak_mem (~3.09× [2.71, 3.32]; ~16.9 vs 5.5 MiB; compile floor) |
| **docx** | WIN / WIN / WIN | **LOSS** / **LOSS** / WIN | NOT READY (4/6) | docx S/M peak_mem (~5.35× / ~2.30×); arm deferred under **#23** |

Tip openDocument WASM/TS (median [95% CI]): xlsx S **0.301** [0.240, 0.367]; pptx S **0.028**
[0.026, 0.031]; docx S **0.247** [0.237, 0.268]. pptx S peak_mem **0.513** [0.498, 0.561] WIN.

#### Known-loss exception — xlsx S + docx S/M peak_mem (Jared 2026-10-22)

**Decision:** [Jared on #30](https://github.com/jgridifier/nitro-xml/issues/30#issuecomment-6341154384)
accepts the first-open peak-memory LOSS on **xlsx S** and **docx S/M** as a known V8
TurboFan compile floor for M1. Tip numbers cited in that comment: xlsx S 14.75 vs 5.72 MiB
TS, docx S 13.52 vs 2.29, docx M 14.71 vs 6.33; liftoff-only 3.8 MiB.

**Gate behaviour (this slice):** those exact cells stay **measured and printed as LOSS**
with their numbers in `perf_beats_ts.sh` / `perf.sh` rollup / this standings table. They are
recorded as documented known-loss exceptions so they **do not** block enforce readiness and
**do not** fail `PERF_GATE_MODE=enforce`. No fixture demotion; no threshold or
`perf/budgets.json` change. **Any other cell that is LOSS / TIE/LOSS still fails.**

Tip remasure on **`3924a77`** (`PERF_GATE_MODE=report bash scripts/ci/perf_beats_ts.sh <fmt>`,
open warmup=8 / runs=21, peak runs=7; post-#449/#452 peak outline + #454/#458/#456):
**15/18 WIN · 3 LOSS/TIE** (all three LOSSes are the documented known-loss cells).
Per-format enforce readiness:

| Format | openDocument S/M/L | peak_mem S/M/L | Enforce ready? | Blockers | Known-loss (still LOSS) |
|--------|--------------------|----------------|:--------------:|----------|-------------------------|
| **pptx** | WIN / WIN / WIN | WIN / WIN / WIN | **READY** (6/6) | — | — |
| **xlsx** | WIN / WIN / WIN | **LOSS** / WIN / WIN | **READY** (5/6) | — | xlsx S peak_mem **2.682** [2.471, 3.256] (~15.7 vs 5.9 MiB) |
| **docx** | WIN / WIN / WIN | **LOSS** / **LOSS** / WIN | **READY** (4/6) | — | docx S **3.896** [3.810, 4.019] (~10.6 vs 2.7 MiB); docx M **2.012** [1.769, 2.101] (~12.9 vs 6.4 MiB) |

Tip openDocument WASM/TS (median [95% CI]): xlsx S **0.312** [0.297, 0.384]; pptx S **0.029**
[0.028, 0.033]; docx S **0.257** [0.232, 0.284]. pptx S peak_mem **0.492** [0.472, 0.514] WIN.

**Ask Jared:** flip `PERF_GATE_MODE=enforce` for formats marked READY (all three after the
known-loss exception) and add `perf-beats-ts` to branch-protection required checks. Agents do
not edit `.github/workflows/` or branch protection. **#415** (viewer-first / per-format milder
opt) left alone — parked `todo-later`.

Merged on `main` ahead of this tip refresh: [#458](https://github.com/jgridifier/nitro-xml/pull/458)
(first-paint), [#456](https://github.com/jgridifier/nitro-xml/pull/456) (A4 page size),
[#454](https://github.com/jgridifier/nitro-xml/pull/454) (defer originalBytes),
[#449](https://github.com/jgridifier/nitro-xml/pull/449)/[#452](https://github.com/jgridifier/nitro-xml/pull/452)
(peak outline), plus earlier [#444](https://github.com/jgridifier/nitro-xml/pull/444) /
[#421](https://github.com/jgridifier/nitro-xml/pull/421) known-loss / readiness work.

#### xlsx S peak_mem — compile-floor attribution (`68c7ba6`)

`peak-mem.mjs` measures **VmHWM growth over the post-`initSync` baseline** (fresh process per
open). On tip `68c7ba6` (local cloud agent VM, 2026-10-21):

| Metric | TS | WASM | WASM/TS | Verdict |
|--------|---:|-----:|--------:|---------|
| openDocument median (ms) | 5.54 | 1.69 | **0.305** [0.215, 0.368] | **WIN** |
| peak_mem median (MiB) | 5.7 | **18.1** | **3.148** [3.010, 3.299] | **LOSS** |

Stage split of absolute process HWM (median of 7–9 fresh processes; Node base ~45 MiB):

| Stage | HWM (MiB) | Δ vs prior |
|-------|----------:|-----------:|
| Node only | 45.2 | — |
| `import` glue JS | 45.5 | +0.3 |
| `initSync` (load 6.5 MB module) | 63.3 | **+17.8** |
| `new Handle(bytes)` | 63.8 | +0.5 |
| first `openDocument` | 81.1 | **+17.3** |
| second `openDocument` (new Handle, same process) | 83.1 | **+2.2** |

Gate-shaped probe (baseline after `initSync`, then open) — median of 9:

| Probe | Peak over baseline (MiB) |
|-------|-------------------------:|
| Cold first open (matches gate) | **18.0** |
| Second open after warmup open | **2.3** |

Reading: **~87% of the xlsx S peak_mem LOSS is V8's first-touch wasm compile** of the
functions the open path executes (Liftoff/TurboFan). Retained document/state alloc is
~2.3 MiB — already **below** TS's 5.7 MiB. No application-heap cut on the xlsx open path
can flip the gate cell without shrinking that compile floor.

Levers that do move the floor (owned elsewhere; **not** changed in this slice):

| Lever | Owner | Expected effect |
|-------|-------|-----------------|
| `wasm-opt -O1` (drop `-O3` inlining tax) | draft [#332](https://github.com/jgridifier/nitro-xml/pull/332) | ~−15 to −20 MiB first-open RSS (measured in hotpaths) |
| Format-split `openDocument` / `process_frame` | draft [#359](https://github.com/jgridifier/nitro-xml/pull/359) (docx peak) | May shrink shared compile surface; do not duplicate here |
| Cap already shipped | [#352](https://github.com/jgridifier/nitro-xml/pull/352) `--one-caller-inline-max-function-size=1500` | xlsx S peak **30.4 → ~18–19 MiB** |

Bar not lowered; budgets/fixtures/loader ceiling unchanged; `Cargo.toml` wasm-opt flags
untouched.

#### Rejected: throwaway open+free “pre-warm cut” (draft [#364](https://github.com/jgridifier/nitro-xml/pull/364) closed)

A throwaway open+free during init / peak-mem baseline was tried so first-touch TurboFan
would sit outside the measured open window. That **gameses** the post-init delta and adds
startup work (against React-embed priority). Honest re-measure (2026-10-21):

- **Whole-process peak RSS** = VmHWM growth from process start (includes import, init,
  optional prewarm, and open); fresh process per sample; **n = 50**; WASM cold vs
  WASM prewarm vs TS.
- **Cold mount→first-paint** and **TTI** (`init_core_ms + mount_to_paint_ms`) via
  `perf/react-embed` first-paint scenario; fresh browser context per sample; **n = 50**;
  before = worker without stub opens; after = worker with stub open+free.

**Verdict:** whole-process peak does **not** drop (xlsx S WASM p50 **37.1 → 38.8 MiB**);
React-embed startup **regresses** (xlsx S mount p50 **129.6 → 159.0 ms**, TTI
**190.2 → 220.1 ms**). Draft #364 closed; code reverted. Confirms the §5.4 attribution:
**xlsx S peak_mem remains a LOSS at the wasm compile floor** — not demoted.

Whole-process peak (VmHWM from process start, MiB, n=50):

| Format | Tier | TS p50/p95 | WASM cold p50/p95 | WASM prewarm p50/p95 | cold→prewarm Δp50 |
|--------|------|-----------:|------------------:|---------------------:|------------------:|
| xlsx | S | 31.3/31.9 | 37.1/42.5 | 38.8/46.3 | **+1.7** |
| xlsx | M | 154.2/158.1 | 39.2/41.5 | 49.8/52.3 | +10.6 |
| xlsx | L | 1042.0/1045.6 | 52.8/55.5 | 64.8/70.5 | +11.9 |
| docx | S | 19.8/20.2 | 31.8/32.9 | 32.4/33.4 | +0.5 |
| docx | M | 23.9/24.8 | 34.9/35.5 | 34.9/35.2 | +0.0 |
| docx | L | 43.6/44.8 | 43.7/44.6 | 45.8/46.4 | +2.1 |
| pptx | S | 63.3/64.1 | 23.6/24.1 | 24.4/25.3 | +0.8 |
| pptx | M | 90.1/91.4 | 32.2/32.6 | 33.5/35.5 | +1.3 |
| pptx | L | 100.2/103.4 | 43.2/44.5 | 46.6/48.0 | +3.4 |

React-embed cold mount→paint / TTI (ms, n=50):

| Format | Tier | mount before p50/p95 | mount after | tti before | tti after |
|--------|------|---------------------:|------------:|-----------:|----------:|
| xlsx | S | 129.6/143.0 | 159.0/181.3 | 190.2/205.8 | 220.1/245.3 |
| xlsx | M | 143.6/158.6 | 184.2/208.6 | 203.0/217.1 | 245.8/274.4 |
| xlsx | L | 126.4/221.9 | 161.7/260.6 | 184.4/281.6 | 222.9/324.5 |
| docx | S | 125.5/141.0 | 157.0/170.7 | 182.5/198.6 | 216.8/231.9 |
| docx | M | 136.5/142.8 | 164.4/187.0 | 194.1/206.1 | 225.6/252.6 |
| docx | L | 160.3/178.4 | 199.0/225.6 | 218.8/238.8 | 261.5/287.4 |
| pptx | S | 97.3/105.3 | 145.4/158.0 | 155.8/167.1 | 207.2/221.4 |
| pptx | M | 100.4/111.5 | 138.4/152.2 | 158.0/169.8 | 197.7/215.6 |
| pptx | L | 108.1/116.8 | 136.9/146.1 | 166.8/177.1 | 195.1/207.5 |

```bash
PERF_GATE_MODE=report bash scripts/ci/perf.sh gate
# arm later:
PERF_GATE_MODE=enforce bash scripts/ci/perf_beats_ts.sh xlsx
```

---

## 6. Bundle / loader

Measured on the same SHA after `bash scripts/build-wasm.sh`:

```bash
bash scripts/ci/bundle_budget.sh
```

| Artifact | Size | Ceiling / notes |
|----------|-----:|-----------------|
| Loader JS (hand-written + wasm-pack glue), brotli q=11 | **17093 B (16.69 KB)** | ≤ **17 KB** (`perf/budgets.json`); `loader/lazy/` excluded |
| `nitro_xml_wasm_bg.wasm` raw | 6774158 B (~6.46 MB) | Per-format payload ceilings TODO(#46) |
| `nitro_xml_wasm_bg.wasm` brotli q=11 | 1512324 B (~1.44 MB) | Informational only (not gated) |
| `nitro_xml_wasm.js` glue (uncompressed) | 30879 B | Included in loader brotli measure above |

`bundle_budget.sh` also prints: `TODO(#46): per-format initial-payload ceilings and time-to-interactive not yet gated`.

---

## 7. Scoreboard standings — Nitro entered in published parse-latest

Self-contained Nitro WASM **parse adapters** landed in
[#299](https://github.com/jgridifier/nitro-xml/pull/299) /
[#329](https://github.com/jgridifier/nitro-xml/pull/329) (+ single-parse xlsx
[#388](https://github.com/jgridifier/nitro-xml/pull/388), pptx extract
[#384](https://github.com/jgridifier/nitro-xml/pull/384)): calamine-style shims +
`competitors.json` **entered**.

**Published** `docs/scoreboard/data/parse-latest.json` (tip **`bbbea70`**, method knobs
warmup=2 / runs=7, all entered competitors including **betteroffice-docx**; **not invented**;
remasure 2026-10-23 after #450/#448/#444/#443 wasm-opt `-O1`):
Nitro status **`entered`**. Archive: `perf/scoreboard/results/parse/2026-10-23-bbbea70.json`.
Prior published tip was **`892f06a`**. Draft [#113](https://github.com/jgridifier/nitro-xml/pull/113) /
stub `scoreboard.sh` (#39) **untouched** — CI reproducible gate still open.

| Format | Nitro comparable | Geomean vs TS | Nitro W/L | Errors |
|--------|-----------------:|--------------:|:---------:|-------:|
| xlsx | **30/30** | **0.252** | **30W/0L** | 0 |
| docx | **30/30** | **0.204** | **30W/0L** | 0 |
| pptx | **30/30** | **0.085** | **30W/0L** | 0 |

**Totals:** Nitro **90W/0L**, **90/90** comparable. Equivalence: timed `parse()` only; xlsx Nitro is
`load_workbook_meta` + `parse_all_worksheets` (one pass per sheet) vs TS `loadWorkbook` (see #388).
Losses for other entrants kept in the published JSON (e.g. exceljs / mammoth).
**betteroffice-docx** on this tip: **21/30** comparable, geomean **0.193**, **21W/0L** (nine NC —
drawings / fields / content-controls / revisions).

#### Tip nitro↔upstream parse remasure (`09a82b3`, 2026-10-22)

Local remasure on tip **`09a82b3`** (post-#413 / #411), nitro-xml vs upstream-ts only
(warmup=1 / runs=3 — same knobs as [#400](https://github.com/jgridifier/nitro-xml/pull/400)).
Does **not** overwrite `parse-latest.json` (keeps full competitor field from #406) and does
**not** touch draft [#113](https://github.com/jgridifier/nitro-xml/pull/113).
Archive: `perf/scoreboard/results/parse/2026-10-22-09a82b3-nitro-upstream.json`.

| Format | Nitro comparable | Geomean vs TS | Nitro W/L | Errors |
|--------|-----------------:|--------------:|:---------:|-------:|
| xlsx | **30/30** | **0.283** | **30W/0L** | 0 |
| docx | **30/30** | **0.257** | **30W/0L** | 0 |
| pptx | **30/30** | **0.091** | **30W/0L** | 0 |

**Totals:** Nitro **90W/0L**, **90/90** comparable on tip. Median-of-medians: xlsx **6.48** ms
(vs TS 18.82), docx **4.16** (vs 16.50), pptx **4.67** (vs 59.28).

#### BetterOffice docx parse entrant (`b0eca79` → published in tip `3a9765a`)

Entered **`@betteroffice/docx@0.4.3`** (Apache-2.0) in the docx parse race — Rust/WASM
`parseDocx` with opc+parse wasm only (layout/edit unused). Adapter:
`perf/scoreboard/adapters/js/betteroffice-docx.mjs`. First entry race on tip
**`b0eca79`** (warmup=1 / runs=3): **21/30** comparable, geomean **0.219**, **21W/0L**
(archive `2026-10-22-b0eca79-betteroffice-docx.json`). Now included in published
`parse-latest.json` at tip **`3a9765a`** (method 2+7; see table above). Draft
[#113](https://github.com/jgridifier/nitro-xml/pull/113) **untouched**.

**Render-to-first-paint** ([#41](https://github.com/jgridifier/nitro-xml/issues/41) closed): `perf/scoreboard/races/render/`
times load+init+first paint (800×480, DPR 1). **xlsx** and **pptx** are entered for `nitro-xml` vs
`upstream-ts`; docx deferred (#23). Equivalence unchanged from [#390](https://github.com/jgridifier/nitro-xml/pull/390).

**Published** `docs/scoreboard/data/first-paint-latest.json` at tip **`b2dcc3c`** (tiers
**S/M/L/XL**, 3 timed + 1 warmup, `--timeout=600000`; remasure 2026-10-23 after
#454 defer host originalBytes + #455 parse-latest + #451). Archive:
`perf/scoreboard/results/render/2026-10-23-b2dcc3c.json`. Draft **#113** / stub
`scoreboard.sh` (#39) **untouched**. Prior published tip was **`642a7dd`**
(`2026-10-23-642a7dd.json`). Parked **#415** untouched.

Harness note (from #413, still in effect): Nitro adapters no longer call host
`initCore` (that double-instantiated wasm vs the worker); they use
`setDefaultFontSource` + `preload()` like React-embed.

| Format | Tier | Nitro median (ms) | TS median (ms) | Comparable | W/L |
|--------|------|------------------:|---------------:|:----------:|:---:|
| xlsx | S | 170 | 778 | yes | **W** |
| xlsx | M | 204 | 978 | yes | **W** |
| xlsx | L | 285 | 2301 | yes | **W** |
| xlsx | XL | 339 | 2207 | yes | **W** |
| pptx | S | 179 | 638 | yes | **W** |
| pptx | M | 189 | 679 | yes | **W** |
| pptx | L | 187 | 878 | yes | **W** |
| pptx | XL | 237 | 897 | yes | **W** |

Nitro **8W/0L** on comparable cells (pptx Nitro pixels **0.044%** vs 0.5% limit).

**Crash recovery** on tip **`94b5523`**: `bash scripts/ci/crash_recovery.sh` → **PASS** (default **10** flake iterations × 3 formats; post-#445 defer-journal edit-recover gate fix).

**M1 gate still open:** CI `scoreboard` job stub (#39) + draft **#113** (reproducible gate; not the Pages data file); **`perf-beats-ts` enforce** (#8) — tip readiness: **xlsx/docx/pptx READY** with documented known-loss exceptions still printed as **LOSS** (xlsx S + docx S/M peak_mem; [Jared](https://github.com/jgridifier/nitro-xml/issues/30#issuecomment-6341154384)). Leftover agent-facing work: Jared flips `PERF_GATE_MODE=enforce` + required-check wiring (agents do not).

| Still blocked for CI scoreboard gate | Link | Why |
|--------------------------------------|------|-----|
| `scripts/ci/scoreboard.sh` | stub → `#39` | Still `NOT IMPLEMENTED — see #39` |
| Scoreboard CI / reproducible publish | draft [#113](https://github.com/jgridifier/nitro-xml/pull/113) | Draft; **do not touch** (per Jared) |

Published `parse-latest.json` and `first-paint-latest.json` are real measured tip data
(Nitro **entered** on both).

---

## 8. Open follow-ups

| Item | Issue / PR | Notes |
|------|------------|-------|
| Flip WASM-beats-TS to **enforce** | **#8**, **#19**, **#27**; docx **#23** | Tip readiness: **xlsx/docx/pptx READY** with known-loss exceptions still printed as LOSS (xlsx S + docx S/M peak_mem; [Jared](https://github.com/jgridifier/nitro-xml/issues/30#issuecomment-6341154384)). Jared flips `PERF_GATE_MODE=enforce` + required checks; agents do not |
| xlsx S peak_mem LOSS | compile floor (measured; **known-loss**) | Tip **`3924a77`** (post-#449/#452/#454/#458): **2.682** [2.471, 3.256] still **LOSS**. Accepted for M1 ([comment](https://github.com/jgridifier/nitro-xml/issues/30#issuecomment-6341154384)). Does not block enforce readiness |
| Docx S/M peak_mem LOSSes | compile floor (measured; **known-loss**) | Tip **`3924a77`** (post-#449/#452/#454/#458): S **3.896** / M **2.012** still **LOSS**. Accepted for M1 ([comment](https://github.com/jgridifier/nitro-xml/issues/30#issuecomment-6341154384)). Does not block enforce readiness |
| xlsx S openDocument | done (clear WIN) | [#360](https://github.com/jgridifier/nitro-xml/pull/360); tip CI upper ≪ 1 |
| Scoreboard CI gate (reproducible) | draft **#113**, **#39** | Pages `parse-latest.json` + `first-paint-latest.json` have real Nitro tip data; CI stub / #113 still open |
| Keep `crash-recovery` / `perf-beats-ts` in branch-protection required checks | Jared / Chief of Staff | Agents do not edit branch protection; add `perf-beats-ts` only after enforce |
| Per-format wasm / TTI ceilings | #46 | Bundle gate extension |

---

## 9. Definition of done (Stage 7 checklist)

- [x] Fuzzing clean (1 CPU-hour / target, 0 crashes)
- [x] Crash recovery tested (viewer + editor; CI injection; flake loop)
- [x] Memory / package ceiling enforced with graceful viewer errors
- [x] M1 report published with **real measured** numbers from existing harnesses (native/WASM/bundle on `164ac7d`; TS-vs-WASM cold open refreshed on `368b311` after xlsx S CF/paint cut + #290 rebase)
- [x] Unsupported-content rate recorded (**0** × 3 formats)
- [x] Bundle / loader sizes recorded (loader **16.69 / 17 KB**; wasm sizes above)
- [x] Per-format×tier **TS vs WASM** cold-open medians + 95% bootstrap CIs recorded (§5.3), **including xlsx S clear WIN after #352** (CI upper 0.321; peak-mem LOSSes kept on xlsx/docx S); **#23 left open** (docx arm deferred)
- [x] CI gate scripts real in **report mode** (§5.4 / [#342](https://github.com/jgridifier/nitro-xml/pull/342)) — bar unchanged; LOSSes posted; enforce not flipped; post-#352 matrix **14/18 WIN**
- [x] Scoreboard standings including losses — adapters **#299/#329/#388/#384**; published `parse-latest.json` Nitro **entered**, tip **`bbbea70`** **90W/0L** (method 2+7, includes betteroffice-docx **21W/0L**); tip nitro↔upstream remasure on **`09a82b3`** still **90W/0L**; published `first-paint-latest.json` tip **`b2dcc3c`** **8W/0L** S–XL (remasure after #454/#455/#451); peak_mem standings keep xlsx S + docx S/M as **LOSS** with known-loss exception ([Jared](https://github.com/jgridifier/nitro-xml/issues/30#issuecomment-6341154384)); CI `scoreboard` job / draft **#113** still open (not required for Pages data)
- [ ] `crash-recovery` / `perf-beats-ts` added to branch-protection required checks — **Jared / Chief of Staff only** (add `perf-beats-ts` only after `PERF_GATE_MODE=enforce`)
- [ ] Flip `perf-beats-ts` to **enforce** — **#8 / #19 / #27** (+ docx **#23**); tip readiness **READY** for xlsx/docx/pptx with documented known-loss exceptions still printed as LOSS; Jared flips workflow env (agents do not)

---

## Appendix: docx peak_mem compile-floor attribution (2026-10-21)

Companion to the xlsx S attribution. Measured on main `fd11a8c` and [#359](https://github.com/jgridifier/nitro-xml/pull/359) tip `165a1e7` (now merged). Later drawing-outline cut: see `docs/perf-rust-hotpaths.md` slice 10.

| Tier | Cold first open Δ MiB (main → #359) | Second open Δ MiB | TS peak MiB (gate) |
|---|--:|--:|--:|
| S | 15.1 → **13.3** | 0.3–0.5 | ~2.5 |
| M | 16.7 → **15.7** | 0.3–0.4 | ~6.2 |
| L | 25.2 → 25.1 | 6.5–7.1 | ~21.6 |

`initSync` adds ~17.8 MiB before the gate baseline. After warmup, S/M document alloc already beats TS;
the LOSS rows are first-touch compile. [#359](https://github.com/jgridifier/nitro-xml/pull/359) is the
application cut that measurably helps; further docx-heap edits did not clear noise (see
`docs/perf-rust-hotpaths.md` slice 7).
