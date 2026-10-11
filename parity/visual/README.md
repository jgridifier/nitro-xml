<!-- SPDX-License-Identifier: MIT -->
# Visual gate (#45)

Pixelmatch harness: upstream TS oracle vs Rust at **DPR 1 and 2**, plus **print-to-PDF** (PDF emitted, page 1 rasterized, then pixelmatched). Thresholds from `docs/PRODUCT_SPEC.md` §5.

## What is covered today

| Registration | Source | Notes |
|---|---|---|
| `xlsx-view-ir` | CellItem view-IR (#18) painted by a shared canvas painter | TS dump vs Rust dump |
| `xlsx-viewer` | Live `<nitro-xlsx>` vs upstream `<xlsx-editor>` | Same fixture URL; **informational** (~3.0% dpr1 on `poi-45544` after upstream `GRID_CSS` + `.xg-*` classes in the loader/painters — still dominated by WASM `estimateWidth` vs TS `measureText` and incomplete multi-frame measure batch; minor border/CF gaps). Sheet tabs/status are #29. |
| `chart-svg` | Chart S1 SVG (#28) hosted in a fixed viewport | TS `chart_ref` vs Rust `chart_render_svg` |
| `docx` / `pptx` | — | `ready: false` — add a registry row when hosted |

The headless static server + Playwright runner is the minimal host slice of #6 (no perf metrics). See the note on issue #6.

`xlsx-viewer` prepare builds upstream `ooxml-core` + `ooxml-ui` when their `dist/` is missing, then `bun build`s `hosts/xlsx-editor-entry.mjs` into `out/bundles/xlsx-editor.js`. Nitro side needs `bash scripts/build-wasm.sh` (already run by `scripts/ci/parity.sh`).

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

1. Add a `ready: true` row in `registry.mjs` with `kind`, fixture, viewport size.
2. Teach `prepare.mjs` how to emit `out/payloads/{id}-{ts,rust}.*`.
3. Extend `pages/host.html` if a new `kind` needs a different host mount.
