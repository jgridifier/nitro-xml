<!-- SPDX-License-Identifier: MIT -->
# React embed latency bench

Minimal **Vite + React 18** host that mounts Nitro in `mode="embedded"` and
measures startup / first-paint / edit-to-repaint latency. Used as the before
baseline for the React embedding lag work (streaming compile, prewarm, module
cache, worker reuse, rAF edit coalesce, clean unmount). See
`docs/react-embedding.md` once published.

## Setup

```bash
# once, from repo root (wasm package must exist)
bash scripts/build-wasm.sh   # if packages/nitro-xml/pkg is missing

cd perf/react-embed
npm install
```

## Run

```bash
npm run bench:smoke   # startup + xlsx S first-paint
npm run bench         # full suite → results/latest.json
```

Or manually:

```bash
npx vite --port 5179
# then open http://127.0.0.1:5179/?scenario=first-paint&format=xlsx&tier=S
```

## Scenarios (`?scenario=`)

| Scenario | What it measures |
|---|---|
| `startup` | WASM fetch / compile / instantiate, worker construct, `initCore` |
| `first-paint` | mount → open → first paint (`format`, `tier`) |
| `edit-cell` | xlsx `setCellInput` → tick p50/p95 |
| `edit-docx-text` | docx `setParagraphText` → tick |
| `edit-pptx-text` | pptx `setShapeText` → tick |
| `edit-bold` | `shellAction("cmd","home.bold")` → tick |
| `remount` | unmount + remount same file |
| `strict-double-mount` | React 18 StrictMode double mount |

## Fixtures

S/M paths in `fixtures.json` (symlinked under `public/`). No corpus changes.

## Constraints

Measurement only — does not change loader budgets, thresholds, or grid paint.
