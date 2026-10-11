<!-- SPDX-License-Identifier: MIT -->
# Visual gate (#45)

Pixelmatch harness: upstream TS oracle vs Rust at **DPR 1 and 2**, plus **print-to-PDF** (PDF emitted, page 1 rasterized, then pixelmatched). Thresholds from `docs/PRODUCT_SPEC.md` §5.

## What is covered today

| Registration | Source | Notes |
|---|---|---|
| `xlsx-view-ir` | CellItem view-IR (#18) painted by a shared canvas painter | TS dump vs Rust dump |
| `chart-svg` | Chart S1 SVG (#28) hosted in a fixed viewport | TS `chart_ref` vs Rust `chart_render_svg` |
| `docx` / `pptx` | — | `ready: false` — add a registry row when hosted |

The headless static server + Playwright runner is the minimal host slice of #6 (no perf metrics). See the note on issue #6.

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
