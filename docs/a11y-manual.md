<!-- SPDX-License-Identifier: MIT -->
# Accessibility manual spot-check (NVDA / VoiceOver)

Automated coverage for Stage 3 lives in `tests/a11y/` and `scripts/ci/a11y.sh` (#42):
differential axe-core (zero new serious/critical vs upstream TS), CDP
`Accessibility.getFullAXTree` role/name diff, and keyboard reachability of the
Stage-6 shell controls. This checklist is the remaining human pass per stage.

## When to run

- After the automated `a11y` gate goes green on a format’s viewer stage.
- Before declaring a stage done in PRODUCT_SPEC §6 (Stages 3/4/5 viewers; Stage 6 shell).

## Formats

| Format | Automated now | Manual when |
|--------|---------------|-------------|
| xlsx   | yes (#42)     | Stage 3 merge |
| pptx   | yes (#42)     | Stage 5 merge |
| docx   | deferred (#158) | After Print Layout + `nitro-docx` land |

## NVDA (Windows) — viewer smoke

1. Open the format’s demo/host with a real fixture (same as the a11y registry).
2. Confirm the content surface is announced:
   - **xlsx:** “Spreadsheet grid” (or equivalent) with row/column headers as you arrow.
   - **pptx:** slide region (“Slide”) and visible text in reading order.
3. Tab through Stage-6 chrome: Zoom out, Zoom in, Find, Copy, Diagnostics, sheet/slide tabs.
4. Open Find (`Ctrl/Cmd+F` or Find), type a known string, Next/Previous, Escape closes.
5. No surprise “blank” focus stops on decorative paint nodes.

## VoiceOver (macOS) — viewer smoke

1. VO + A reads the content surface without looping on chrome.
2. VO + Command + Y (or rotor → Form Controls) lists shell buttons/tabs with names.
3. Grid/slide roles match the automated AX snapshot roles for that fixture.

## Editor surface (later)

Cell-editor focus sink, formula bar, and IME/caret a11y are E-text (#55+), not this gate.
Allow-listed in `tests/a11y/allowlist.mjs` until those issues land.

## Recording a spot-check

Note date, OS/AT versions, fixture id, and pass/fail in the stage handoff comment
(or attach to the closing PR for that format’s viewer stage).
