<!-- SPDX-License-Identifier: MIT -->
# Provenance: upstream glyph-width tables (`font-advance-widths*.generated.ts`) (#98)

Status: recorded from the upstream repo and its history at the pin. **Not ported yet**, so `PROVENANCE.md` gets its row when a renderer issue ports them. This is non-blocking: renderer issues (#18, #22, #23, #27) do not wait for this record or for upstream's answer.

## What the tables are

Upstream pin `ChristopherVR/ooxml@e66aaec24adc0b68472eae1fc42aa4598e09c352`, directory `src/core/text/font-metrics/`:

| File | Font | Method (see below) | Pair kerning |
|---|---|---|---|
| `font-advance-widths.generated.ts` | barrel: `FontAdvanceTable` type, lookup, re-exports | n/a | n/a |
| `font-advance-widths-calibri.generated.ts` | Calibri | AutoSize `Shape.Width` difference (older) | no |
| `font-advance-widths-calibri-light.generated.ts` | Calibri Light | older | no |
| `font-advance-widths-arial.generated.ts` | Arial | older | no |
| `font-advance-widths-times-new-roman.generated.ts` | Times New Roman | older | no |
| `font-advance-widths-segoe-ui.generated.ts` | Segoe UI | older | no |
| `font-advance-widths-aptos-display.generated.ts` | Aptos Display | older | no |
| `font-advance-widths-aptos.generated.ts` | Aptos | `TextRange.BoundWidth` between kerning-neutral anchors (current) | yes |

Each per-font table holds numbers only:
- the per-1000-em advance of the printable ASCII code points 32 to 126;
- their average;
- PowerPoint's default `a:bodyPr` text-frame insets for that font, in points;
- a line-height ratio;
- for Aptos, the non-zero pair-kerning values.

No glyph outlines, hinting or other font software is included. Upstream `NOTICE` says: "numeric widths measured through COM automation; no font software is included". Upstream `PROVENANCE.md` (text area row, and the pptx row's notes) records them as "glyph advance measurements taken from Microsoft PowerPoint through COM automation (numeric facts, no font data is embedded)", moved unchanged from pptx-viewer.

Upstream users of the tables: `src/core/text/font-metrics/font-kerning.ts`, the SmartArt/diagram layout (`src/core/diagram/layout/smartart-*.ts`, `src/core/diagram/engine/text-measure.ts`) and the pptx SVG exporter (`src/core/pptx/converter/svg-text-lines.ts`).

## How they were produced

The generator is `src/core/scripts/pptx/make-font-advance-table.ps1` (PowerShell, 374 lines, Windows only; it needs a local PowerPoint install). It drives `PowerPoint.Application` over COM:
1. It creates a blank slide with a reused non-wrapping text box at a 100 pt reference size.
2. **Current method.** Each glyph's advance is `BoundWidth("H"+g+"H") - BoundWidth("HH")`, using `TextFrame2.TextRange.BoundWidth`. Pair kerning is `BoundWidth("H"+a+b+"H") - BoundWidth("HH") - adv(a) - adv(b)`, keeping the non-zero pairs.
3. **Older method.** It differenced `Shape.Width` of an AutoSize box. Upstream measured this as reading Aptos about 1.1% wide.
4. Values are snapped to PowerPoint's 1/8 pt grid and stored per 1000 em.

Defaults are `-Fonts Calibri, 'Calibri Light', Arial, 'Times New Roman', 'Segoe UI'`. Aptos and Aptos Display were measured with explicit `-Fonts`; Aptos was regenerated with `-Fonts Aptos -SkipBarrel`.

**Not recorded anywhere upstream:**
- the PowerPoint version or build;
- the Windows version;
- the installed font file versions (Calibri, Aptos, Segoe UI and the rest ship with Windows or Office and change between releases).

That gap is the main open question for upstream.

## History (commit links)

Origin repo `ChristopherVR/pptx-viewer`, path `packages/core/src/core/utils/font-advance-widths*.generated.ts`, generator `scripts/make-font-advance-table.ps1`:
- [d1b43a5](https://github.com/ChristopherVR/pptx-viewer/commit/d1b43a5203b499ac1b18c7da58a3d4a7c38ba640), 2026-09-09: first added with the COM-authored SmartArt ground-truth corpus ("a COM-measured line-height ratio and advance tables (Aptos is the gallery's minor font)").
- [de5c891](https://github.com/ChristopherVR/pptx-viewer/commit/de5c8911eb), 2026-09-10: docs only.
- [9603333](https://github.com/ChristopherVR/pptx-viewer/commit/960333342c7676a35b9c9e845017615fdd8a0b2a), 2026-09-25: method changed to kerned `BoundWidth`, and Aptos was regenerated with pair kerning. The other fonts were not regenerated, so they keep the older method.
- [caa5172](https://github.com/ChristopherVR/pptx-viewer/commit/caa517204c), 2026-10-01: pptx-viewer core became a thin entry over `@christophervr/ooxml-core`; the tables left that repo.

In `ChristopherVR/ooxml`:
- [d5b8d6d](https://github.com/ChristopherVR/ooxml/commit/d5b8d6d911), 2026-10-06: generator under `src/core/scripts/pptx/`.
- [91767c0](https://github.com/ChristopherVR/ooxml/commit/91767c03d4), 2026-10-08: tables at `src/core/text/font-metrics/` when the diagram engine moved out of pptx.

Commit trailers show the work was AI-assisted (Claude Code sessions). The authorship and licence are still ChristopherVR's, under Apache-2.0.

## Licence assessment

- The files are part of an Apache-2.0 repository, and the copyright holder's NOTICE and PROVENANCE describe them explicitly. That is a clear grant for the files as published.
- The content is numeric measurements of rendered text taken through the authors' own PowerPoint install. It contains no font program or outline data, so the font EULAs (Microsoft for Calibri, Aptos, Segoe UI, Arial and Times New Roman) cover the fonts, not width facts read off a layout engine. The residual risk is low, and it is the same for every metric-compatible font project.
- The data cannot be reproduced on our box. The generator needs Windows plus PowerPoint over COM, and we have neither in CI.

## Recommendation

**Use as-is, with Apache-2.0 provenance**, when a renderer issue ports them:
- Port the numbers into a generated Rust table, preferably generated from the upstream `.ts` files by a script so it cannot drift.
- Add a `PROVENANCE.md` row naming the upstream path, pin and NOTICE text.
- Carry upstream's NOTICE sentence into ours.
- Mark the crate `MIT AND Apache-2.0`, as for any ported file.

Regenerating is not possible without Windows and PowerPoint, and would only reproduce the same facts. Dropping them would break the "fonts and measurement follow upstream" rule (R0) and SmartArt fit parity. Revisit only if upstream's answer below says otherwise. In that case, replace the tables with measurements from metric-compatible open fonts (Carlito, Liberation, and so on). That work is post-port (#92-#94) and changes no renderer issue's dependencies.

## Question to upstream

**Not opened yet.** Opening an issue or discussion on someone else's repository is an external action that a person on the team should take or approve. The draft below is for a GitHub Discussion (upstream has discussions enabled) or an issue on `ChristopherVR/ooxml`. Record the link here once it is opened.

> **Title:** Provenance of the font-advance-widths*.generated.ts tables
>
> Hi! We're building a Rust/WASM port of ooxml (nitro-xml, Apache-2.0 provenance kept for ported files) and would like to reuse the PowerPoint glyph-advance tables in `src/core/text/font-metrics/`. Two short questions:
>
> 1. Do you have a record of the PowerPoint build and Windows/font versions they were measured with (make-font-advance-table.ps1 doesn't log them)? Only Aptos was re-measured with the BoundWidth + kerning method in pptx-viewer 9603333; are the others due for the same?
> 2. Are you happy for the tables to be redistributed under Apache-2.0 with your NOTICE text, as the rest of the repo is?
>
> Thanks for the project and the careful provenance notes.

Link: _(pending)_
