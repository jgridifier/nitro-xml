<!-- SPDX-License-Identifier: MIT -->
# Upstream painter inventory (R0 (a), #34)

Every painter in upstream `ChristopherVR/ooxml` at the pinned commit `e66aaec24adc0b68472eae1fc42aa4598e09c352` (`upstream/`, read only), for the R0 port record (#33). R0 ports these painters one to one: same drawing technology, structure, fonts and measurement. The one allowed change is that each frame's draw and DOM calls cross the WASM boundary as a single command buffer. The Rust module column is confirmed in #35 (see `docs/adr/0002-renderer-port.md`).

## Summary

| Format | Rows | Production lines | DOM | SVG | MathML | WebGL | Canvas 2D |
|---|---:|---:|---:|---:|---:|---:|---:|
| xlsx | 13 | 3,284 | 9 | 3 | 0 | 0 | 0 |
| docx (M1 Print Layout) | 7 | 1,830 | 6 | 1 | 0 | 0 | 0 |
| pptx | 27 | 117,525 | 14 | 8 | 1 | 3 | 1 |
| shared (chart, SmartArt, metafile) | 3 | 2,441* | 0 | 2 | 0 | 0 | 0 |
| E3 Word editing (Draft) | 4 | 2,838 | 2 | 1 | 1 | 0 | 0 |
| **Total** | **54** | **127,918*** | **31** | **15** | **2** | **3** | **1** |

\* S3 (`emf-converter`) lives outside `upstream/` line counts; production lines for shared rows still reflect S1+S2 only until the metafile port lands.

The xlsx conditional-format row (X3) is DOM plus inline SVG; it is counted under DOM, which is why the xlsx output columns add up to 12 of 13 (X3 is the extra row). X9 (xlsx charts) has no files of its own, so it adds 0 lines. M1 Word is Print Layout (D1–D7) plus D4 charts; Draft (D8–D11) is inventoried under E3 and is not an M1 viewer gate.

- **Counts.** "Production lines" are physical lines (`wc -l`) of the files assigned to the row, excluding `*.test.ts`, `*.spec.ts`, `__tests__/`, `__fixtures__/`, `fixtures/`, stories and test-support files. Each row's files are its listed painter plus the helpers folded into it (including the docx stylesheets folded into D1, D8 and D11). The full per-row file list is under [Coverage check](#coverage-check). In total, 1,493 candidate files were enumerated: 813 files (127,918 lines) fall into the inventoried upstream rows and 680 files (83,035 lines) are excluded with a reason. Row **S3** (metafile) was added after R0 sign-off for the external `emf-converter` package.
- **Canvas vs DOM.** No in-scope painter draws the document onto a `<canvas>`. Upstream's on-screen output is DOM (absolutely positioned elements and CSS) for the xlsx grid, docx pages and pptx slides; inline SVG for charts, sparklines, SmartArt, connectors, ink and WordArt; and native MathML for equations. The one canvas-2D painter is the pptx raster export (P26), which is not on the view path. Canvas does appear elsewhere, but only for:
  - text measurement (`measureText`) in every format;
  - pixel operations on pictures (P5 duotone and colour change);
  - WordArt glyph tracing (P4).

  WebGL (three.js) is used only by the pptx 3D paths: P10, P12 and P13, plus model previews in P17. In P10 and P12 the 3D view is opt-in through viewer options. In P17 it is mounted only when the user clicks "view in 3D".
- **Text measurement.** Upstream measures with the browser in all three formats:
  - **xlsx** cells and charts use an offscreen canvas `measureText` with a width cache.
  - **docx** Print Layout uses canvas `measureText` plus a hidden DOM `<span>` for ligatures and small caps, and re-measures on `document.fonts` `loadingdone`.
  - **pptx** text is laid out by the browser's own CSS line breaking, corrected with canvas `measureText` for tracking, tabs and East Asian breaks.

  xlsx and docx load no fonts. They name metric-compatible system fallbacks in CSS stacks (Carlito, Caladea, Arimo, Tinos, Cousine, Liberation, Gelasio, Inconsolata).   pptx loads fonts three ways:
  - the deck's embedded fonts as `@font-face` data URLs (EOT via MTX — clean-room MIT rewrite; see ADR 0002);
  - a remote webfont CSS2 `<link>` fallback for referenced families (upstream defaults to Google Fonts);
  - user-supplied `FontFace` files.

  Following upstream therefore keeps a browser measurement path behind the command buffer. **Font source ([#121](https://github.com/jgridifier/nitro-xml/issues/121), merged in #126):** host-configurable via loader `fontSource` (upstream default, internal mirror URL, remote fetch off with `font_substitution` diagnostics, or resolver). See loader README and `nitro-xml-opc::fonts`.
- **Batching priorities, measured.** `tools/painter-calls/` counted every DOM and canvas-2D call per animation frame with upstream running unbundled in headless Chrome 154. Calls are attributed to rows by call stack; the raw data is in `tools/painter-calls/results.json`, and the column is filled in each row below. The stand-in M-tier fixtures are `poi-StructuredRefs-lots-with-lookups.xlsx` (325 KB), `poi-drawing.docx` (95 KB) and `poi-60810.pptx` (875 KB, 28 slides); #4's manifest will replace them. Ranked by calls in one handoff:
  1. **D1 docx Print Layout pages.** First paint is about 169,000 calls (84,700 per active frame) for one 95 KB document, plus D2 tables (142) and D3 pictures (28). Scrolling Print Layout makes no page calls at all: pages are built once per relayout, not per frame. So the hot path is the relayout handoff, rebuilt in full after every edit (150 ms debounce) and again when fonts load.
  2. **P3 pptx text.** 2,100 to 6,000 calls per slide change (median about 5,350 over slides 2 to 11). That is two orders of magnitude above P1 stage (16 to 24), P2 shapes (20 to 90) and P5 pictures (17 to 68). P11 (SmartArt or diagram) adds 306 on the one slide that has it.
  3. **X2 xlsx cells.** 4,132 calls at first paint, then a median of 399 per steady scroll frame (max 21,945 in one frame when a quadrant re-renders). X4 gridlines (58 median per scroll frame) and X5 headers (25) are next, and X1 frame (4) and X6 selection (6) are small.
  4. **Editor chrome is outside the inventory but is the largest xlsx and docx first-paint cost.** The xlsx ribbon and dialogs make 5,443 calls (charged to `other`, mostly `xlsx/ribbon/*`) plus 4,764 calls in excluded chrome. docx Draft makes 5,834 and 3,241. Head of Product: chrome goes through the same command buffer (`nitro-xml-render::command_buffer`).
  5. **Rows marked "not exercised by fixture".** Charts, sparklines, conditional-format visuals in this sheet, SmartArt in docx, 3D, slide show and export were not hit by these three files or by the default view options. They need fixtures from #4 before they get numbers.

## Columns

- **Output.** DOM means HTML elements styled with CSS. SVG means inline SVG, either created with `createElementNS` or as a markup string parsed into the DOM. Canvas means a 2D context the painter draws on. WebGL means three.js.
- **Text measurement.** The measurement path the painter relies on for line breaks and fitting, and the fonts it names or loads.
- **Rust module (confirmed #35).** Filesystem path under `rs/crates/` for the one-to-one port. Shared S1/S2 and the command buffer live in `nitro-xml-render`, not `nitro-xml-opc`. Rows marked **deferred** or **E3** keep a reserved path but are out of M1 viewer scope. Details: `docs/adr/0002-renderer-port.md`.
- **Draw/DOM calls per frame (M-tier).** Measured with the TS reference by `tools/painter-calls/` on the stand-in M-tier fixtures above. The cell gives total calls and frames at first paint, the median and max per frame while scrolling (xlsx), and per slide change (pptx). The docx Draft and Print Layout scroll scenarios produced no painter calls, because the pages are not re-rendered on scroll.

## xlsx (`.xlsx`, `.xlsm`, `.xltx`, `.xltm`)

Shared inputs, not counted as painters: `src/core/xlsx/layout/` (6,452 lines, the DOM-free grid metrics, cell views, conditional formats and chart view the Excel UI paints) and `src/core/xlsx/ui/grid/geometry.ts`. Font stacks come from `src/core/text/css-font-family.ts` (`cssFontFamily`: Calibri to Carlito, Aptos to Calibri or Carlito, and so on).

| # | Painter | Upstream file(s) | Lines | Output | What it draws | Text measurement, fonts | Rust module (confirmed #35) | Draw/DOM calls per frame (M-tier) |
|---|---|---|---:|---|---|---|---|---|
| X1 | Grid frame and quadrants | `src/ui/src/xlsx/grid/grid-view.ts` (`GridView.paint`, one paint per animation frame), `grid/index.ts` (mount), `grid/dom.ts`, `grid/styles.ts` (grid CSS); folded: `src/core/xlsx/ui/grid/geometry.ts`, `view-cache.ts` | 1,046 | DOM | Native scroller with a sticky view. Four quadrant planes (frozen corner, frozen rows, frozen columns, scrolling cells) are moved by `translate3d`. Also the scroll extent, the freeze lines and the order of the per-quadrant layers. | Owns the shared `TextMeasurer` (X2). | `rs/crates/nitro-xml-xlsx/src/render/grid.rs` | first paint: 210 calls over 4 frames (max 133/frame); scroll: median 4, max 10/frame |
| X2 | Cell layer | `src/ui/src/xlsx/grid/cell-layer.ts` (`CellLayer.render`, `paintCell`), `grid/measure.ts`; folded: `src/core/xlsx/ui/grid/cell-items.ts` (`buildItems`), `cell-paint.ts`, `number-fit.ts` | 679 | DOM (inline SVG for diagonal borders; CSS `data:` SVG tiles for pattern fills) | One recycled node per visible cell with: fill (solid, gradient, pattern), text (with rich runs, rotation, wrap, vertical text, overflow into neighbours, `###` for numbers that do not fit), borders, merged cells, the comment triangle and the error mark. | Canvas `measureText` on an offscreen 2D context (50k-entry width cache). `fontBoundingBoxAscent/Descent` for mixed-run baselines. Estimate fallback only without a canvas. Text itself is browser-rendered in `<span>`s with CSS `font` built from `cssFontFamily` stacks; no fonts loaded. | `rs/crates/nitro-xml-xlsx/src/render/cells.rs` | first paint: 4,132 calls over 2 frames (max 4,124/frame); scroll: median 399, max 399/frame |
| X3 | Conditional-format visuals | Data bars and icons in `grid/cell-layer.ts` `paintCell` (lines counted in X2); `src/core/xlsx/ui/grid/icon-sets.ts` | 137 | DOM + SVG | Data bars as `<div>`s with CSS gradients and an optional dashed axis. Icon sets as inline SVG (16 px at 100%). Colour scales arrive as the cell fill from the core view (painted by X2). | None (geometry from the cell box). | `rs/crates/nitro-xml-xlsx/src/render/cond_format.rs` | not exercised by fixture |
| X4 | Gridlines | `src/ui/src/xlsx/grid/gridlines.ts` (`GridlineLayer.render`) | 53 | DOM | One 1 px `<div>` per visible row and column per quadrant, reused by index. Filled cells paint over them. | None. | `rs/crates/nitro-xml-xlsx/src/render/gridlines.rs` | first paint: 578 calls over 4 frames (max 294/frame); scroll: median 58, max 58/frame |
| X5 | Row and column headers | `src/ui/src/xlsx/grid/headers.ts` (`HeaderLayer.render`), `grid/pane-chrome.ts` (`paintChrome`) | 155 | DOM | Column letters and row numbers for frozen and scrolling panes with selected and fully-selected highlight, the select-all corner and the freeze-pane lines. | None. CSS font size `11 * zoom`. | `rs/crates/nitro-xml-xlsx/src/render/headers.rs` | first paint: 710 calls over 4 frames (max 471/frame); scroll: median 25, max 25/frame |
| X6 | Selection overlay | `src/ui/src/xlsx/grid/selection-layer.ts` (`SelectionLayer.render`) | 143 | DOM (CSS `clip-path` cut-out) | Shaded ranges with the active cell cut out, the green outline, the fill handle, the copy marquee, the formula reference boxes and the fill-drag preview, per quadrant. | None. | `rs/crates/nitro-xml-xlsx/src/render/selection.rs` | first paint: 101 calls over 4 frames (max 81/frame); scroll: median 6, max 6/frame |
| X7 | Remote selections | `src/ui/src/xlsx/grid/remote-layer.ts` | 72 | DOM | Collaborators' coloured outlines with name tags, per quadrant (paint hook). | None. | `rs/crates/nitro-xml-xlsx/src/render/remote_selection.rs` (**deferred**) | first paint: 12 calls over 1 frames (max 12/frame) |
| X8 | Drawings layer (pictures, object frames) | `src/ui/src/xlsx/grid/drawings.ts` (`DrawingLayer.render`, `#paint`) | 265 | DOM (`<img>` from package-part object URLs) | Pictures positioned from their anchors in every quadrant they cross. Hosts charts (X9) and SmartArt (X12). Selection grips. A "(not shown)" frame for unsupported objects. | None. | `rs/crates/nitro-xml-xlsx/src/render/drawings.rs` | not exercised by fixture |
| X9 | Charts | Chart branch of `grid/drawings.ts` (lines in X8); view model `chartView` in `src/core/xlsx` (input); painter is S1 `renderChartSvg` | 0 (see S1) | SVG (string parsed with `svgNode`) | Chart from live cell values (`createRefEvaluator`), redrawn when the generation counter changes. | Host canvas measurer passed to S1 (`measureText`, `measureFont`). Family `Calibri, Carlito, Arial, sans-serif`. | `rs/crates/nitro-xml-xlsx/src/render/chart.rs` (adapter → `nitro-xml-render::chart`) | not exercised by fixture |
| X10 | Print | `src/ui/src/xlsx/print.ts` | 173 | DOM (HTML `<table>` printed through a hidden frame) | Print area or used range as a table with column widths, row heights, merges and per-cell CSS. No headers, footers or fit-to-page (upstream says so). | Browser table layout. CSS stack `"<font>", Calibri, sans-serif`. | `rs/crates/nitro-xml-xlsx/src/render/print.rs` (**deferred**) | not exercised by fixture |
| X11 | Sparklines | `src/ui/src/xlsx/grid/sparkline.ts` (`sparklineSvg`) | 88 | SVG (`createElementNS`) | Line, column and win/loss sparklines with axis and markers, behind the cell's text (called from X2). | None. | `rs/crates/nitro-xml-xlsx/src/render/sparkline.rs` | not exercised by fixture |
| X12 | SmartArt frame | `src/ui/src/xlsx/grid/smartart.ts` (`paintSmartArt`); painter is S2 | 80 | SVG via S2 (`<office-ui-smartart>`); DOM list placeholder | Cached SmartArt drawing scaled to the frame, theme colours and fonts mapped, a "display only" badge. Without a cached drawing, a list of node texts. | As S2. | `rs/crates/nitro-xml-xlsx/src/render/smartart.rs` (adapter → `nitro-xml-render::smartart`) | not exercised by fixture |
| X13 | Sheet tabs | `src/ui/src/xlsx/sheet-tabs/render.ts`, `index.ts`, `styles.ts` | 393 | DOM | Tab strip for visible sheets with tab colours and the active tab. Drag and rename (`drag.ts`, `rename.ts`) are editor code and were not enumerated. | Browser layout. | `rs/crates/nitro-xml-xlsx/src/render/sheet_tabs.rs` | first paint: 106 calls over 2 frames (max 71/frame) |

## docx (`.docx`, `.docm`, `.dotx`, `.dotm`) — M1 Print Layout

Upstream has two views. **Print Layout** is the M1 Word viewer: a read-only paginated render of `layoutDocumentModel` (`src/core/docx/layout/`, the layout engine; it is input, not a painter, apart from the measurement files folded into D1). **Draft** (ProseMirror, the default `viewMode`) is the editing surface and is inventoried under [E3](#e3--word-editing-draft-view). Equations are typeset only in Draft. Print Layout shows an `[Equation]` marker and reports the approximation (`src/core/docx/layout/adapter.ts`). Print Layout styling is `src/ui/src/docx/styles/print.css` (folded into D1; it styles D1–D7).

| # | Painter | Upstream file(s) | Lines | Output | What it draws | Text measurement, fonts | Rust module (confirmed #35) | Draw/DOM calls per frame (M-tier) |
|---|---|---|---:|---|---|---|---|---|
| D1 | Print Layout pages | `src/ui/src/docx/print-layout.ts` (`renderPrintLayout`), `print-layout-view.ts` (controller, 150 ms relayout, font reload), `canvas-measurer.ts`; folded: `print-fragment-hit.ts`, `print-layout-cursor.ts` (click to cursor), `styles/print.css`, `src/core/docx/layout/fonts.ts`, `layout/measure.ts` | 1,013 | DOM | One sheet per page with columns and column rules, blocks, lines and absolutely positioned fragment `<span>`s (font, scale, letter spacing, kerning, baseline). Also tab leaders, paragraph borders and shading, the footnote area with its separator, and floating pictures. | Canvas `measureText` (`createCanvasMeasurer`, 20k width cache, `fontKerning`). A hidden DOM `<span>` with `getBoundingClientRect` when ligatures or small caps are set. `fontBoundingBox*` for line height. `cssFontStack` metric-compatible substitutes (Carlito, Caladea, Arimo, Tinos, Cousine, Liberation, Gelasio, Inconsolata). No fonts loaded. Re-measures on `document.fonts` `loadingdone`. | `rs/crates/nitro-xml-docx/src/render/print/pages.rs` (+ `print/styles.rs` for `print.css`) | first paint (Draft): 3 calls over 1 frames (max 3/frame); first paint (Print Layout): 169,449 calls over 2 frames (max 169,448/frame) |
| D2 | Print Layout tables | `src/ui/src/docx/print-table.ts` (`renderTable`); folded: `src/core/docx/ui/table-render.ts`, `src/core/docx/table-visuals.ts` | 294 | DOM | Table grid with cell borders and shading resolved from table styles, and cell paragraphs through D1's block renderer. | As D1 (cell text). | `rs/crates/nitro-xml-docx/src/render/print/table.rs` | first paint (Print Layout): 142 calls over 1 frames (max 142/frame) |
| D3 | Print Layout pictures | `src/ui/src/docx/print-picture.ts` (`pictureElement`) | 54 | DOM (`<img>`, placeholder box) | Inline and floating pictures from package parts (`PictureUrl`). Hosts charts (D4). | None. | `rs/crates/nitro-xml-docx/src/render/print/picture.rs` | first paint (Print Layout): 28 calls over 1 frames (max 28/frame) |
| D4 | Charts (Print Layout and Draft) | `src/core/docx/chart-paint.ts` (`docxChartPaint`), `src/core/docx/ui/chart-spec.ts` (Draft node spec); chart branch of `print-picture.ts`; painter is S1 `renderChartSpaceSvg` | 130 | SVG (string parsed with `DOMParser`) | Chart from the chart part's cached values with the document theme, or a labelled frame for families S1 does not draw. | **No host measurer.** S1 falls back to its character-width estimate (`textWidth` in `chart-svg-util.ts`). | `rs/crates/nitro-xml-docx/src/render/chart.rs` (adapter → `nitro-xml-render::chart`) | not exercised by fixture |
| D5 | Headers, footers and page fields | `src/ui/src/docx/print-header-footer.ts` (`decoratePages`) | 236 | DOM | Header and footer blocks per page with page-number and page-count fields and pictures. | As D1. | `rs/crates/nitro-xml-docx/src/render/print/header_footer.rs` | not exercised by fixture |
| D6 | Page borders | `src/ui/src/docx/print-page-borders.ts` (`pageBorderBox`) | 56 | DOM | Section page borders per sheet (from the page edge or the text). Art borders drawn as plain lines. | None. | `rs/crates/nitro-xml-docx/src/render/print/page_borders.rs` | not exercised by fixture |
| D7 | Line numbers | `src/ui/src/docx/print-line-numbers.ts` (`lineNumberLabels`) | 47 | DOM | Line-number labels per section and column, skipping suppressed paragraphs. | As D1. | `rs/crates/nitro-xml-docx/src/render/print/line_numbers.rs` | not exercised by fixture |

## E3 — Word editing (Draft view)

Draft is the ProseMirror editing surface. Head of Product (2026-10-11): D8–D11 move to the Word editing stage E3. They stay in this inventory so the port mapping is complete, but they are not M1 viewer work. `styles/document.css` and the Draft helpers (`review-*.ts`, `preview-run-styles.ts`, `scaled-paragraph-breaks.ts`, `tab-stops-view.ts`) fold into D8; `styles/smartart.css` folds into D11. The styles aggregator `styles/index.ts` is chrome (`X:chrome`).

| # | Painter | Upstream file(s) | Lines | Output | What it draws | Text measurement, fonts | Rust module (confirmed #35) | Draw/DOM calls per frame (M-tier) |
|---|---|---|---:|---|---|---|---|---|
| D8 | Draft surface | `src/ui/src/docx/editor-render.ts`, `schema.ts` (ProseMirror `toDOM` specs), `run-styles.ts`, `run-scale.ts`, `list-marker-display.ts`, `paragraph-decoration.ts`, `header-footer-view.ts`, `notes-view.ts`; folded: `preview-run-styles.ts`, `scaled-paragraph-breaks.ts`, `tab-stops-view.ts`, `review-*.ts` (commands, controller, display, options-dialog, paragraph-display, ribbon, run-css, run-marks, story-previews), `styles/document.css` | 2,312 | DOM (ProseMirror view) | Continuous document: paragraphs, runs and run CSS, list markers, paragraph borders and shading, tables (schema nodes), header, footer and notes previews, review marks and tab-stop helpers. | Browser line breaking. Canvas measurer (as D1) for scaled runs and list markers. `cssFontStack`. | `rs/crates/nitro-xml-docx/src/render/draft/surface.rs` (**E3**) | first paint (Draft): 7,503 calls over 4 frames (max 7,023/frame) |
| D9 | Draft pictures | `src/ui/src/docx/image-media.ts` (node views, object-URL cache) | 143 | DOM (`<img>`) | Inline and floating pictures in Draft. Dispatches chart (D4) and SmartArt (D11) node views. | None. | `rs/crates/nitro-xml-docx/src/render/draft/picture.rs` (**E3**) | first paint (Draft): 234 calls over 1 frames (max 234/frame) |
| D10 | Equations | `src/ui/src/docx/equation-schema.ts` (`equationDom`); converter `src/core/math/omml-to-mathml.ts` (input) | 39 | MathML (native) | OMML converted to MathML and shown read-only (inline or display), with a LaTeX `aria-label`. | Browser MathML layout and fonts. | `rs/crates/nitro-xml-docx/src/render/draft/equation.rs` (**E3**) | not exercised by fixture |
| D11 | SmartArt | `src/ui/src/docx/smartart-node-view.ts`, `src/core/docx/ui/smartart-theme.ts`, `styles/smartart.css`; painter is S2 | 344 | SVG via S2 | Cached SmartArt drawing in Draft with theme colours. | As S2. | `rs/crates/nitro-xml-docx/src/render/draft/smartart.rs` (**E3**; adapter → `nitro-xml-render::smartart`) | not exercised by fixture |

## pptx (`.pptx`, `.pptm`, `.potx`, `.potm`)

The slide renderer is `src/ui/src/pptx/dom/` (`renderSlideStage` plus a registry of per-element renderers in `dom/elements/`). Its framework-free decisions live in `src/ui/src/pptx/render/` (975 files), and upstream's `AGENTS.md` says new view logic goes there. Each row is one registered element renderer (or stage-level painter), with the `render/` helpers it uses folded in. Model inputs (`src/core/pptx/core`, the parser) are not counted.

| # | Painter | Upstream file(s) | Lines | Output | What it draws | Text measurement, fonts | Rust module (confirmed #35) | Draw/DOM calls per frame (M-tier) |
|---|---|---|---:|---|---|---|---|---|
| P1 | Slide stage and background | `src/ui/src/pptx/dom/slide-stage.ts` (`renderSlideStage`), `dom/dom.ts`, `registry.ts`, `types.ts`, `presentation-rerender.ts`, `load-presentation.ts`, `dom/elements/index.ts` (`createDefaultRegistry`), `dom/index.ts`, `dom/elements/register-*.ts`; +29 folded `render/` helpers (backgrounds, device-pixel strokes, field context, visibility, accessibility, hyperlinks, `render/index.ts`) | 6,382 | DOM | Slide box at scale, background (solid, gradient, picture, pattern, from master or layout), inherited template elements then slide elements in z-order, hit targets, the View > Grid overlay, and field substitution. | As P3 for text. Loads the deck's embedded fonts (P3). | `rs/crates/nitro-xml-pptx/src/render/stage.rs` | first paint (slide 1): 17; slide change (slides 2-11): median 17, max 24 per change |
| P2 | Shapes (geometry, fill, line, effects) | `dom/elements/text-shape.ts` (`renderTextShapeElement`, also `text` elements), `dom/element-styles.ts`, `shape-filter-defs.ts`, `extrusion-overlay.ts`, `reflection-mirror-content.ts`; +50 folded `render/` helpers (`shape-geometry*`, `fill-style`, `stroke-*`, `svg-*`, `visual-effects`, `visual-3d-*`, `reflection`, `drawing-color`, ...) | 12,229 | DOM + CSS (`clip-path` preset geometry, CSS gradients, filters, CSS 3D transforms); inline SVG for `custGeom` paths, gradient and stroke-only outlines, filter defs | Preset and custom geometry, solid, gradient, pattern and picture fills, lines (dash, compound, arrows), shadow, glow, soft edge, reflection, bevel and extrusion approximations, then the text body (P3). | As P3. | `rs/crates/nitro-xml-pptx/src/render/shape.rs` | first paint (slide 1): 20; slide change (slides 2-11): median 20, max 90 per change |
| P3 | Text body | `dom/elements/text-block.ts` (`renderTextBlock`), `text-run-content.ts`; +64 folded `render/` helpers (`text-*` except WordArt, `paragraph-*`, `bullet-autonum*`, `kinsoku-styles`, `embedded-fonts`, `google-*`, `custom-fonts`, `font-*`, `linked-text-box-overflow`). **`render/text-warp.ts` → P4.** | 11,780 | DOM (`<p>`, `<span>`, `<a>`, ruby) | Paragraphs, runs, bullets and autonumbering, tabs with leaders, autofit shrink, columns, vertical and East Asian text, fields, hyperlinks, text effects and gradient text. | **Browser CSS line breaking**, corrected with canvas `measureText` for per-run GDI tracking (`text-metric-tracking.ts`), tab stops, East Asian breaks and gradient spans. **Fonts loaded**: embedded deck fonts as `@font-face` data URLs (`embedded-fonts.ts`); Google Fonts CSS2 `<link>` fallback for referenced families (`google-webfonts.ts`, metric clones); user `FontFace` files (`custom-fonts.ts`). | `rs/crates/nitro-xml-pptx/src/render/text.rs` | first paint (slide 1): 2,139; slide change (slides 2-11): median 5351.5, max 5,997 per change |
| P4 | WordArt text warp | `dom/elements/text-warp.ts`, `dom/glyph-outline-cache.ts`; +20 folded `render/text-warp.ts`, `render/text-warp-*`, `text-3d-*`, `text-effects-3d` | 4,205 | SVG (paths) plus CSS approximations; offscreen canvas for glyph tracing | `prstTxWarp` presets: path warps, envelope warps (inflate, deflate, can), glyph outlines, 3D text. | Canvas `measureText` per glyph (`text-warp-envelope-measure.ts`). Glyph outlines from font files parsed with `opentype.js` (embedded or Google font bytes fetched), else traced from canvas rendering. | `rs/crates/nitro-xml-pptx/src/render/wordart.rs` | not exercised by fixture |
| P5 | Pictures | `dom/elements/image.ts` (`renderImageElement`); +12 folded `render/image-*` (including `image-native-size.ts` for `a:tile` sx/sy against intrinsic pixels); `src/core/pptx/core/utils/svg-stretch-fill.ts` | 2,675 | DOM (`<img>`, CSS filter and crop) + SVG `<filter>` defs; canvas for duotone and `clrChange` pixel work | Pictures with crop, fit, tile, stretch, brightness and contrast, recolour, duotone, artistic effects, colour change and transparency. | None. EMF and WMF converted via **S3** (`nitro-xml-render::metafile`). Intrinsic size via post-apply `NaturalWidth` read-back. | `rs/crates/nitro-xml-pptx/src/render/picture.rs` | first paint (slide 1): 17; slide change (slides 2-11): median 17, max 68 per change |
| P6 | Group | `dom/elements/group.ts` (`renderGroupElement`), `render/group-fill.ts` | 221 | DOM | Group transform (offset, scale, flip, rotation) and children, including `grpFill` inheritance. | None. | `rs/crates/nitro-xml-pptx/src/render/group.rs` | not exercised by fixture |
| P7 | Connectors | `dom/elements/connector.ts` (`renderConnectorElement`); +5 folded `render/connector-{dash,elbow-geometry,markers,path,style}` | 912 | SVG | Straight, bent and curved connectors with dash, compound lines and arrowheads, plus the connector's text. | As P3 for text. | `rs/crates/nitro-xml-pptx/src/render/connector.rs` | not exercised by fixture |
| P8 | Tables | `dom/elements/table.ts` (`renderTableElement`); +12 folded `render/table-style*`, `table-cell-{css,padding,paragraphs}`, `table-advanced-fill` | 2,551 | DOM (`<table>`) | Cells, merges, borders, built-in and custom table styles with banding, cell fills, 3D cell bevels, and cell text. | As P3. | `rs/crates/nitro-xml-pptx/src/render/table.rs` | not exercised by fixture |
| P9 | Charts (2D) | `dom/elements/chart.ts` (`renderChartElement`), `chart-svg.ts`; +141 folded `render/chart-*` (view model `chart-view-model-*`, painter `chart-view-model-dom.ts`, axes, legend, labels, pie, radar, waterfall, treemap, sunburst, funnel, box-and-whisker, histogram, region map, stock, surface (flat and isometric), trendlines, error bars, data table, user shapes) | 20,387 | SVG (`createElementNS`) | Every 2D chart family PowerPoint draws, including `cx:` chartex. Upstream's own pptx chart painter, separate from S1. | SVG `<text>`. Canvas `measureText` for data-label boxes (`chart-label-measure.ts`). `text-wrap-estimate.ts` for horizontal-bar labels. | `rs/crates/nitro-xml-pptx/src/render/chart.rs` | not exercised by fixture |
| P10 | Charts (3D, opt-in) | +37 folded `render/chart-3d-*`, `chart-bar3d-*`, `chart-pie3d-tilt`, `surface-chart-3d-*` (hosted by P13) | 5,502 | WebGL (three.js) with an SVG fallback (P9) | 3D bar, line, area, pie and surface charts when the host enables `barChart3D` and the other 3D options. | Label sizes from `text-wrap-estimate.ts`. | `rs/crates/nitro-xml-pptx/src/render/chart3d.rs` (**deferred**, opt-in WebGL) | not exercised by fixture |
| P11 | SmartArt | `dom/elements/smartart.ts` (`renderSmartArtElement`), `smartart-svg.ts`, `smartart-fallback.ts`; +28 folded `render/smartart-{drawing,layout,node-label,builtin-layouts,accessibility}*`, `smart-art-preset*` | 3,708 | SVG | **Cached drawing** (`dsp:drawing` shapes) projected as SVG with gradients and text. Without a cached drawing, a fallback layout (`computeSmartArtLayout`, the 19 `smartart-layout-*` interpreters). | Text wrapped by estimated widths (`svg-text-lines.ts` with `text-wrap-estimate.ts`). SVG `<text>`. Theme fonts. | `rs/crates/nitro-xml-pptx/src/render/smartart.rs` | first paint (slide 1): 0; slide change (slides 2-11): median 0, max 306 per change |
| P12 | SmartArt 3D (opt-in) | `dom/elements/smartart-3d.ts`; +22 folded `render/smartart-3d-*`; `src/ui/src/pptx/smartart-3d/*` (9 files); `src/core/geometry/svg-path-{flatten,curves,types}.ts` | 4,225 | WebGL (three.js) with an SVG fallback (P11) | SmartArt with `scene3d` and `sp3d` as lit meshes and extruded text, when `smartArt3D` is set. | Canvas text textures (`smartart-3d/text-block-texture.ts`, `measureText`). | `rs/crates/nitro-xml-pptx/src/render/smartart3d.rs` (**deferred**, opt-in WebGL) | not exercised by fixture |
| P13 | Three.js view host | `src/ui/src/pptx/three-view/*` (11 files), `dom/elements/three-view.ts`, `three-view-reuse.ts` | 1,943 | WebGL | The shared `<pptx-three-view>` element: one WebGL context for every 3D scene, sizing, overflow, export snapshot. | None. | `rs/crates/nitro-xml-pptx/src/render/three_host.rs` (**deferred**, opt-in WebGL) | not exercised by fixture |
| P14 | Media (audio, video) and posters | `dom/elements/media.ts` (`renderMediaElement`), `media-fullscreen-overlay.ts`; folded `render/media-{fullscreen,persistent-audio,playback,trim-*}`, `online-video`. **`media-file-type.ts` excluded** (insert MIME classifier → `X:editor`). | 1,373 | DOM (`<video>`, `<audio>`, `<img>` poster) | Poster frame, or the poster alone when no source is playable. Native controls in reading mode. Trim, fade and full-screen playback. Online-video poster. | None. | `rs/crates/nitro-xml-pptx/src/render/media.rs` | not exercised by fixture |
| P15 | OLE objects (previews) | `dom/elements/ole.ts` (`renderOleElement`), `ole-icons.ts`; folded `render/ole-{icon-primitives,renderer-helpers}` | 602 | DOM (`<img>` preview, badge) + SVG icons | The OLE preview image (`previewImageData`) with a type badge, or a typed placeholder (Excel, Word, PDF, Visio, MathType icons). EMF/WMF previews via **S3**. | None. | `rs/crates/nitro-xml-pptx/src/render/ole.rs` | not exercised by fixture |
| P16 | ActiveX controls | `dom/activex-controls-overlay.ts`, `render/activex-overlay-view.ts` | 139 | DOM | Static fallback picture or a labelled placeholder for each `p:control`. | None. | `rs/crates/nitro-xml-pptx/src/render/activex.rs` | not exercised by fixture |
| P17 | 3D models (previews) | `dom/elements/model3d.ts` (`renderModel3DElement`); folded `render/model3d-scene*` | 831 | DOM (`<img>` poster); WebGL on demand (P13) | Poster image or a "3D Model" placeholder. GLB or glTF mounted with three.js only on "view in 3D". | None. | `rs/crates/nitro-xml-pptx/src/render/model3d.rs` (M1: **poster only**; WebGL with P13) | not exercised by fixture |
| P18 | Ink | `dom/elements/ink.ts`, `ink-stroke-svg.ts` (`buildStrokeSvg`); +5 folded `render/ink-{curve-sampling,rendering,stroke-view,tilt-nib,group-strokes}` | 1,187 | SVG | Ink strokes: smoothed paths, pressure circles, tilt nib marks, highlighter blending. | None. | `rs/crates/nitro-xml-pptx/src/render/ink.rs` | not exercised by fixture |
| P19 | Content parts | `dom/elements/contentpart.ts`, `render/content-part-strokes.ts` | 126 | SVG | `p:contentPart` (`mc:AlternateContent` ink and drawing parts). | None. | `rs/crates/nitro-xml-pptx/src/render/content_part.rs` | not exercised by fixture |
| P20 | Equations | `dom/elements/equation.ts`; +6 folded `render/omml-*`, `mathml-sanitize` | 134 | MathML (native) | Pure-OMML equation boxes as sanitized MathML, centred in the shape. | Browser MathML layout and fonts. | `rs/crates/nitro-xml-pptx/src/render/equation.rs` | not exercised by fixture |
| P21 | Slide and section zoom | `dom/elements/zoom.ts`; folded `render/summary-zoom.ts`, `zoom-return-navigation.ts` | 585 | DOM | Zoom tiles (slide thumbnail or image) and summary zoom. | As P3 for labels. | `rs/crates/nitro-xml-pptx/src/render/zoom.rs` | not exercised by fixture |
| P22 | Unknown-element placeholder | `dom/elements/placeholder.ts` (registry fallback), `render/placeholder-prompt.ts` | 125 | DOM | Typed placeholder box for element types without a renderer. Prompt text for empty inherited placeholders. | As P3. | `rs/crates/nitro-xml-pptx/src/render/placeholder.rs` | not exercised by fixture |
| P23 | Comment markers | `dom/comment-markers.ts`, `render/comment-markers.ts` | 141 | DOM | Numbered comment dots with author and text tooltips. | None. | `rs/crates/nitro-xml-pptx/src/render/comments.rs` | not exercised by fixture |
| P24 | Slide show: animations, transitions, Morph (**deferred** — Jared's call; does not block R0) | +172 folded `render/animation-*`, `slide-transition-*`, `p14-transition-*`, `morph-*`, `motion-path-*`, `effect-sound-*`, `presentation-{animation,blackboard,show,visibility,hit,action,ole,subtitles,context}`, `diagram-build`, `chart-reveal-descriptor`, `ink-replay-timeline`, `annotation-overlay` and others | 29,944 | DOM + CSS keyframes | Entrance, emphasis, exit and motion-path animations, build steps for text, charts and diagrams, slide transitions (including p14), Morph, sounds, the blackboard, and pen annotations during a show. | As P3. | `rs/crates/nitro-xml-pptx/src/render/slideshow.rs` (**deferred**) | not exercised by fixture |
| P25 | Print, notes pages and handouts | `src/ui/src/pptx/export/svg-print.ts`, `render/presentation-print.ts`, `render/notes/notes-{print,html,style-cascade,utils}.ts` | 1,100 | SVG and print-HTML strings (DOM-free assembly; the DOM driver that clones the live slide is in the React binding under `viewers/`) | Vector print of slides, notes pages and handouts. | As P3. | `rs/crates/nitro-xml-pptx/src/render/print.rs` (**deferred**) | not exercised by fixture |
| P26 | Raster export | `src/ui/src/pptx/export/*` (8 files: `rasterize-element*`, `rasterize-foreign-object`, `html2canvas-clone`, `canvas-jpeg`, `canvas-color-fix`, `canvas-size-probe`); folded `render/foreign-object-*`, `export-{clone,progress,tile-plan}`, `canvas-size-probe` | 2,070 | Canvas 2D (SVG `foreignObject` to canvas, tiled) | PNG and JPEG export of slides. Off the view path. | Embeds fonts into the `foreignObject` SVG (`foreign-object-font-embed.ts`). | `rs/crates/nitro-xml-pptx/src/render/raster_export.rs` (**deferred**) | not exercised by fixture |
| P27 | SVG exporter (core converter) | `src/core/pptx/converter/SvgExporter.ts`, `ShapeImageRenderer.ts`, `ShapeTextRenderer.ts`, `SlideMetadataRenderer.ts`, `TextSegmentRenderer.ts`, `svg-*.ts` (10 files) | 2,448 | SVG string (DOM-free) | Slides to standalone SVG (shapes, text lines, images with effects, rich elements, charts through S1's `renderChartSvg`). | Optional host `measureText`. Headless, it falls back to generated advance-width tables (`src/core/text/font-metrics/font-advance-widths.generated`) through `svg-text-lines.ts`. | `rs/crates/nitro-xml-pptx/src/render/svg_export.rs` (**deferred**) | not exercised by fixture |

## Shared painters

Shared chart and SmartArt painters live in the **`nitro-xml-render`** crate with the command-buffer types and drawing primitives — not in `nitro-xml-opc` (Head of Product, #33 review). `nitro-xml-opc::drawingml` remains DrawingML **model** only.

| # | Painter | Upstream file(s) | Lines | Output | What it draws | Text measurement, fonts | Rust module (confirmed #35) | Draw/DOM calls per frame (M-tier) |
|---|---|---|---:|---|---|---|---|---|
| S1 | Chart SVG painter (`ooxml-core/chart/render`) | `src/core/chart/render/chart-svg.ts` (`renderChartSvg`), `render-space.ts` (`renderChartSpaceSvg`), `chart-svg-{cartesian,radial,bars,legend,title-text,appearance,text-metrics,util}.ts`, `chart-view.ts` and the rest of `render/` (21 files) | 2,121 | SVG markup string (DOM-free) | Title, legend, axes, gridlines and series for bar, column, line, area, scatter, pie, doughnut and radar charts. A labelled frame for bubble, stock and surface. Used by xlsx (X9), docx (D4) and the pptx SVG exporter (P27). | Host-supplied `measureText` and `measureFont` when given (xlsx). Otherwise a character-width estimate (docx, P27). Family `Calibri, Carlito, Arial, sans-serif`. | `rs/crates/nitro-xml-render/src/chart/` (#28) | not exercised by fixture |
| S2 | SmartArt cached-drawing painter (`<office-ui-smartart>`) | `src/ui/src/smartart/smartart-svg.ts` (`renderDiagramDrawing`), `smartart/smartart.ts` (Lit element), `src/ui/src/smartart.ts` | 320 | SVG (`createElementNS`, inside a Lit custom element) | The producing application's cached `DiagramDrawing`, never re-laid out. Reports approximations: preset outlines drawn as rectangles, gradients and patterns by their first stop, 3D drawn flat. Used by xlsx (X12) and docx (D11). pptx uses its own P11. | SVG `<text>` at cached positions. Theme `major` and `minor` Latin fonts. No measurement. | `rs/crates/nitro-xml-render/src/smartart/` | not exercised by fixture |
| S3 | EMF/WMF metafile converter | External package [`emf-converter`](https://github.com/ChristopherVR/emf-converter) (**Apache-2.0**), loaded on demand by `src/core/pptx/core/utils/metafile-convert.ts` | n/a (outside `upstream/`) | Raster / SVG data URL (off the DOM paint path; feeds `<img>` sources for P5/P15) | Converts EMF and WMF picture and OLE preview binaries to PNG/JPEG data URLs (or SVG) before the picture/OLE painters draw them. | None. | `rs/crates/nitro-xml-render/src/metafile/` (`nitro-xml-render::metafile`) | not exercised by fixture |


## Confirmed module layout (#35)

Full crate and per-issue lists: [`docs/adr/0002-renderer-port.md`](adr/0002-renderer-port.md).

- **Shared once:** `nitro-xml-render::{command_buffer, primitives, chart (S1), smartart (S2), metafile (S3), measure}`.
- **Format crates:** `nitro-xml-{xlsx,docx,pptx}/src/render/` — one module per inventory row (paths in the tables above).
- **Renderer issues:** #18 xlsx · #23 docx Print Layout (D1–D7) · #27 pptx (split by element family) · #28 charts (S1 + P9; not opc) · #29 shell (no painter rows). D8–D11 → E3.
- **Deferred modules (paths reserved):** X7, X10, P10, P12, P13, P24, P25, P26, P27.

## Out of scope

Not rows. Scope is `.docx`, `.xlsx` and `.pptx` and their macro and template variants only.

- **Visio:** `src/ui/src/visio/`, `src/core/visio/` (9 enumerated paint files: `layer-paint.ts`, `source-paint.ts`, `edit-formatting-paint*.ts`, `ui/draw-plan.ts`, `ui/draw-path-plan.ts`, `ui/svg-stroke.ts`, `ui/sample-drawing.ts`, `ui/format-painter.ts`), `viewers/visio/`.
- **Teams:** `src/ui/src/teams/` (including its `presentation-preview.ts`, which reuses the pptx slide stage), `src/core/teams/`, `viewers/teams/`.
- **Legacy binary formats** (`.doc`, `.xls`, `.ppt`): codecs live in the separate `ole2` package. The in-tree `.ppt` writer (`src/core/pptx/core/ppt/writer/`, including `svg-rasterize*.ts`) and `.xls` loading (`src/core/xlsx/load/`) are excluded.
- **Framework bindings** under `viewers/{xlsx,docx,pptx}/packages/*` (React, Vue, Angular, Svelte, Solid, vanilla). Upstream's `AGENTS.md` says they own only framework wiring, and the shared painters are the `src/ui/src/<product>` copies inventoried above.

## Follow-ups

1. **Calls per frame for the rows not yet exercised.** 36 of the original 53 rows read "not exercised by fixture". Re-run `tools/painter-calls/` on #4's M-tier files and on fixtures that hit charts, sparklines, SmartArt, docx headers, footers and equations, and on the opt-in pptx paths. The docx Draft scroll scenario produced no calls, so check that the harness scrolls the right element.
2. **EMF and WMF pictures and OLE previews** — inventory row **S3** added; Rust module stub `nitro-xml-render::metafile` exists. Upstream package licence **Apache-2.0**. Converter implementation lands with the first P5/P15 port.
3. **Two chart painters.** Confirmed in #35: keep both (1:1). S1 → `nitro-xml-render::chart`; P9 → `nitro-xml-pptx::render::chart`. Not in `nitro-xml-opc`.
4. **Deferred / tracked separately:**
   - **P24 slide show** (29,944 lines, the largest row) stays deferred until Jared decides (Head of Product, 2026-10-11).
   - **Font source (P3/P4):** [#121](https://github.com/jgridifier/nitro-xml/issues/121) implemented in #126 — loader `fontSource` (default / mirror / off / resolver).
   - **MTX / EOT:** clean-room MIT rewrite (not MPL `mtx-decompressor`); see ADR 0002 licence decisions.
   - Also deferred per Head of Product: X7 (remote selections), X10 and P25 (print), P26 and P27 (export), P10/P12/P13 (opt-in WebGL). P17 3D keeps only the poster image. D8–D11 are E3.

   Stage 5 says "all element kinds". Export and opt-in 3D remain out of the M1 viewer gate until Jared says otherwise.
5. **Text measurement and post-apply layout read-back.** Every format measures in the browser; pptx also loads web fonts at runtime. Batched `measureText` and post-apply node-id layout read-back (span rect, scrollHeight, computed style, naturalWidth) are specified in the loader README, `docs/boundary.md` and ADR 0002. PRODUCT_SPEC §6 says R0 re-decides this if shaping moves into WASM.
6. **docx equations** are typeset only in Draft (D10, E3). Print Layout shows `[Equation]` markers, so the parity reference for equations is Draft / E3.
7. **Excluded editor code that later milestones will need.** These files sit next to painters and are excluded here as editor or shell:
   - selection and edit overlays (`chart-editable`, `table-editable`, `smartart-editable`, edit points, crop);
   - xlsx `cell-editor.ts`;
   - pptx slide sorter, thumbnails, outline and reading views.

   They belong to Stage 6 or M2.

## Coverage check

The candidates were enumerated at the pin with the script below, run from `upstream/`. It produced 1,493 files. It takes the union of:

- **(A)** paths naming render, paint, draw, canvas or svg under the three format UIs and the whole core;
- **(B)** files calling drawing or measurement APIs in the format UIs and shared SmartArt;
- **(C)** whole painter directories (xlsx grid, pptx `dom/`, `three-view/`, `smartart-3d/`, shared SmartArt, `core/chart/render`) plus `print-*`;
- **(D)** painters whose names say none of those (docx Draft, equations, tables, charts and SmartArt node views, Draft helpers, docx layout measurement, xlsx sheet tabs);
- **(E)** docx `styles/` (`.css` and `index.ts`), including Print Layout, Draft and SmartArt stylesheets.

```bash
#!/usr/bin/env bash
# Enumerate candidate painter files at the pinned upstream commit (run from upstream/).
set -euo pipefail
prod() { grep -vE '\.(test|spec)\.tsx?$|/__tests__/|/__fixtures__/|/fixtures?/|\.stories\.|/stories/|/test-support/|test-support\.ts$|test-context\.ts$|test-helpers\.ts$'; }
{
  # A: path names render/paint/draw/canvas/svg anywhere in the format UIs and the core
  find src/ui/src/xlsx src/ui/src/docx src/ui/src/pptx src/core -type f \( -name '*.ts' -o -name '*.tsx' \) | grep -iE '(render|paint|draw|canvas|svg)'
  # B: files in the format UIs and shared SmartArt that call drawing / measurement APIs
  grep -rlE 'getContext\(|measureText|createElementNS|fillText|drawImage|WebGLRenderer|new FontFace|@font-face' \
    src/ui/src/xlsx src/ui/src/docx src/ui/src/pptx src/ui/src/smartart src/ui/src/smartart.ts --include='*.ts' --include='*.tsx'
  # C: whole painter directories and painter file families
  find src/ui/src/xlsx/grid src/ui/src/pptx/dom src/ui/src/pptx/three-view src/ui/src/pptx/smartart-3d \
       src/ui/src/smartart src/core/chart/render src/core/xlsx/ui/grid -type f -name '*.ts'
  ls src/ui/src/docx/print-*.ts src/ui/src/xlsx/print.ts src/ui/src/smartart.ts
  # D: painters whose names say neither (docx Draft surface, equations, tables, SmartArt and charts on it;
  #    docx layout measurement; xlsx sheet tabs)
  ls src/ui/src/docx/{schema,run-styles,run-scale,image-media,smartart-node-view,equation-schema,list-marker-display,header-footer-view,notes-view,paragraph-decoration,preview-run-styles,scaled-paragraph-breaks,tab-stops-view}.ts \
     src/ui/src/docx/review-{commands,controller,display,options-dialog,paragraph-display,ribbon,run-css,run-marks,story-previews}.ts \
     src/core/docx/ui/{chart-spec,smartart-theme}.ts src/core/docx/table-visuals.ts src/core/docx/layout/{fonts,measure}.ts \
     src/ui/src/xlsx/sheet-tabs/{index,styles}.ts
  # E: docx stylesheets (Print Layout, Draft, SmartArt) and the styles entry; chrome CSS classified as X:chrome
  find src/ui/src/docx/styles -type f \( -name '*.css' -o -name 'index.ts' \)
} | prod | sort -u
```

Each file was then assigned by the ordered rules in the classifier below (first match wins). **All 1,493 files matched a rule and 0 were unmatched.** Of these:

- **813 files (127,918 lines) are rows or folded into rows**, listed per row below (including E3 Draft rows D8–D11).
- **680 files (83,035 lines) are excluded**, with one of these reasons:

| Exclusion | Files | Lines | Reason |
|---|---:|---:|---|
| `X:editor` | 352 | 41,964 | Editing code (M2), not view painting. Includes pptx edit points, merge shapes, crop, inline text editing, collaboration, table, chart and SmartArt edit overlays (`*-editable.ts`, `chart-title-editor.ts`), connector authoring and routing, adjustment handles, LaTeX-to-OMML input, theme and master editors, `src/core/pptx/editor/**` (despite `render/` in its path), `src/core/xlsx/edit/drawings.ts`, `src/core/geometry/shape-boolean-svg.ts`, `src/core/pptx/core/utils/raster-canvas.ts` (OLE preview regeneration on edit), xlsx cell editor, pointer, keys, menus, point mode and validation list, xlsx and docx editor-only canvas uses (`editor-core.ts` autofit, `picture-commands.ts`, `format-painter.ts`, `chart-gradient-preview.ts`). Leftover pptx `render/` editor files are listed explicitly in the classifier — there is no catch-all; unmatched files fail. |
| `X:chrome` | 213 | 25,906 | Ribbon, backstage, dialogs, galleries, icons, context menus, toolbars, title and status bars, customization, the pptx `web-components/`, and docx chrome stylesheets plus `styles/index.ts`. These are product chrome (Stage 6 or E0), not document painters. |
| `X:shell` | 60 | 8,674 | Viewer shell (Stage 6): pptx slide sorter, thumbnail pane, outline and reading views, presenter console, audience window, viewer stores and preferences, viewport fit, wheel and touch input, ruler, find, accessibility checker, autosave, password. Also xlsx hover tooltips (`grid/indicators.ts`) and the accessibility mirror (`grid/grid-a11y.ts`). |
| `X:parser` | 14 | 2,000 | Model readers that painters consume: `src/core/xlsx/read/drawing.ts`, `src/core/docx/drawing.ts`, `src/core/diagram/drawing*.ts`, pptx SmartArt drawing-part runtime and VML loader, line-dash, media-reference and bullet-marker model helpers. |
| `X:drawingml-model` | 18 | 1,876 | `src/core/drawingml/*` (matched by "draw"): colour, fill, geometry, text and theme readers, which are model inputs mapped to `nitro-xml-opc::drawingml`, not painters. |
| `X:writer` | 10 | 1,115 | Save path: `src/core/drawingml/write-*`, `src/core/docx/write-drawing.ts`, `src/core/xlsx/write/drawing.ts`, the legacy `.ppt` writer, pptx XML builders. |
| `X:visio` | 9 | 929 | Out of scope (Visio). |
| `X:diagram-engine` | 4 | 571 | `src/core/diagram/{engine,layout}` SmartArt layout interpreter (computes geometry for the save and decompose pipeline, `smartart-interpreter-drawing-bridge.ts`). It is not on the cached-drawing paint path. pptx's fallback layout is its own `render/smartart-layout-*` (P11). |

<details>
<summary>Classifier (ordered rules; run as <code>python3 -I classify.py . candidates.txt</code> from <code>upstream/</code>)</summary>

```python
#!/usr/bin/env python3
"""Assign every candidate painter file to a row or an exclusion and total production lines.

Usage: python3 -I classify.py <upstream-dir> <candidates.txt> [--json]
Rules are ordered; the first match wins. Every candidate must match exactly one rule.
"""
import collections
import json
import re
import sys

UP, CANDS = sys.argv[1], sys.argv[2]

R = []  # (regex on path, target) ; target = row id or 'X:<reason id>'


def rule(target, *patterns):
    for p in patterns:
        R.append((re.compile(p), target))


U = r'^src/ui/src/'
PR = U + r'pptx/render/'
PD = U + r'pptx/dom/'
PE = PD + r'elements/'

# ---------------------------------------------------------------- out of scope formats
rule('X:visio', r'^src/core/visio/', U + r'visio/')
rule('X:teams', r'^src/core/teams/', U + r'teams/')

# ---------------------------------------------------------------- shared
rule('S-chart', r'^src/core/chart/render/')
rule('S-smartart', U + r'smartart\.ts$', U + r'smartart/')
rule('X:drawingml-model', r'^src/core/drawingml/(?!write-)')
rule('X:writer', r'^src/core/drawingml/write-', r'^src/core/docx/write-drawing\.ts$',
     r'^src/core/xlsx/write/', r'^src/core/pptx/core/ppt/writer/', r'^src/core/pptx/core/core/builders/')
rule('X:parser', r'^src/core/xlsx/read/', r'^src/core/docx/drawing\.ts$', r'^src/core/diagram/drawing(-bounds)?\.ts$',
     r'^src/core/pptx/core/core/runtime/smartart-drawing', r'^src/core/pptx/core/core/runtime/smartart-fabrication',
     r'^src/core/pptx/core/utils/vml-drawing-loader\.ts$', r'^src/core/pptx/core/utils/drawing-media-reference\.ts$',
     r'^src/core/pptx/core/utils/drawing-line-dash\.ts$', r'^src/core/pptx/core/utils/rendered-bullet-marker\.ts$')
rule('P-image', r'^src/core/pptx/core/utils/svg-stretch-fill\.ts$')
rule('X:diagram-engine', r'^src/core/diagram/(engine|layout)/', r'^src/core/pptx/core/utils/smartart-interpreter-drawing-bridge\.ts$')
rule('X:editor', r'^src/core/xlsx/edit/', r'^src/core/pptx/editor/', r'^src/core/geometry/shape-boolean-svg\.ts$',
     r'^src/core/pptx/core/utils/raster-canvas\.ts$')

# ---------------------------------------------------------------- xlsx
X = U + r'xlsx/'
XC = r'^src/core/xlsx/ui/grid/'
rule('X-frame', X + r'grid/(grid-view|dom|styles|index)\.ts$', XC + r'(geometry|view-cache)\.ts$')
rule('X-cells', X + r'grid/(cell-layer|measure)\.ts$', XC + r'(cell-paint|cell-items|number-fit)\.ts$')
rule('X-cf', XC + r'icon-sets\.ts$')
rule('X-gridlines', X + r'grid/gridlines\.ts$')
rule('X-headers', X + r'grid/(headers|pane-chrome)\.ts$')
rule('X-selection', X + r'grid/selection-layer\.ts$')
rule('X-remote', X + r'grid/remote-layer\.ts$')
rule('X-drawings', X + r'grid/drawings\.ts$')
rule('X-sparklines', X + r'grid/sparkline\.ts$')
rule('X-smartart', X + r'grid/smartart\.ts$')
rule('X-print', X + r'print\.ts$')
rule('X-tabs', X + r'sheet-tabs/(render|index|styles)\.ts$')
rule('X:editor', X + r'grid/(alert|cell-editor|drawing-keys|formula-field|function-assist|grid-commands|grid-keys|grid-menus|grid-pointer|grid-selection|header-resize|point-mode|validation-list)\.ts$',
     XC + r'(edit-bridge|selection-ops)\.ts$', X + r'editor-core\.ts$', X + r'chart-gradient-preview\.ts$')
rule('X:shell', X + r'grid/(indicators|grid-a11y)\.ts$')
rule('X:chrome', X + r'ribbon/', X + r'dialogs/')

# ---------------------------------------------------------------- docx
D = U + r'docx/'
rule('D-print-pages', D + r'print-(layout|layout-view|fragment-hit|layout-cursor)\.ts$', D + r'canvas-measurer\.ts$',
     r'^src/core/docx/layout/(fonts|measure)\.ts$', D + r'styles/print\.css$')
rule('D-print-tables', D + r'print-table\.ts$', r'^src/core/docx/ui/table-render\.ts$', r'^src/core/docx/table-visuals\.ts$')
rule('D-print-pictures', D + r'print-picture\.ts$')
rule('D-charts', r'^src/core/docx/chart-paint\.ts$', r'^src/core/docx/ui/chart-spec\.ts$')
rule('D-print-hf', D + r'print-header-footer\.ts$')
rule('D-print-borders', D + r'print-page-borders\.ts$')
rule('D-print-linenum', D + r'print-line-numbers\.ts$')
# Draft view (D8–D11) is E3 Word editing, not M1. M1 Word is Print Layout D1–D7 plus D4.
rule('D-draft', D + r'(editor-render|schema|run-styles|run-scale|list-marker-display|paragraph-decoration|header-footer-view|notes-view|'
     r'preview-run-styles|scaled-paragraph-breaks|tab-stops-view|review-commands|review-controller|review-display|'
     r'review-options-dialog|review-paragraph-display|review-ribbon|review-run-css|review-run-marks|review-story-previews)\.ts$',
     D + r'styles/document\.css$')
rule('D-draft-pictures', D + r'image-media\.ts$')
rule('D-equations', D + r'equation-schema\.ts$')
rule('D-smartart', D + r'smartart-node-view\.ts$', r'^src/core/docx/ui/smartart-theme\.ts$', D + r'styles/smartart\.css$')
rule('X:editor', D + r'(format-painter|picture-commands)\.ts$')
rule('X:chrome', D + r'(chrome-icons|ribbon-gallery-menu|ribbon-icons)\.ts$',
     D + r'styles/(aliases|base|backstage|chrome-document|chrome-titlebar|keyboard-menu|page-navigator|'
          r'ribbon|ribbon-buttons|ribbon-gallery|ribbon-polish|ribbon-popups|ribbon-styles-group|ribbon-widgets|'
          r'status-dialogs)\.css$',
     D + r'styles/index\.ts$')

# ---------------------------------------------------------------- pptx: core converter (DOM-free SVG export)
rule('P-svg-export', r'^src/core/pptx/converter/')
rule('P-smartart-3d', r'^src/core/geometry/svg-path-(flatten|curves|types)\.ts$')

# ---------------------------------------------------------------- pptx: editor-only DOM element decorations
rule('X:editor', PE + r'(chart-editable|chart-title-editor|smartart-editable|table-editable)\.ts$')

# ---------------------------------------------------------------- pptx rows (dom/ + render/ helpers folded in)
rule('P-stage', PD + r'(slide-stage|dom|registry|types|index|presentation-rerender|load-presentation)\.ts$',
     PE + r'(index|register-extras|register-rich-media|register-table-chart)\.ts$',
     PR + r'(index|element|clone|slide-background|background-shade-to-title|template-background-rows|device-pixel-stroke|'
          r'element-visibility|element-style|element-style-transform|element-accessibility-dom|element-aria-attributes|'
          r'element-decorative|element-non-visual-description|element-action-affordance|element-action-affordance-dom|'
          r'element-actionability|field-context|grid-spacing|slide-size|css-escape|css-top-level-split|dompurify-safe|'
          r'preview-elements|color-units|secure-random|hyperlink-target|viewer-hyperlinks|hyperlink-security)\.ts$')
rule('P-wordart', PE + r'text-warp\.ts$', PD + r'glyph-outline-cache\.ts$', PR + r'text-warp\.ts$', PR + r'text-warp-', PR + r'text-(3d-|effects-3d)')
rule('P-text', PE + r'text-(block|run-content)\.ts$',
     PR + r'text-(?!format-presets|list-style-update|style-update|case-transform|segment-paragraph-break)',
     PR + r'paragraph-', PR + r'bullet-autonum', PR + r'(kinsoku-styles|unicode-script-detection|tab-leader|'
          r'linked-text-box-overflow|verified-text-rect-presets|embedded-fonts|custom-fonts|used-fonts|shape-autofit-resize)\.ts$',
     PR + r'font-(availability|catalog|embedding)\.ts$', PR + r'google-')
rule('P-shape', PE + r'(text-shape|shape-filter-defs|extrusion-overlay|reflection-mirror-content)\.ts$', PD + r'element-styles\.ts$',
     PR + r'shape-(geometry|geometry-cascade|preset-catalog|adjustment|adjustment-model|adjustment-custom-geometry)\.ts$',
     PR + r'(fill-style|path-gradient-rect|stroke-outline|stroke-paint|stroke-style|stroke-only-preset|effects-helpers|'
          r'effects-shadow-helpers|visual-effects|reflection|subpath-fill-overlay|vector-subpath-paint|custom-geometry-live-path|'
          r'crop-shape-clip|drawing-color)\.ts$', PR + r'svg-', PR + r'visual-3d')
rule('P-image', PE + r'image\.ts$', PR + r'image-(adjustments|artistic-presets|color-change|duotone-canvas|effect-corrections|'
     r'effect-filter-values|effects|fill-counter-transform|fill-overlay|native-size|source-effects|tiling)\.ts$')
rule('P-group', PE + r'group\.ts$', PR + r'group-fill\.ts$')
rule('P-connector', PE + r'connector\.ts$', PR + r'connector-(dash|elbow-geometry|markers|path|style)\.ts$')
rule('P-table', PE + r'table\.ts$', PR + r'table-(style|style-borders|style-builtins|style-builtins-data|style-cell3d|style-fill|'
     r'style-image|style-scheme|cell-css|cell-padding|cell-paragraphs|advanced-fill)\.ts$')
rule('P-chart3d', PR + r'(chart-3d-(?!interaction|mark-interaction|oblique-drag|oblique-interaction|hover-tooltip)|chart-bar3d-|chart-pie3d-|surface-chart-3d-)')
rule('P-chart', PE + r'chart(-svg)?\.ts$',
     PR + r'chart-(?!3d-|editor-|ext-editor-|ex-insert-|interaction|quick-|canvas-drag|data-grid-ops|gridlines-toggle|'
          r'schema-label-keys|subtype-options|user-shape-edit|reveal-descriptor|event-target)')
rule('P-smartart', PE + r'smartart(-svg|-fallback)?\.ts$',
     PR + r'smartart-(drawing|layout|node-label|builtin-layouts|accessibility)', PR + r'smart-art-preset')
rule('P-smartart-3d', PE + r'smartart-3d\.ts$', PR + r'smartart-3d-(?!edit-layer)', U + r'pptx/smartart-3d/')
rule('P-three-host', U + r'pptx/three-view/', PE + r'three-view(-reuse)?\.ts$')
rule('P-model3d', PE + r'model3d\.ts$', PR + r'model3d-scene')
rule('X:editor', PR + r'media-file-type\.ts$')  # insert MIME classifier; not the view painter
rule('P-media', PE + r'media(-fullscreen-overlay)?\.ts$', PR + r'media-(fullscreen|persistent-audio|playback|trim-)', PR + r'online-video\.ts$')
rule('P-ole', PE + r'ole(-icons)?\.ts$', PR + r'ole-(icon-primitives|renderer-helpers)\.ts$')
rule('P-activex', PD + r'activex-controls-overlay\.ts$', PR + r'activex-overlay-view\.ts$')
rule('P-ink', PE + r'ink(-stroke-svg)?\.ts$', PR + r'ink-(curve-sampling|rendering|stroke-view|tilt-nib|group-strokes)\.ts$')
rule('P-contentpart', PE + r'contentpart\.ts$', PR + r'content-part-strokes\.ts$')
rule('P-equation', PE + r'equation\.ts$', PR + r'(omml-|mathml-sanitize)')
rule('P-zoom', PE + r'zoom\.ts$', PR + r'(summary-zoom|zoom-return-navigation)\.ts$')
rule('P-placeholder', PE + r'placeholder\.ts$', PR + r'placeholder-prompt\.ts$')
rule('P-comments', PD + r'comment-markers\.ts$', PR + r'comment-markers\.ts$')
rule('P-slideshow', PR + r'(animation-|morph-|motion-path|p14-transition|effect-sound-|diagram-(build|drawing-shape-reveal|reveal-descriptor)|chart-reveal-descriptor|'
     r'ink-replay-timeline|cross-slide-audio|annotation-overlay|presentation-(animation|blackboard|show|visibility|hit|action|ole|subtitles|context)|'
     r'subtitle-settings|custom-shows)',
     PR + r'slide-transition-(?!edits|label-keys|options)')
rule('P-print', U + r'pptx/export/svg-print\.ts$', PR + r'presentation-print\.ts$', PR + r'notes/notes-(print|html|style-cascade|utils)\.ts$')
rule('P-raster-export', U + r'pptx/export/', PR + r'(foreign-object-|export-(clone|progress|tile-plan)|canvas-size-probe)')
rule('X:chrome', PR + r'(customization|editor-chrome|options|responsive|ribbon-galleries|ribbon-icons)/', U + r'pptx/web-components/',
     PR + r'(ribbon-|backstage|account|title-bar|status-bar|toolbar-|tab-row|chrome-|context-menu|canvas-context-menu|'
          r'section-context-menu|slide-pane-context-menu|slide-sorter-context-menu|command-search|modal-focus|flyout-|anchored-|'
          r'sheet-dismiss|mobile-|notes-toolbar|compatibility-warning|read-only-recommendation|run-program-notice|share-form|'
          r'open-file-picker|hyperlink-dialog|header-footer-dialog|inspector-)')
rule('X:shell', PR + r'(slide-sorter-|slide-pane-|outline-view\.ts|reading-view|present-|presenter-|audience-|slide-show-start|'
     r'presentation-(entry|file|keymap|session|setup|toolbar|touch)|viewer-|viewport-fit|virtualized|wheel-intent|touch-gestures|'
     r'ruler|zoom-step|hidden-slide-cue|slide-search|find-replace|accessibility|document-statistics|comments-list|comment-mentions|'
     r'session-restore|slide-compare|broadcast-|state-equality|secure-random|tag-collections|password-protection|modify-password|'
     r'deck-save|autosave-)')
# pptx/render editor surfaces (M2). Explicit list only — leftovers fail as unmatched (no catch-all).
rule('X:editor', PR + r'(edit-points|merge-shapes|picture-crop|slide-templates)/',
     PR + r'notes/',  # notes-print* already claimed by P-print
     PR + r'(action-buttons|alignment-guides|arrange-extras|bar-chart-3d-hit-test|bullet-list|bullet-toggle|bullet-toggle-selection|canvas-element-node|chart-3d-hover-tooltip|chart-3d-interaction|chart-3d-mark-interaction|chart-3d-oblique-drag)\.ts$',
     PR + r'(chart-3d-oblique-interaction|chart-canvas-drag|chart-data-grid-ops|chart-editor-options|chart-editor-support|chart-event-target|chart-ex-insert-defaults|chart-ext-editor-actions|chart-gridlines-toggle|chart-interaction|chart-interaction-mark-drag|chart-interaction-pie)\.ts$',
     PR + r'(chart-interaction-radar|chart-interaction-stacked|chart-quick-action-styles|chart-quick-action-toggles|chart-quick-actions|chart-schema-label-keys|chart-subtype-options|chart-user-shape-edit|collaboration-active-session|collaboration-assets|collaboration-broadcast-follow|collaboration-departure)\.ts$',
     PR + r'(collaboration-external-readiness|collaboration-external-session|collaboration-inline-dom|collaboration-inline-editor|collaboration-inline-input|collaboration-live-patch|collaboration-live-patch-target|collaboration-load-origin|collaboration-presence|collaboration-presence-projector|collaboration-presence-publisher|collaboration-reconcile)\.ts$',
     PR + r'(collaboration-shell-state|collaboration-sync|collaboration-sync-gate|collaboration-teardown|collaboration-text-codec|collaboration-text-lease|collaboration-text-legacy|collaboration-text-merge|collaboration-text-native-edit|collaboration-text-positions|collaboration-text-projection|collaboration-text-session)\.ts$',
     PR + r'(collaboration-text-session-apply|collaboration-text-session-delta|collaboration-text-session-paragraph|collaboration-text-snapshot-positions|collaboration-text-target|collaboration-writeback|color-contrast|color-swatches|connector-arrow-controls|connector-authoring|connector-endpoints|connector-hit-target)\.ts$',
     PR + r'(connector-reroute|connector-router|connector-router-types|connector-sites|editor-geometry|editor-gestures|editor-history|editor-image-paste|editor-insert|editor-key-target|editor-keyboard-focus|editor-keymap)\.ts$',
     PR + r'(editor-keymap-arrows|editor-keymap-chords|editor-mutations|element-action-options|element-align|element-clipboard|element-height-observer|element-highlight-click|element-hit-test|element-insertion|element-interaction|element-locks)\.ts$',
     PR + r'(element-operations|element-update-batch|equation-compile|equation-templates|eyedropper|fill-pattern-label-keys|format-helpers|format-painter|freeform-stroke-geometry|gradient-picker|group-drill|group-ops)\.ts$',
     PR + r'(hollow-shape-hit-test|image-background-removal|image-file-insertion|ink-drawing|ink-eraser-hit-test|ink-live-preview|inline-caret|inline-editor-selection|inline-list-activation|inline-list-body|inline-list-case|inline-list-controller)\.ts$',
     PR + r'(inline-list-decoration|inline-list-dom|inline-list-format|inline-list-format-style|inline-list-model|inline-list-presentation|inline-list-provenance|inline-list-reconcile|inline-list-run-reader|inline-list-run-style|inline-list-save|inline-list-seed)\.ts$',
     PR + r'(inline-list-selection|inline-list-snapshot|inline-list-style|inline-list-types|inline-selection-utils|inline-text-commit|inline-text-extract|insert-chart|latex-omml-siblings|latex-omml-symbols|latex-to-omml|latex-to-omml-commands)\.ts$',
     PR + r'(latex-to-omml-constructs|latex-to-omml-environments|layout-gallery|layout-preview|master-page-layout|master-view|master-view-crud|master-view-editing|ole-actions|ole-edit-model|outline-view-edit|paste-special)\.ts$',
     PR + r'(presentation-print-settings|recent-colors|remap-empty-paragraph|remap-paragraph-sources|remap-text|remap-text-bullets|remap-text-indents|remap-text-paragraph-match|remap-text-renumber|resize-handle-hit-area|rotate-handle-attachment|rotate-handle-dom)\.ts$',
     PR + r'(rotate-handle-placement|rotation-drag|schema-label-keys|section-operations|selection-control-artwork|selection-cycle|selection-format-state|selection-handle-visibility|selection-pane-rename|selection-transform|shape-adjustment-handles|shape-adjustment-probe)\.ts$',
     PR + r'(shape-adjustment-solver|shape-quick-styles|share-session|slide-background-patch|slide-operations|slide-size-rescale|slide-transition-edits|slide-transition-label-keys|slide-transition-options|smartart-3d-edit-layer|smartart-inline-edit|smartart-node-commit)\.ts$',
     PR + r'(smartart-node-limits|smartart-node-pane-handlers|smartart-reflow-element|smartart-reflow-to-shapes|snap-guides|stroke-dash-options|stroke-line-style-options|table-cell-edit|table-cell-merge|table-cell-pointer|table-data-grid|table-data-grid-ops)\.ts$',
     PR + r'(table-inspector|table-layout|table-merge|table-resize|table-resize-merge|table-style-editor-descriptor|table-style-editor-edit|table-style-editor-parts|table-style-map-edits|table-style-presets|template-editing|text-case-transform)\.ts$',
     PR + r'(text-format-presets|text-list-style-update|text-segment-paragraph-break|text-style-update|theme-color-field-commit|theme-color-picker-state|theme-color-swatches|theme-editor-model|theme-editor-presets|transition-preview|update-text-segment-style)\.ts$')

rows = collections.defaultdict(list)
unmatched = []
for path in open(CANDS).read().split():
    for rx, target in R:
        if rx.search(path):
            rows[target].append(path)
            break
    else:
        unmatched.append(path)


def lines(p):
    with open(f'{UP}/{p}', encoding='utf-8') as f:
        return sum(1 for _ in f)


out = {k: {'files': sorted(v), 'lines': sum(lines(p) for p in v)} for k, v in rows.items()}
if '--json' in sys.argv:
    json.dump({'rows': out, 'unmatched': unmatched}, sys.stdout, indent=1)
    sys.exit(0)
for k in sorted(out):
    print(f"{k:22} files={len(out[k]['files']):4} lines={out[k]['lines']}")
print('unmatched', len(unmatched))
for u in unmatched:
    print('  ', u)
```

</details>

### Files per row

Directories are upstream paths. Names omit `.ts` (`.css` kept). Every file listed here counts toward its row's lines.

- **D-charts** (2 files, 130 lines): `src/core/docx/`: chart-paint; `src/core/docx/ui/`: chart-spec
- **D-draft** (21 files, 2312 lines): `src/ui/src/docx/`: editor-render, header-footer-view, list-marker-display, notes-view, paragraph-decoration, preview-run-styles, review-commands, review-controller, review-display, review-options-dialog, review-paragraph-display, review-ribbon, review-run-css, review-run-marks, review-story-previews, run-scale, run-styles, scaled-paragraph-breaks, schema, tab-stops-view; `src/ui/src/docx/styles/`: document.css
- **D-draft-pictures** (1 files, 143 lines): `src/ui/src/docx/`: image-media
- **D-equations** (1 files, 39 lines): `src/ui/src/docx/`: equation-schema
- **D-print-borders** (1 files, 56 lines): `src/ui/src/docx/`: print-page-borders
- **D-print-hf** (1 files, 236 lines): `src/ui/src/docx/`: print-header-footer
- **D-print-linenum** (1 files, 47 lines): `src/ui/src/docx/`: print-line-numbers
- **D-print-pages** (8 files, 1013 lines): `src/core/docx/layout/`: fonts, measure; `src/ui/src/docx/`: canvas-measurer, print-fragment-hit, print-layout-cursor, print-layout-view, print-layout; `src/ui/src/docx/styles/`: print.css
- **D-print-pictures** (1 files, 54 lines): `src/ui/src/docx/`: print-picture
- **D-print-tables** (3 files, 294 lines): `src/core/docx/`: table-visuals; `src/core/docx/ui/`: table-render; `src/ui/src/docx/`: print-table
- **D-smartart** (3 files, 344 lines): `src/core/docx/ui/`: smartart-theme; `src/ui/src/docx/`: smartart-node-view; `src/ui/src/docx/styles/`: smartart.css
- **P-activex** (2 files, 139 lines): `src/ui/src/pptx/dom/`: activex-controls-overlay; `src/ui/src/pptx/render/`: activex-overlay-view
- **P-chart** (143 files, 20387 lines): `src/ui/src/pptx/dom/elements/`: chart-svg, chart; `src/ui/src/pptx/render/`: chart-area-fill, chart-auto-title, chart-axis-crossing, chart-axis-nice, chart-axis-primitives, chart-axis-range, chart-axis-render, chart-axis-style, chart-axis, chart-bar-cluster-geometry, chart-blank-display, chart-box-whisker-stats, chart-box-whisker, chart-build, chart-cartesian-area, chart-cartesian-axes, chart-cartesian-bars, chart-cartesian-bubbles, chart-cartesian-line-area, chart-cartesian-percent-stacked, chart-cartesian-plots, chart-cartesian-stacked-labels, chart-cartesian, chart-category-axis, chart-category-labels, chart-category-position, chart-combo-classify, chart-combo-series, chart-combo-stock, chart-combo, chart-cx-axis-units, chart-data-label-anchor, chart-data-label-callout, chart-data-label-text, chart-data-table-metrics, chart-data-table-render, chart-datapoint-picture-fills, chart-datapoint-picture-resolve, chart-datapoint-style, chart-date-axis, chart-date-format, chart-distribution-range, chart-distribution, chart-error-bars, chart-font, chart-funnel-sunburst, chart-gradient-defs, chart-helper-lines, chart-helpers, chart-histogram-binning, chart-histogram, chart-horizontal-axis, chart-horizontal-bars-helpers, chart-horizontal-bars, chart-label-lines, chart-label-measure, chart-legend-build, chart-legend-entries, chart-legend-layout, chart-legend-placement, chart-legend-swatch, chart-line-path, chart-manual-layout, chart-map-projection, chart-marker-shape, chart-number-format-pattern, chart-number-format, chart-ofpie-secondary, chart-ofpie-split, chart-ofpie, chart-overlays-axis-titles, chart-overlays-regression, chart-overlays-trendline-fit, chart-overlays-trendline, chart-overlays, chart-palette, chart-pareto, chart-picture-pattern-def, chart-pie-best-fit, chart-pie-label-collision, chart-pie-label-helpers, chart-pie-labels, chart-plot-clip, chart-radar-geometry, chart-region-map-alias, chart-region-map-colors, chart-region-map-data, chart-region-map-fallback-table, chart-region-map-view, chart-scatter-x-axis, chart-secondary-axis, chart-series-line-style, chart-sparkline, chart-stacked-series, chart-stock-candles, chart-stock-close-label, chart-style-defaults, chart-style-palette, chart-sunburst-hierarchy, chart-sunburst-view, chart-surface-axes, chart-surface-bands, chart-surface-common, chart-surface-flat, chart-surface-isometric, chart-surface-legend, chart-surface-treemap, chart-svg-def-types, chart-svg-primitives, chart-text-defaults, chart-title-band, chart-title-runs, chart-title-style, chart-treemap-hierarchy, chart-treemap-squarify, chart-treemap-view, chart-trendline-defaults, chart-trendline-equation-format, chart-trendlines, chart-user-shape-group-child, chart-user-shape-overlay, chart-user-shape-row-frame, chart-user-shape-row-transform, chart-user-shape-tree, chart-view-model-bars, chart-view-model-build, chart-view-model-chrome, chart-view-model-dom-helpers, chart-view-model-dom, chart-view-model-kinds, chart-view-model-layout, chart-view-model-manual, chart-view-model-pie, chart-view-model-points, chart-view-model-radar, chart-view-model-scale, chart-view-model-types, chart-view-model, chart-waterfall-layout, chart-waterfall-map, chart-waterfall-view
- **P-chart3d** (37 files, 5502 lines): `src/ui/src/pptx/render/`: chart-3d-bar-mesh, chart-3d-chrome-overlay, chart-3d-depth, chart-3d-mesh-highlight, chart-3d-oblique-bars, chart-3d-oblique-labels, chart-3d-oblique-layout, chart-3d-oblique-shape-mesh, chart-3d-oblique-types, chart-3d-persp-bars, chart-3d-persp-gridlines, chart-3d-persp-labels, chart-3d-persp-layout, chart-3d-persp-mark-set, chart-3d-persp-marks, chart-3d-persp-mesh, chart-3d-persp-oblique, chart-3d-persp-scene, chart-3d-persp-shape-mesh, chart-3d-persp-surface-mesh, chart-3d-persp-surface, chart-3d-persp-view, chart-3d-pie-layout, chart-3d-pie-marks, chart-3d-pie-scene, chart-3d-projection, chart-3d-shading, chart-3d-spec, chart-3d-surfaces, chart-3d-three-loader, chart-3d-view-scene, chart-bar3d-face-picture-sample, chart-bar3d-face-picture, chart-bar3d-series-depth, chart-pie3d-tilt, surface-chart-3d-geom, surface-chart-3d-walls
- **P-comments** (2 files, 141 lines): `src/ui/src/pptx/dom/`: comment-markers; `src/ui/src/pptx/render/`: comment-markers
- **P-connector** (6 files, 912 lines): `src/ui/src/pptx/dom/elements/`: connector; `src/ui/src/pptx/render/`: connector-dash, connector-elbow-geometry, connector-markers, connector-path, connector-style
- **P-contentpart** (2 files, 126 lines): `src/ui/src/pptx/dom/elements/`: contentpart; `src/ui/src/pptx/render/`: content-part-strokes
- **P-equation** (7 files, 134 lines): `src/ui/src/pptx/dom/elements/`: equation; `src/ui/src/pptx/render/`: mathml-sanitize, omml-color, omml-to-latex-helpers, omml-to-latex-layout, omml-to-latex, omml-to-mathml
- **P-group** (2 files, 221 lines): `src/ui/src/pptx/dom/elements/`: group; `src/ui/src/pptx/render/`: group-fill
- **P-image** (14 files, 2675 lines): `src/core/pptx/core/utils/`: svg-stretch-fill; `src/ui/src/pptx/dom/elements/`: image; `src/ui/src/pptx/render/`: image-adjustments, image-artistic-presets, image-color-change, image-duotone-canvas, image-effect-corrections, image-effect-filter-values, image-effects, image-fill-counter-transform, image-fill-overlay, image-native-size, image-source-effects, image-tiling
- **P-ink** (7 files, 1187 lines): `src/ui/src/pptx/dom/elements/`: ink-stroke-svg, ink; `src/ui/src/pptx/render/`: ink-curve-sampling, ink-group-strokes, ink-rendering, ink-stroke-view, ink-tilt-nib
- **P-media** (9 files, 1373 lines): `src/ui/src/pptx/dom/elements/`: media-fullscreen-overlay, media; `src/ui/src/pptx/render/`: media-fullscreen, media-persistent-audio, media-playback, media-trim-fade-scheduler, media-trim-range, media-trim-timeline, online-video
- **P-model3d** (4 files, 831 lines): `src/ui/src/pptx/dom/elements/`: model3d; `src/ui/src/pptx/render/`: model3d-scene-apply, model3d-scene-descriptor, model3d-scene
- **P-ole** (4 files, 602 lines): `src/ui/src/pptx/dom/elements/`: ole-icons, ole; `src/ui/src/pptx/render/`: ole-icon-primitives, ole-renderer-helpers
- **P-placeholder** (2 files, 125 lines): `src/ui/src/pptx/dom/elements/`: placeholder; `src/ui/src/pptx/render/`: placeholder-prompt
- **P-print** (5 files, 1100 lines): `src/ui/src/pptx/export/`: svg-print; `src/ui/src/pptx/render/notes/`: notes-html, notes-print, notes-style-cascade, notes-utils
- **P-raster-export** (16 files, 2070 lines): `src/ui/src/pptx/export/`: canvas-color-fix, canvas-jpeg, canvas-size-probe, html2canvas-clone, rasterize-element-strategy, rasterize-element-tiled-canvas, rasterize-element, rasterize-foreign-object; `src/ui/src/pptx/render/`: canvas-size-probe, export-clone, export-progress, export-tile-plan, foreign-object-font-embed, foreign-object-image-embed, foreign-object-style-inline, foreign-object-svg-document
- **P-shape** (55 files, 12229 lines): `src/ui/src/pptx/dom/`: element-styles; `src/ui/src/pptx/dom/elements/`: extrusion-overlay, reflection-mirror-content, shape-filter-defs, text-shape; `src/ui/src/pptx/render/`: crop-shape-clip, custom-geometry-live-path, drawing-color, effects-helpers, effects-shadow-helpers, fill-style, path-gradient-rect, reflection, shape-adjustment-custom-geometry, shape-adjustment-model, shape-adjustment, shape-geometry-cascade, shape-geometry, shape-preset-catalog, stroke-only-preset, stroke-outline, stroke-paint, stroke-style, subpath-fill-overlay, svg-gradient-paint, svg-gradient-rect-path, svg-path-flatten, svg-stroke-pattern-paint, svg-text-lines, vector-subpath-paint, visual-3d-bevel-light, visual-3d-bevel-lighting-layer, visual-3d-bevel-lighting-material, visual-3d-bevel-lighting-profile, visual-3d-bevel-lighting-routing, visual-3d-bevel-lighting-tables, visual-3d-bevel-lighting, visual-3d-camera-fov, visual-3d-camera-homography-math, visual-3d-camera-homography, visual-3d-camera-override, visual-3d-camera-parametric, visual-3d-camera-presets, visual-3d-camera, visual-3d-color, visual-3d-constants, visual-3d-extrusion-panels, visual-3d-extrusion, visual-3d-materials, visual-3d-panel-depth-skew-map, visual-3d-panel-quad, visual-3d-panel-sides-perspective, visual-3d-panel-sides, visual-3d, visual-effects
- **P-slideshow** (172 files, 29944 lines): `src/ui/src/pptx/render/`: animation-advanced-triggers-end, animation-advanced-triggers-shared, animation-advanced-triggers, animation-after-effect-authoring, animation-after-effect, animation-attribute-geometry, animation-attribute-transform, animation-authoring, animation-behavior-accumulate, animation-behavior-keyframes, animation-behavior-player, animation-behavior-timing, animation-behavior-values, animation-bookmark-trigger, animation-bounce-end, animation-build, animation-circle-iris, animation-color-base-style, animation-color, animation-css, animation-direction-options, animation-directional, animation-easing, animation-effects, animation-emphasis-blink-shimmer, animation-emphasis-ground-truth-early, animation-emphasis-ground-truth-late, animation-emphasis-ground-truth-types, animation-emphasis-ground-truth, animation-fill-repeat, animation-filter-effects, animation-filter-random, animation-keyframes-emphasis-base, animation-keyframes-entrance, animation-keyframes-exit-shapes, animation-keyframes-exit, animation-keyframes-motion-family, animation-keyframes-rotation-family, animation-keyframes-scale-spin, animation-keyframes, animation-mask-hole-reveal, animation-mask-reveal, animation-media-bookmark-gating, animation-media-commands, animation-media-end-gating, animation-media-playback, animation-motion-chain-keyframes, animation-motion-path-chain, animation-motion-path-paced, animation-motion-path, animation-parallel-composition, animation-pixelate-filter, animation-playback-engine, animation-playback-seek, animation-playback-steps, animation-playback, animation-ppt-formula, animation-preset-ground-truth, animation-preset-labels, animation-presets-emphasis, animation-presets-entrance, animation-presets-exit, animation-presets-extended, animation-presets-subtypes, animation-presets, animation-preview-behavior, animation-preview, animation-render-context, animation-ribbon-preview, animation-sampled-mask-keyframes, animation-sequence-gating, animation-sequencer, animation-slide-filter, animation-sound-authoring, animation-strips-reveal, animation-target-id, animation-text-style-css, animation-text-style-resolve, animation-text-style-state, animation-timeline-absolute, animation-timeline-build-descriptors, animation-timeline-build-level, animation-timeline-builder-helpers, animation-timeline-builder, animation-timeline-effect-names, animation-timeline-engine, animation-timeline-group-helpers, animation-timeline-group, animation-timeline-helpers, animation-timeline-regular-step, animation-timeline-rows, animation-timeline-sequence-builder, animation-timeline-step-effect, animation-timeline-step-scheduling, animation-timeline-step, animation-timeline-text-build-pieces, animation-timeline-text-build-ripple, animation-timeline-text-build, animation-timeline-text-range, animation-timeline-types, animation-transform-keyframes, animation-wedge-reveal, annotation-overlay, chart-reveal-descriptor, cross-slide-audio, custom-shows, diagram-build, diagram-drawing-shape-reveal, diagram-reveal-descriptor, effect-sound-catalogue, effect-sound-dsp, effect-sound-generators, effect-sound-synth, effect-sound-wav-encoder, ink-replay-timeline, morph-animation, morph-color, morph-crossfade-group, morph-flatten, morph-geometry-interp, morph-geometry-keyframes, morph-geometry, morph-heuristics, morph-image-crop, morph-matching, morph-media-assignment, morph-name, morph-overlay-order, morph-plan, morph-predicates, morph-svg-path, morph-text-slot, morph-text-tokens, morph-text, morph-types, morph-z-order, motion-path-authoring, motion-path-geometry, motion-path-presets, p14-transition-css-directional, p14-transition-css, p14-transition-keyframes, presentation-action, presentation-animation-controller, presentation-blackboard, presentation-context-menu, presentation-hit-test, presentation-ole-verb, presentation-show-order, presentation-subtitles, presentation-visibility, slide-transition-box, slide-transition-cinematic-mirror, slide-transition-cinematic, slide-transition-css, slide-transition-directional-fx, slide-transition-directional-shapes, slide-transition-fragment-types, slide-transition-fragments-grid, slide-transition-fragments-panels, slide-transition-fragments-radial, slide-transition-fragments, slide-transition-keyframes, slide-transition-prism, slide-transition-rotate, slide-transition-sound-playback, slide-transition-sound, slide-transition-types, slide-transition-warp, slide-transition-wheel-keyframes, slide-transition-wipe-keyframes, subtitle-settings
- **P-smartart** (31 files, 3708 lines): `src/ui/src/pptx/dom/elements/`: smartart-fallback, smartart-svg, smartart; `src/ui/src/pptx/render/`: smart-art-preset-data, smart-art-presets, smartart-accessibility, smartart-builtin-layouts, smartart-drawing-gradient, smartart-drawing-label-color, smartart-drawing-viewbox, smartart-drawing, smartart-layout-families-extra, smartart-layout-families-flow, smartart-layout-families, smartart-layout-family-map, smartart-layout-helpers, smartart-layout-interpreter-aux, smartart-layout-interpreter-composite, smartart-layout-interpreter-cycle, smartart-layout-interpreter-fidelity-fixtures, smartart-layout-interpreter-fidelity-geometry, smartart-layout-interpreter-flow, smartart-layout-interpreter-hierarchy, smartart-layout-interpreter-linear, smartart-layout-interpreter-model, smartart-layout-interpreter-pyramid, smartart-layout-interpreter-render, smartart-layout-interpreter, smartart-layout-types, smartart-layout, smartart-node-label
- **P-smartart-3d** (35 files, 4225 lines): `src/core/geometry/`: svg-path-curves, svg-path-flatten, svg-path-types; `src/ui/src/pptx/dom/elements/`: smartart-3d; `src/ui/src/pptx/render/`: smartart-3d-bevel-profile, smartart-3d-drawing-model, smartart-3d-drawing-solid, smartart-3d-element, smartart-3d-geom, smartart-3d-layout-source, smartart-3d-light-rig, smartart-3d-lighting, smartart-3d-model, smartart-3d-primitive-outline, smartart-3d-regenerated-drawing, smartart-3d-ring, smartart-3d-scene-camera, smartart-3d-solid-geometry, smartart-3d-solid-types, smartart-3d-solid, smartart-3d-spatial, smartart-3d-style-path, smartart-3d-text-extrusion, smartart-3d-translucency, smartart-3d-types, smartart-3d-vertex-shading; `src/ui/src/pptx/smartart-3d/`: connector-object, flat-mesh-object, lit-mesh-object, lit-text-object, text-block-layout, text-block-texture, view-camera, view-overflow, view-scene
- **P-stage** (40 files, 6382 lines): `src/ui/src/pptx/dom/`: dom, index, load-presentation, presentation-rerender, registry, slide-stage, types; `src/ui/src/pptx/dom/elements/`: index, register-extras, register-rich-media, register-table-chart; `src/ui/src/pptx/render/`: background-shade-to-title, clone, color-units, css-escape, css-top-level-split, device-pixel-stroke, dompurify-safe, element-accessibility-dom, element-action-affordance-dom, element-action-affordance, element-actionability, element-aria-attributes, element-decorative, element-non-visual-description, element-style-transform, element-style, element-visibility, element, field-context, grid-spacing, hyperlink-security, hyperlink-target, index, preview-elements, secure-random, slide-background, slide-size, template-background-rows, viewer-hyperlinks
- **P-svg-export** (10 files, 2448 lines): `src/core/pptx/converter/`: ShapeImageRenderer, ShapeTextRenderer, SlideMetadataRenderer, SvgExporter, TextSegmentRenderer, svg-chart-extended, svg-image-effect-values, svg-image-effects, svg-rich-elements, svg-text-lines
- **P-table** (13 files, 2551 lines): `src/ui/src/pptx/dom/elements/`: table; `src/ui/src/pptx/render/`: table-advanced-fill, table-cell-css, table-cell-padding, table-cell-paragraphs, table-style-borders, table-style-builtins-data, table-style-builtins, table-style-cell3d, table-style-fill, table-style-image, table-style-scheme, table-style
- **P-text** (66 files, 11780 lines): `src/ui/src/pptx/dom/elements/`: text-block, text-run-content; `src/ui/src/pptx/render/`: bullet-autonum-scripts, bullet-autonum, custom-fonts, embedded-fonts, font-availability, font-catalog, font-embedding, google-fonts-catalogue, google-fonts-lookup, google-webfonts-link, google-webfonts-metric-clones, google-webfonts, kinsoku-styles, linked-text-box-overflow, paragraph-bullet-marker-style, paragraph-geometry-overrides, paragraph-run-build, paragraph-run-enrich, paragraph-run-text, paragraph-spacing, paragraph-strut, paragraph-trailing-space, paragraph-types, shape-autofit-resize, tab-leader, text-advanced, text-autofit-shrink-measure, text-autofit-shrink, text-block-style, text-body-layout, text-body-rect, text-build-spans, text-decoration, text-east-asian-breaks, text-effects, text-field-datetime, text-field-substitution, text-fill, text-font-alignment, text-line-height, text-metric-tracking, text-outline-dash, text-overflow-lines, text-paragraph-style, text-paragraphs, text-run-decoration, text-run-effects, text-run-gradient-span, text-run-hollow, text-run-meta, text-run-ruby, text-run-spacing, text-run-style, text-script-fonts, text-style-helpers, text-style-scope, text-tab-decimal, text-tab-layout, text-tab-run-build, text-theme, text-wrap-estimate, unicode-script-detection, used-fonts, verified-text-rect-presets
- **P-three-host** (13 files, 1943 lines): `src/ui/src/pptx/dom/elements/`: three-view-reuse, three-view; `src/ui/src/pptx/three-view/`: chart-events, element, export-snapshot, index, renderer-host, scene-registry, types, view-controller, view-overflow, view-size, view-spec
- **P-wordart** (22 files, 4205 lines): `src/ui/src/pptx/dom/`: glyph-outline-cache; `src/ui/src/pptx/dom/elements/`: text-warp; `src/ui/src/pptx/render/`: text-warp, text-3d-fields, text-3d-presets, text-effects-3d, text-warp-envelope-block, text-warp-envelope-curves, text-warp-envelope-layout, text-warp-envelope-map, text-warp-envelope-measure, text-warp-envelope-types, text-warp-glyph-outline, text-warp-glyph-slicing, text-warp-glyph-trace-contour, text-warp-glyph-trace, text-warp-opentype-parser, text-warp-outline-font-cache, text-warp-outline-webfont-fetch, text-warp-preset-definitions, text-warp-preset-sampler, text-warp-presets
- **P-zoom** (3 files, 585 lines): `src/ui/src/pptx/dom/elements/`: zoom; `src/ui/src/pptx/render/`: summary-zoom, zoom-return-navigation
- **S-chart** (21 files, 2121 lines): `src/core/chart/render/`: chart-appearance, chart-colors, chart-scale, chart-spacing, chart-svg-appearance, chart-svg-bars, chart-svg-cartesian, chart-svg-legend, chart-svg-radial, chart-svg-text-metrics, chart-svg-title-text, chart-svg-util, chart-svg, chart-title-text, chart-value-axis, chart-view, index, render-space, summary-from-space, summary, theme-palette
- **S-smartart** (3 files, 320 lines): `src/ui/src/`: smartart; `src/ui/src/smartart/`: smartart-svg, smartart
- **X-cells** (5 files, 679 lines): `src/core/xlsx/ui/grid/`: cell-items, cell-paint, number-fit; `src/ui/src/xlsx/grid/`: cell-layer, measure
- **X-cf** (1 files, 137 lines): `src/core/xlsx/ui/grid/`: icon-sets
- **X-drawings** (1 files, 265 lines): `src/ui/src/xlsx/grid/`: drawings
- **X-frame** (6 files, 1046 lines): `src/core/xlsx/ui/grid/`: geometry, view-cache; `src/ui/src/xlsx/grid/`: dom, grid-view, index, styles
- **X-gridlines** (1 files, 53 lines): `src/ui/src/xlsx/grid/`: gridlines
- **X-headers** (2 files, 155 lines): `src/ui/src/xlsx/grid/`: headers, pane-chrome
- **X-print** (1 files, 173 lines): `src/ui/src/xlsx/`: print
- **X-remote** (1 files, 72 lines): `src/ui/src/xlsx/grid/`: remote-layer
- **X-selection** (1 files, 143 lines): `src/ui/src/xlsx/grid/`: selection-layer
- **X-smartart** (1 files, 80 lines): `src/ui/src/xlsx/grid/`: smartart
- **X-sparklines** (1 files, 88 lines): `src/ui/src/xlsx/grid/`: sparkline
- **X-tabs** (3 files, 393 lines): `src/ui/src/xlsx/sheet-tabs/`: index, render, styles

