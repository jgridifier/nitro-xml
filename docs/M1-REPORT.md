<!-- SPDX-License-Identifier: MIT -->
# M1 report — Stage 7 gate (#30)

Status: **draft for review** (2026-10-19). Milestone 1 viewer gate record for Nitro XML.
Does not close #23 (scoreboard / races stay open). Perf optimization PRs from the
speed/memory workstream are in flight and will refresh the baseline section.

Upstream pin: ChristopherVR/ooxml @ `e66aaec`. Shipped surface: Rust/WASM only; TS is
test-oracle / pixel reference / perf baseline and never ships.

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

Unsupported-content rate at the format stage gates: **0** (any unsupported kind
counts as a failing parity item).

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

## 5. Performance baseline (large fixtures)

**Source:** [PR #275](https://github.com/jgridifier/nitro-xml/pull/275) (`perf/large/` harness).
This table is the **M1 baseline**. Optimization PRs from the parallel speed/memory
workstream are in flight and will update numbers here; do not treat these as the
final M1 perf claim until that workstream lands.

Host: 2026-10-19, release native, fresh process per fixture. Headline fixture
**`cells_1m.xlsx`** (~4.9 MiB / 1M cells):

| metric | value |
|--------|------:|
| `open_first_view` median | 170.8 ms |
| `open_first_view` RSS HWM | 15.7 MiB |
| scroll `ensure_viewport` | 176 ms |
| `load_workbook` full median | 2279 ms |
| `load_workbook` RSS HWM | ~1.90 GiB |
| WASM `openDocument` median | 241 ms |

Reproduce (fixtures generated on demand, not committed):

```bash
python3 fixtures/gen/generate.py --id gen-xlsx-cells-100k --id gen-xlsx-cells-1m \
  --id gen-docx-pages-500 --id gen-pptx-slides-200
cargo run --release --manifest-path perf/large/Cargo.toml -- \
  --xlsx fixtures/generated/xlsx/cells_1m.xlsx --runs 7
bash scripts/build-wasm.sh
node perf/large/wasm-baseline.mjs fixtures/generated/xlsx/cells_1m.xlsx 5
```

Per-format / per-tier TS-vs-WASM medians and bootstrap CIs for the full scoreboard
remain tracked under **#23** (left open).

---

## 6. Bundle / loader

| Budget | Ceiling | Notes |
|--------|--------:|-------|
| Loader JS (hand-written + wasm-pack glue), brotli q=11 | **17 KB** | `perf/budgets.json`; `loader/lazy/` excluded (WordArt outlines, edit journal) |
| Per-format wasm payload + TTI | TBD | TODO(#46) |

---

## 7. Open follow-ups

| Item | Issue / PR | Notes |
|------|------------|-------|
| Scoreboard races / full TS-vs-WASM CI tables | **#23** (open) | Not closed by this report |
| Perf optimization vs §5 baseline | speed/memory workstream | Updates §5 of this doc |
| Keep `crash-recovery` in branch-protection required checks | Jared / Chief of Staff | Agents do not edit branch protection |
| Per-format wasm / TTI ceilings | #46 | Bundle gate extension |

---

## 8. Definition of done (Stage 7 checklist)

- [x] Fuzzing clean (1 CPU-hour / target, 0 crashes)
- [x] Crash recovery tested (viewer + editor; CI injection; flake loop)
- [x] Memory / package ceiling enforced with graceful viewer errors
- [x] M1 report published (this document)
- [ ] Scoreboard standings including losses (#23 — separate)
- [ ] Perf baseline refreshed after optimization PRs land
