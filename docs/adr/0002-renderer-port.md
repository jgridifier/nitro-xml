<!-- SPDX-License-Identifier: MIT -->
# ADR 0002: Renderer port — Rust module mapping (R0 (b), #35)

**Status:** Accepted for module layout (R0 record #33 still needs Head of Product sign-off after #35–#37 land).  
**Date:** 2026-10-11  
**Depends on:** painter inventory (#34, merged).  
**Decisions recorded from:** PRODUCT_SPEC.md §6; Head of Product review of R0 on #33 (2026-10-11); final inventory in `docs/painters.md`.

## Context

R0 ports upstream painters one to one (same drawing approach, structure, fonts and measurement). The only allowed change is that each frame's draw and DOM-building calls cross the WASM boundary as one command buffer applied by the loader. This ADR turns the inventory in `docs/painters.md` into the Rust module layout that renderer issues (#18, #23, #27, #28, #29) fill. Module names and responsibilities only; no painter code in this change.

Inventory already records (from #34 / HoP review):

- `image-native-size.ts` in **P5** (`a:tile` sx/sy).
- Stylesheets: `print.css` → D1–D7; `document.css` + Draft helpers → D8 (E3); `smartart.css` → D11; `styles/index.ts` is `X:chrome`.
- Draft **D8–D11** under **E3**, not M1. M1 Word is Print Layout D1–D7 plus D4.
- **P24** deferred (Jared's call). Google Fonts fetch (P3/P4) ported unchanged until Jared decides.
- Deferred: X7, X10, P10, P12, P13, P25–P27; P17 M1 is poster only.

This mapping also assigns (HoP #35):

- `render/text-warp.ts` → **P4** (WordArt), not P3.
- `media-file-type.ts` → **`X:editor`** (insert MIME classifier), not P14.

## Decision: crate layout

| Crate | Path | Role |
|---|---|---|
| `nitro-xml-render` | `rs/crates/nitro-xml-render/` | **Shared render crate.** Command-buffer types and writer; shared drawing primitives (fills, lines, text runs, images); shared painters **S1** (chart SVG) and **S2** (SmartArt cached drawing). |
| `nitro-xml-xlsx` | `rs/crates/nitro-xml-xlsx/src/render/` | Excel grid painters (X*). |
| `nitro-xml-docx` | `rs/crates/nitro-xml-docx/src/render/` | Word Print Layout painters (D1–D7) and E3 Draft modules (D8–D11). |
| `nitro-xml-pptx` | `rs/crates/nitro-xml-pptx/src/render/` | PowerPoint slide painters (P*). |
| `nitro-xml-opc` | `rs/crates/nitro-xml-opc/src/drawingml/` | DrawingML **model** (colour, fill, geometry, theme readers). **Not** chart or SmartArt painters. |
| `nitro-xml-wasm` | `rs/crates/nitro-xml-wasm/src/` | WASM boundary; applies the per-frame command buffer (loader API #37; worker/main #36). |

**Why not `nitro-xml-opc` for S1/S2.** OPC owns package and DrawingML model. Chart SVG and SmartArt cached-drawing painters are renderers shared by formats; they share the command-buffer types. Head of Product: S1 and S2 live in the shared render crate.

Format crates depend on `nitro-xml-render` for the buffer writer, primitives and S1/S2. Chart/SmartArt **adapters** (X9, X12, D4, D11) stay in the format crate and call into `nitro-xml-render`.

## Shared modules (`nitro-xml-render`)

Paths are under `rs/crates/nitro-xml-render/src/`.

| Module | Path | Responsibility |
|---|---|---|
| Command buffer | `command_buffer/` (`mod.rs`, `writer.rs`, `ops.rs`) | One handoff per frame: typed draw/DOM ops the loader applies. Editor chrome uses the same buffer (HoP). Batched measurement read-back is in #37. |
| Primitives | `primitives/` (`fill.rs`, `line.rs`, `text_run.rs`, `image.rs`) | Fills, lines, text runs and images reused by format painters; emit buffer ops, never call JS per element. |
| S1 Chart SVG | `chart/` | Port of `src/core/chart/render/` (`renderChartSvg`, `renderChartSpaceSvg`). Used by X9, D4 and P27. |
| S2 SmartArt | `smartart/` | Port of shared `<office-ui-smartart>` cached-drawing painter. Used by X12 and D11. Includes `smartart.css` styling contract for docx hosts. |
| Measure bridge | `measure/` | Types for the browser measurement path (canvas `measureText`, DOM span probes) behind the buffer; worker/main split in #36. |

## xlsx modules (`nitro-xml-xlsx::render`)

Paths under `rs/crates/nitro-xml-xlsx/src/render/`.

| Row | Module path | M1? | Responsibility |
|---|---|---|---|
| X1 | `grid.rs` | yes (#18) | Grid frame, quadrants, scroll extent, freeze lines. |
| X2 | `cells.rs` | yes (#18) | Cell layer: fills, text, borders, merges, comments, errors. |
| X3 | `cond_format.rs` | yes (#18) | Data bars and icon sets. |
| X4 | `gridlines.rs` | yes (#18) | Per-quadrant gridline divs. |
| X5 | `headers.rs` | yes (#18) | Row/column headers and pane chrome. |
| X6 | `selection.rs` | yes (#18) | Selection overlay. |
| X7 | `remote_selection.rs` | **deferred** | Remote collaborator outlines. |
| X8 | `drawings.rs` | yes (#18) | Picture/object frames; hosts X9 and X12. |
| X9 | `chart.rs` | via #28 | Adapter onto S1. |
| X10 | `print.rs` | **deferred** | Print HTML table. |
| X11 | `sparkline.rs` | yes (#18; split if needed) | Sparklines. |
| X12 | `smartart.rs` | via S2 | Adapter onto S2. |
| X13 | `sheet_tabs.rs` | yes (#18) | Sheet tab strip. |

## docx modules (`nitro-xml-docx::render`)

Paths under `rs/crates/nitro-xml-docx/src/render/`.

### M1 — Print Layout (#23)

| Row | Module path | Responsibility |
|---|---|---|
| D1 | `print/pages.rs` | Paginated pages, fragments, measurer. Stylesheet: `print.css` (styles all of D1–D7). |
| D2 | `print/table.rs` | Print Layout tables. |
| D3 | `print/picture.rs` | Print Layout pictures; hosts D4. |
| D4 | `chart.rs` | Adapter onto S1 (Print Layout and Draft hosts). |
| D5 | `print/header_footer.rs` | Headers, footers, page fields. |
| D6 | `print/page_borders.rs` | Section page borders. |
| D7 | `print/line_numbers.rs` | Line-number labels. |
| — | `print/styles.rs` | Port of `print.css`. |

### E3 — Draft surface (not M1)

| Row | Module path | Responsibility |
|---|---|---|
| D8 | `draft/surface.rs` | Continuous Draft surface. Stylesheet: `document.css`. Helpers: `review-*`, `preview-run-styles`, `scaled-paragraph-breaks`, `tab-stops-view`. |
| D9 | `draft/picture.rs` | Draft pictures; dispatches D4 and D11. |
| D10 | `draft/equation.rs` | OMML → MathML (Draft only). |
| D11 | `draft/smartart.rs` | Adapter onto S2; `smartart.css`. |
| — | `draft/styles.rs` | Port of `document.css`. |

## pptx modules (`nitro-xml-pptx::render`)

Paths under `rs/crates/nitro-xml-pptx/src/render/`.

| Row | Module path | M1? | Responsibility |
|---|---|---|---|
| P1 | `stage.rs` | yes (#27) | Slide stage, background, registry, hyperlinks. JS lib: DOMPurify → Rust allowlist sanitizer. |
| P2 | `shape.rs` | yes (#27) | Geometry, fill, line, effects; text body via P3. |
| P3 | `text.rs` | yes (#27) | Text body, bullets, autofit, East Asian. **Google Fonts CSS2 fetch ported unchanged** until Jared decides. |
| P4 | `wordart.rs` | yes (#27) | WordArt warps. Includes `dom/elements/text-warp.ts` and `render/text-warp.ts`. JS lib: opentype.js → Rust font outline parse (same glyph path). |
| P5 | `picture.rs` | yes (#27) | Pictures, crop, effects, `image-native-size.ts`. EMF/WMF via `emf-converter` (follow-up). |
| P6 | `group.rs` | yes (#27) | Group transforms and children. |
| P7 | `connector.rs` | yes (#27) | Connectors. |
| P8 | `table.rs` | yes (#27) | Tables. |
| P9 | `chart.rs` | via #28 | pptx's own 2D chart painter (kept separate from S1 per 1:1 rule). |
| P10 | `chart3d.rs` | **deferred** | Opt-in WebGL 3D charts; three.js leaves with this row. |
| P11 | `smartart.rs` | yes (#27) | pptx SmartArt (separate from S2). |
| P12 | `smartart3d.rs` | **deferred** | Opt-in WebGL SmartArt 3D. |
| P13 | `three_host.rs` | **deferred** | Shared three.js host. |
| P14 | `media.rs` | yes (#27) | Audio/video posters and playback. **`media-file-type.ts` excluded** (`X:editor`). |
| P15 | `ole.rs` | yes (#27) | OLE previews. |
| P16 | `activex.rs` | yes (#27) | ActiveX placeholders. |
| P17 | `model3d.rs` | yes (#27), poster only | Poster image; on-demand WebGL deferred with P13. |
| P18 | `ink.rs` | yes (#27) | Ink strokes. |
| P19 | `content_part.rs` | yes (#27) | Content parts. |
| P20 | `equation.rs` | yes (#27) | MathML equations. |
| P21 | `zoom.rs` | yes (#27) | Slide/section zoom. |
| P22 | `placeholder.rs` | yes (#27) | Unknown-element placeholder. |
| P23 | `comments.rs` | yes (#27) | Comment markers. |
| P24 | `slideshow.rs` | **deferred** | Animations, transitions, Morph. |
| P25 | `print.rs` | **deferred** | Print, notes, handouts. |
| P26 | `raster_export.rs` | **deferred** | Raster export. |
| P27 | `svg_export.rs` | **deferred** | DOM-free SVG exporter (uses S1). |

## Renderer issue → painters and modules

### #18 — xlsx grid painter

| Painters | Modules |
|---|---|
| X1–X6, X8, X11, X13 | `nitro-xml-xlsx::render::{grid,cells,cond_format,gridlines,headers,selection,drawings,sparkline,sheet_tabs}` |
| X9 (adapter) | lands with #28 → `chart` + `nitro-xml-render::chart` |
| X12 (adapter) | `smartart` + `nitro-xml-render::smartart` |
| Deferred | X7, X10 |

**Split if over one session:** core grid (X1–X6) first; drawings + sparklines + sheet tabs as a follow-up; charts already #28.

### #23 — docx Print Layout

| Painters | Modules |
|---|---|
| D1–D7, D4 adapter | `nitro-xml-docx::render::print::*`, `chart`, `print/styles` |
| Not in #23 | D8–D11 → E3 |

**Split if over one session:** D1 pages + styles + measurer first; D2 tables; D3/D5–D7 chrome; D4 with #28.

### #27 — pptx slide painters

| Painters | Modules |
|---|---|
| P1–P9, P11, P14–P23, P17 poster | see pptx table above |
| Via #28 | P9 (and S1 for any shared path) |
| Deferred | P10, P12, P13, P24, P25, P26, P27 |

**Split (Size L):** (1) stage + shapes + text + WordArt (P1–P4); (2) pictures, groups, connectors, tables (P5–P8); (3) charts + SmartArt (P9, P11) with #28; (4) media, OLE, ActiveX, ink, content parts, equations, zoom, placeholders, comments, model3d poster (P14–P23, P17).

### #28 — chart painters (shared)

| Painters | Modules |
|---|---|
| S1 | `nitro-xml-render::chart` (**not** `nitro-xml-opc::drawingml`) |
| P9 | `nitro-xml-pptx::render::chart` |
| Adapters | `nitro-xml-xlsx::render::chart`, `nitro-xml-docx::render::chart` |

Chart **model/parse** may live under format crates or `nitro-xml-opc` model modules; only the **painters** are in `nitro-xml-render` / pptx `render::chart`. Update #28's "Files and modules" from `nitro-xml-opc/src/drawingml/chart/` to the paths above when that issue is claimed.

**Split if over one session:** S1 (+ xlsx/docx adapters) first; P9 (20k lines) as its own issue.

### #29 — viewer shell

Not a painter inventory row. Shell UI under `nitro-xml-wasm::shell` consumes the same command buffer. No document painters assigned here.

## Other R0 decisions (recorded for the module map)

- **Editor chrome** uses the same command buffer as document painters.
- **Both chart painters** (S1 and P9) are kept (1:1 rule).
- **Google Fonts fetching (P3/P4):** port unchanged until Jared decides.
- **P17:** poster image only in M1; WebGL "view in 3D" deferred with P13.
- **JS libraries M1 painters depend on:** DOMPurify (P1), opentype.js (P4), emf-converter (pictures/OLE); html2canvas only if export returns; three.js only with deferred WebGL rows.
- Worker/main split and loader budget: #36 and #37 (loader API already landed).

## Consequences

- Renderer issues implement modules listed here; they do not invent parallel layouts.
- Creating the `nitro-xml-render` crate and empty `render/` module files is deferred to the first port PR that needs them (this ADR is names and responsibilities only).
- #28's file list should be corrected when claimed.
