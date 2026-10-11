<!-- SPDX-License-Identifier: MIT -->
# Visual gate (#45)

Pixelmatch harness: upstream TS oracle vs Rust at **DPR 1 and 2**, plus **print-to-PDF** (PDF emitted, page 1 rasterized, then pixelmatched). Thresholds from `docs/PRODUCT_SPEC.md` §5.

## What is covered today

| Registration | Source | Notes |
|---|---|---|
| `xlsx-view-ir` | CellItem view-IR (#18) painted by a shared canvas painter | TS dump vs Rust dump |
| `xlsx-viewer` | Live `<nitro-xlsx>` vs upstream `<xlsx-editor>` | Same fixture; **hard** ≤0.1% (0.0000% dpr1/dpr2/print after #150). Grid painters + `GRID_CSS` (visual-only) + Liberation Sans `#121` mirror + lightTheme tokens. Shell chrome hidden; host flex for `#149` `.chrome` hide. |
| `chart-svg` | Chart S1 SVG (#28) hosted in a fixed viewport | TS `chart_ref` vs Rust `chart_render_svg` |
| `pptx-viewer` | Live `<nitro-pptx>` vs upstream `renderSlideStage` | Corpus from `pptx-corpus.mjs` (S/M redistributable); **≤0.1% hard** for allow-listed fixtures (`PPTX_HARD_FIXTURE_IDS`), rest informational; Liberation `#121` mirror; DPR 1/2; print skipped until painters converge |
| `docx` | — | `ready: false` — add a registry row when hosted |

The headless static server + Playwright runner is the minimal host slice of #6 (no perf metrics). See the note on issue #6.

`xlsx-viewer` / `pptx-viewer` prepare builds upstream `ooxml-core` + `ooxml-ui` when their `dist/` is missing, then `bun build`s the host entry into `out/bundles/`. Nitro side needs `bash scripts/build-wasm.sh` (already run by `scripts/ci/parity.sh`).

Cap the pptx corpus locally with `VISUAL_PPTX_MAX=N` (unset = full S/M pool).

## Local run

```bash
cd parity/visual
bun install
bunx playwright install chromium
bun run prepare-payloads
bun run visual
```

Or via CI script (builds release examples, installs Playwright Chromium if needed):

```bash
bash scripts/ci/parity.sh
```

Outputs land in `parity/visual/out/` (`shots/`, `diffs/`, `pdfs/`, `summary.txt`).

## Adding a format

1. Add a `ready: true` row in `registry.mjs` with `kind`, fixture, viewport size (pptx corpus rows come from `pptx-corpus.mjs`).
2. Teach `prepare.mjs` how to emit `out/payloads/{id}-{ts,rust}.*` (or live-serve a fixture for viewer kinds).
3. Extend `pages/host.html` or add a `pages/*-viewer.html` if a new `kind` needs a different host mount.
