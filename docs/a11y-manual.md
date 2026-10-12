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
| docx   | yes (#42 / #23) | Stage 4 merge |

## NVDA (Windows) — viewer smoke

1. Open the format’s demo/host with a real fixture (same as the a11y registry).
2. Confirm the content surface is announced:
   - **xlsx:** “Spreadsheet grid” (or equivalent) with row/column headers as you arrow.
   - **pptx:** slide region (“Slide”) and visible text in reading order.
   - **docx:** paginated print pages; body text in visual reading order.
3. Tab through Stage-6 chrome: Zoom out, Zoom in, Find, Copy, Diagnostics, sheet/slide tabs.
4. Open Find (`Ctrl/Cmd+F` or Find), type a known string, Next/Previous, Escape closes.
5. No surprise “blank” focus stops on decorative paint nodes.

## VoiceOver (macOS) — viewer smoke

1. VO + A reads the content surface without looping on chrome.
2. VO + Command + Y (or rotor → Form Controls) lists shell buttons/tabs with names.
3. Grid/slide roles match the automated AX snapshot roles for that fixture.

## Editor surface — NVDA / VoiceOver (#59)

Automated edit coverage: `tests/a11y/edit/` (axe + CDP AX-tree + live region), wired
into `scripts/ci/a11y.sh` after the viewer gate (#42). Host paint is still #69; the
harness mounts upstream-shaped formula fields and Rust `a11yEval` projections.

1. Open an xlsx fixture in **edit** mode (not read-only viewer).
2. Tab to the **Formula Bar** or in-cell editor; NVDA should report a textbox with that name.
3. Type `=A1`; the polite status (`.xg-live`) should announce the value (same string as automated gate).
4. Shift+arrow to extend selection; status should mention the selected characters.
5. **Negative:** read-only viewer — no formula-bar textbox in the rotor; no edit announcements.

Record OS, AT version, and fixture id with pass/fail in the #59 handoff when you run this manually.

## Recording a spot-check

Note date, OS/AT versions, fixture id, and pass/fail in the stage handoff comment
(or attach to the closing PR for that format’s viewer stage).
