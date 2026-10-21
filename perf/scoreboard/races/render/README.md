<!-- SPDX-License-Identifier: MIT -->
# Scoreboard race: render-to-first-paint (#41)

Times **load + init + first paint** (including wasm fetch/compile/instantiate for Nitro)
per format × size tier. An entrant is ranked only when its first painted viewport is
within the PRODUCT_SPEC §5 pixel limit vs the upstream TS reference; otherwise it is
**not comparable**.

## Scope

| Format | Status |
|--------|--------|
| **xlsx** | Entered: `upstream-ts` (reference), `nitro-xml` |
| **pptx** | Entered: `upstream-ts` (reference), `nitro-xml` |
| **docx** | Deferred — TODO(#23) |

## Fixtures (tiers)

Same generated corpus as `perf/large/ts-vs-wasm.mjs`:

| Tier | xlsx | pptx |
|------|------|------|
| S | `fixtures/generated/xlsx/cells_1k.xlsx` | `slides_10.pptx` |
| M | `cells_100k.xlsx` | `slides_50.pptx` |
| L | `cells_1m.xlsx` | `slides_200.pptx` |
| XL | `xl_a.xlsx` | `xl_a.pptx` |

Viewport: 800×480 CSS px, DPR 1 (matches visual host defaults).

## Reproduce

```bash
# once: wasm pkg + upstream oracle bundles used by the visual host
bash scripts/build-wasm.sh
# upstream build if needed (same as parity/visual prepare):
#   cd upstream && bun install && bun run build
#   cd parity/visual && bun ./prepare.mjs

cd perf/scoreboard && npm ci
node races/render/run.mjs --format=xlsx,pptx --tier=S
# writes results/render/<date>-<sha>.json and docs/scoreboard/data/first-paint-latest.json
```

Options: `--runs=7 --warmup=2 --seed=20261011 --format=xlsx --tier=S,M --entrants=nitro-xml,upstream-ts --chrome=/usr/bin/google-chrome`.

## Fairness

Same machine/browser rules as the parse race (`docs/scoreboard-method.md`). Timer covers
module/wasm load through the first frame that puts grid DOM under the host root (not
deferred SeekTail completion — that is visual-parity only).
