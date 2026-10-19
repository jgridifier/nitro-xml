<!-- SPDX-License-Identifier: MIT -->
# M1 report — Stage 7 gate (#30)

Status: **metrics refresh** (2026-10-19). Milestone 1 viewer gate record for Nitro XML.
Measured on tip **`164ac7d40f72d68d549bd8218e955a76584e9c7c`** (includes #279 embedded pptx
and #283 O(viewport) xlsx first paint). Does **not** close #23 (docx perf-gate arm
and scoreboard / races stay open). Does **not** arm any gate.

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
| Editor: mid-edit kill → restore unsaved edits + undo stack (xlsx / docx / pptx); graceful UI | **Met** | Op journal (`loader/lazy/journal.js`); `recoverEditOnce`; `EDITOR_RECOVERY_*` messages; harness in `tests/crash/run.mjs` (#276 / #277) |
| CI `crash-recovery` job gates on every PR (no path filters) | **Met** | `.github/workflows/ci.yml` job has no `paths:` / `paths-ignore:`; default **10** deterministic flake iterations (`NITRO_CRASH_FLAKE_RUNS`) |
| Stub `NOT IMPLEMENTED` removed from `scripts/ci/crash_recovery.sh` | **Met** | Job is live; ask Jared / Chief of Staff to keep it in branch-protection required checks |

Local flakiness proof (harness tip): **10/10** flake iterations × 3 formats = **30/30** happy-path+undo and **30/30** negative controls.

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
| **Commit** | `42b72b08c393cabc82431f3d82a5b9ea6004b9e7` |
| **When** | 2026-10-19T16:42:02Z |
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
```

| Format | Tier | TS median (ms) | WASM median (ms) | WASM/TS | 95% CI | Verdict |
|--------|------|---------------:|-----------------:|--------:|-------:|---------|
| xlsx | S | 4.65 | 8.33 | 1.792 | [1.435, 2.600] | **LOSS** |
| xlsx | M | 201.72 | 30.98 | 0.154 | [0.134, 0.165] | **WIN** |
| xlsx | L | 2141.16 | 240.09 | 0.112 | [0.108, 0.117] | **WIN** |
| xlsx | XL | 2072.91 | 245.87 | 0.119 | [0.117, 0.123] | **WIN** |
| docx | S | 2.78 | 1.32 | 0.474 | [0.436, 0.551] | **WIN** |
| docx | M | 6.88 | 3.52 | 0.512 | [0.434, 0.605] | **WIN** |
| docx | L | 26.10 | 13.37 | 0.512 | [0.499, 0.544] | **WIN** |
| docx | XL | 42.78 | 23.34 | 0.546 | [0.438, 0.565] | **WIN** |
| pptx | S | 13.19 | 1.13 | 0.086 | [0.070, 0.095] | **WIN** |
| pptx | M | 43.65 | 4.51 | 0.103 | [0.091, 0.123] | **WIN** |
| pptx | L | 179.74 | 22.49 | 0.125 | [0.106, 0.149] | **WIN** |
| pptx | XL | 194.36 | 33.92 | 0.175 | [0.152, 0.196] | **WIN** |

**Wins / losses summary:** WASM wins **11 / 12** cells. The only loss is **xlsx S**
(WASM/TS ≈ 1.79, CI entirely above 1). At M/L/XL xlsx, WASM is ~6–9× faster on this
metric; pptx is ~6–12×; docx is ~1.8–2.1×.

**Honest scope notes:**

- Metric is **cold open only** (not scroll / peak memory / browser first-paint including
  wasm fetch+compile+instantiate). Those remain for the armed gate under #8 once filled in.
- **xlsx asymmetry:** TS `loadWorkbook` is a full workbook parse; WASM `openDocument` uses
  `open_first_view` (viewport) + first `process_frame`. That is the viewer path (#283), not
  identical work — especially visible at L/XL.
- Gate scripts are **unchanged stubs** (report does not arm anything):

```text
$ bash scripts/ci/perf_beats_ts.sh xlsx   # NOT IMPLEMENTED — see #19 (#8)
$ bash scripts/ci/perf_beats_ts.sh docx   # NOT IMPLEMENTED — see #23 (#8)
$ bash scripts/ci/perf_beats_ts.sh pptx   # NOT IMPLEMENTED — see #27 (#8)
$ bash scripts/ci/perf.sh gate            # NOT IMPLEMENTED — see #8
```

**Docx perf-gate arm stays deferred under #23; this slice does not close #23.**

---

## 6. Bundle / loader

Measured on the same SHA after `bash scripts/build-wasm.sh`:

```bash
bash scripts/ci/bundle_budget.sh
```

| Artifact | Size | Ceiling / notes |
|----------|-----:|-----------------|
| Loader JS (hand-written + wasm-pack glue), brotli q=11 | **17274 B (16.87 KB)** | ≤ **17 KB** (`perf/budgets.json`); `loader/lazy/` excluded |
| `nitro_xml_wasm_bg.wasm` raw | 6058755 B (~5.78 MB) | Per-format payload ceilings TODO(#46) |
| `nitro_xml_wasm_bg.wasm` brotli q=11 | 1512324 B (~1.44 MB) | Informational only (not gated) |
| `nitro_xml_wasm.js` glue (uncompressed) | 30879 B | Included in loader brotli measure above |

`bundle_budget.sh` also prints: `TODO(#46): per-format initial-payload ceilings and time-to-interactive not yet gated`.

---

## 7. Scoreboard standings — **deferred**

Scoreboard standings (parse / render-to-first-paint / open-edit-save), including
losses as prominently as wins, **cannot be produced honestly on this tip**:

| Blocker | Link | Why |
|---------|------|-----|
| `scripts/ci/scoreboard.sh` | stub → `#39` | Still prints `NOT IMPLEMENTED — see #39` |
| Render-to-first-paint race | [#41](https://github.com/jgridifier/nitro-xml/issues/41) | Race not landed; labelled blocked |
| Scoreboard CI / reproducible gate | draft [#113](https://github.com/jgridifier/nitro-xml/pull/113) | Draft; **do not touch from this slice** |
| Nitro XML entrant | `docs/scoreboard/data/parse-latest.json` | Still `"status": "not entered"` (stale reason from #11–#13 era) |

No standings invented. When #41 / #113 land, refresh this section from the harness
output and list W/L columns without demoting losses.

---

## 8. Open follow-ups

| Item | Issue / PR | Notes |
|------|------------|-------|
| Arm WASM-beats-TS gates (CI) | **#8**, **#19**, **#27**; docx **#23** (deferred) | Report tables exist (§5.3); stubs still `NOT IMPLEMENTED` — do not arm from this report |
| Scoreboard standings including losses | **#41**, draft **#113**, **#39** | Link only; do not invent numbers |
| Keep `crash-recovery` in branch-protection required checks | Jared / Chief of Staff | Agents do not edit branch protection |
| Per-format wasm / TTI ceilings | #46 | Bundle gate extension |

---

## 9. Definition of done (Stage 7 checklist)

- [x] Fuzzing clean (1 CPU-hour / target, 0 crashes)
- [x] Crash recovery tested (viewer + editor; CI injection; flake loop)
- [x] Memory / package ceiling enforced with graceful viewer errors
- [x] M1 report published with **real measured** numbers from existing harnesses (native/WASM/bundle on `164ac7d`; TS-vs-WASM cold open on `42b72b0`)
- [x] Unsupported-content rate recorded (**0** × 3 formats)
- [x] Bundle / loader sizes recorded (loader **16.87 / 17 KB**; wasm sizes above)
- [x] Per-format×tier **TS vs WASM** cold-open medians + 95% bootstrap CIs recorded (§5.3), **including the xlsx S loss** — gate scripts still stubbed; **#23 left open** (docx arm deferred)
- [ ] Scoreboard standings including losses — **blocked on #41 / draft #113** (link only; no invented numbers)
- [ ] `crash-recovery` added to branch-protection required checks — **Jared / Chief of Staff only**
- [ ] Arm `perf-beats-ts` CI gates — **#8 / #19 / #27**; docx **#23** deferred
