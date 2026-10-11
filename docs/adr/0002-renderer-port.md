<!-- SPDX-License-Identifier: MIT -->
# ADR 0002: Renderer port — module mapping and worker/main split (R0 (b)+(c), #35 / #36)

**Status:** Accepted for module layout and threading. R0 signed off by Head of Product (main @ de85dac); this ADR also records the post-sign-off follow-ups (layout read-back, emf/mtx licences, font source #121).  
**Date:** 2026-10-11  
**Depends on:** painter inventory (#34, merged); boundary protocol (#14, merged); loader API (#37, merged).  
**Decisions recorded from:** PRODUCT_SPEC.md §6; Head of Product review of R0 on #33 (2026-10-11); final inventory in `docs/painters.md`.

## Context

R0 ports upstream painters one to one (same drawing approach, structure, fonts and measurement). The only allowed change is that each frame's draw and DOM-building calls cross the WASM boundary as one command buffer applied by the loader. This ADR turns the inventory in `docs/painters.md` into:

1. **(#35)** the Rust module layout that renderer issues (#18, #23, #27, #28, #29) fill — names and responsibilities only; no painter code in this change.
2. **(#36)** the worker vs main-thread split each of those modules implies, given one command-buffer handoff per frame and no per-element WASM→JS calls.

Inventory already records (from #34 / HoP review):

- `image-native-size.ts` in **P5** (`a:tile` sx/sy).
- Stylesheets: `print.css` → D1–D7; `document.css` + Draft helpers → D8 (E3); `smartart.css` → D11; `styles/index.ts` is `X:chrome`.
- Draft **D8–D11** under **E3**, not M1. M1 Word is Print Layout D1–D7 plus D4.
- **P24** deferred (Jared's call). **Font source** for P3/P4 remote webfont fetch is **#121** (implemented in #126): host-configurable upstream default, internal mirror URL, remote fetch off with `font_substitution` diagnostics, or a resolver callback. See loader README `fontSource` and `nitro-xml-opc::fonts`.
- Deferred: X7, X10, P10, P12, P13, P25–P27; P17 M1 is poster only.

This mapping also assigns (HoP #35):

- `render/text-warp.ts` → **P4** (WordArt), not P3.
- `media-file-type.ts` → **`X:editor`** (insert MIME classifier), not P14.

On-screen output is almost entirely DOM / inline SVG / MathML. The only canvas-2D painter is pptx raster export (P26, off the view path). WebGL appears only on opt-in 3D rows. PLAN.md's "OffscreenCanvas in the worker" therefore has **no M1 view-path consumer**.

Head of Product locked the threading rule for #36:

> Parse, model and layout run in the worker. DOM painting and measurement run on the main thread. PowerPoint text layout stays on the main thread.

Speed testing is paused (Jared). Placement below is design reasoning from upstream's drawing technology and the `#14` / `#37` loader loop — not measured timings.

## Decision: crate layout

| Crate | Path | Role |
|---|---|---|
| `nitro-xml-render` | `rs/crates/nitro-xml-render/` | **Shared render crate.** Command-buffer types and writer; measure / layout read-back batches; shared drawing primitives; shared painters **S1** (chart SVG), **S2** (SmartArt), **S3** (EMF/WMF metafile). |
| `nitro-xml-xlsx` | `rs/crates/nitro-xml-xlsx/src/render/` | Excel grid painters (X*). |
| `nitro-xml-docx` | `rs/crates/nitro-xml-docx/src/render/` | Word Print Layout painters (D1–D7) and E3 Draft modules (D8–D11). |
| `nitro-xml-pptx` | `rs/crates/nitro-xml-pptx/src/render/` | PowerPoint slide painters (P*). |
| `nitro-xml-opc` | `rs/crates/nitro-xml-opc/src/drawingml/` | DrawingML **model** (colour, fill, geometry, theme readers). **Not** chart or SmartArt painters. |
| `nitro-xml-wasm` | `rs/crates/nitro-xml-wasm/src/` | WASM boundary; applies the per-frame command buffer (loader API #37; worker/main split in this ADR). |

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
| Measure bridge | `measure/` | Types for the browser measurement path (canvas `measureText`) and post-apply layout read-back (span rect, scrollHeight, computed style, naturalWidth) behind the buffer; runs on the **main thread** (see threading decision below). |
| Metafile (EMF/WMF) | `metafile/` | Port of upstream's `emf-converter` use via `metafile-convert.ts` (M1 for P5 pictures and P15 OLE previews). Inventory row **S3**. Licence: Apache-2.0 (see licence decisions below). |

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
| X9 | `chart.rs` | via #28 | Adapter onto S1 (`nitro-xml-render::chart`). |
| X10 | `print.rs` | **deferred** | Print HTML table. |
| X11 | `sparkline.rs` | yes (#18; split if needed) | Sparklines. |
| X12 | `smartart.rs` | via S2 | Adapter onto S2 (`nitro-xml-render::smartart`). |
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
| P3 | `text.rs` | yes (#27) | Text body, bullets, autofit, East Asian. Remote webfont fetch configurable per **#121** (default = upstream). Embedded EOT via clean-room MTX (see licence decisions). |
| P4 | `wordart.rs` | yes (#27) | WordArt warps. Includes `dom/elements/text-warp.ts` and `render/text-warp.ts`. JS lib: opentype.js → Rust font outline parse (same glyph path). Font bytes may come from #121's configured source. |
| P5 | `picture.rs` | yes (#27) | Pictures, crop, effects, `image-native-size.ts`. EMF/WMF via **S3** `nitro-xml-render::metafile`. |
| P6 | `group.rs` | yes (#27) | Group transforms and children. |
| P7 | `connector.rs` | yes (#27) | Connectors. |
| P8 | `table.rs` | yes (#27) | Tables. |
| P9 | `chart.rs` | via #28 | pptx's own 2D chart painter (kept separate from S1 per 1:1 rule). |
| P10 | `chart3d.rs` | **deferred** | Opt-in WebGL 3D charts; three.js leaves with this row. |
| P11 | `smartart.rs` | yes (#27) | pptx SmartArt (separate from S2). |
| P12 | `smartart3d.rs` | **deferred** | Opt-in WebGL SmartArt 3D. |
| P13 | `three_host.rs` | **deferred** | Shared three.js host. |
| P14 | `media.rs` | yes (#27) | Audio/video posters and playback. **`media-file-type.ts` excluded** (`X:editor`). |
| P15 | `ole.rs` | yes (#27) | OLE previews (EMF/WMF preview images via **S3** `nitro-xml-render::metafile`). |
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

Chart **model/parse** may live under format crates or `nitro-xml-opc` model modules; only the **painters** are in `nitro-xml-render` / pptx `render::chart`. Issue #28's file list is corrected to these paths (not `nitro-xml-opc/src/drawingml/chart/`).

**Split if over one session:** S1 (+ xlsx/docx adapters) first; P9 (20k lines) as its own issue.

### #29 — viewer shell

Not a painter inventory row. Shell UI under `nitro-xml-wasm::shell` consumes the same command buffer. No document painters assigned here.

## Decision: worker vs main-thread split (#36)

### Threads and memory

| Rule | Choice |
|---|---|
| Wasm threads / SharedArrayBuffer | **Off** by default (PRODUCT_SPEC Q2). One dedicated worker per document; single-threaded wasm inside it. |
| Document bytes | `ArrayBuffer` **transferred** into the worker (`WorkerDocument.open` in `packages/nitro-xml/loader/`), then copied once into wasm. |
| Crash recovery | Fresh worker (viewer reload once; editor journal replay) — PRODUCT_SPEC §5(e). |

### Where each stage runs

| Stage | Thread | Notes |
|---|---|---|
| Zip / OPC / parse | **Worker** | All formats. |
| Model (sheets, docs, slides) | **Worker** | All formats. |
| Layout (geometry, pagination, grid metrics) | **Worker**, except pptx text | xlsx layout, docx `layoutDocumentModel`, pptx non-text geometry stay in the worker (modules under the format crates above). |
| **pptx text layout** | **Main** | Upstream CSS line breaking for P3 (`nitro-xml-pptx::render::text`) and every "As P3" painter. Worker emits create/style/text opcodes; the browser lays out after apply. |
| Text measurement | **Main** | Batched per frame via loader `flushMeasureBatch` / worker `measure-results` (`packages/nitro-xml/loader/measure.js`). Types live in `nitro-xml-render::measure`. Never one wasm call per string. |
| Paint (DOM / SVG / MathML apply) | **Main** | `applyCommandBuffer` in one pass (`loader/apply.js`). Ops encoded by `nitro-xml-render::command_buffer`. |
| Stylesheet install | **Main** | Once per mount: `print/styles` (D1–D7), `draft/styles` (D8), D11/`smartart.css`. Not a per-frame handoff. |
| Image intrinsic size (P5 `image-native-size`) | **Main** | Upstream reads intrinsic pixels for `a:tile` sx/sy. One DOM/`naturalWidth` read-back when the picture loads (same main-thread read-back family as measure); worker keeps tile math in `nitro-xml-pptx::render::picture` after sizes return. |
| OffscreenCanvas in the worker | **Not used on the M1 view path** | No on-screen canvas painter. Revisit only if deferred P26 returns or post-port redesign moves paint to canvas (#92–#94). |
| Editor chrome (ribbon, dialogs) | **Main** apply, same buffer | HoP: one render path; chrome ops share `nitro-xml-render::command_buffer`. |

### Per-frame handoff (matches `#37` loader)

Worker path (`WorkerDocument.tick` / `worker-host.js`):

```
main                                      worker (wasm)
────                                      ─────────────
coalesce input ── events ────────────────► pushEventBatch
── layout-results (prior frame) ────────► setLayoutResults
take-frame ─────────────────────────────► encode MeasureBatch + LayoutReadBatch + CommandBuffer
◄── frame { commands, measureReqs, layoutReqs, strings }
run measureText for whole batch
── measure-results (Float64Array) ──────► setMeasureResults
applyCommandBuffer(commands) on main
(hold layoutReqs; answer on next tick)
```

Headless same-thread path (`DocumentSession.tick`): prior-frame layout read-back → measure batch → apply commands → take this frame's layout requests → flush events; paint/measure/layout-read still on the calling (main) thread; parse/model/layout stay in the handle.

No SharedArrayBuffer: ordinary `postMessage` transfers of typed arrays only.

### Post-apply layout read-back (batched, design)

Specified next to measureText in the loader README and `docs/boundary.md`. Kinds:

| Kind | Tag | Use |
|---|---:|---|
| SpanRect | 1 | docx `canvas-measurer.ts` span / `getBoundingClientRect` |
| ScrollHeight | 2 | pptx autofit (`text-autofit-shrink-measure.ts`, `shape-autofit-resize.ts`) |
| ComputedStyle | 3 | autofit `font-size` / `line-height` via `getComputedStyle` |
| NaturalWidth | 4 | P5 `image-native-size.ts` (`naturalWidth` / `naturalHeight`) |

Requests are keyed by node id; results are four `f64` lanes per request, one batch per frame. Loader: `packages/nitro-xml/loader/layout-readback.js`. Types: `LayoutReadBatch` in `nitro-xml-render::measure`.

### Text measurement (batched, design)

| Format | Upstream path | Placement |
|---|---|---|
| xlsx | Offscreen canvas `measureText` + width cache (X1/X2; charts may pass a host measurer into S1) | Main; worker queues `{font_id, size_px, text_id}` once per layout/scroll pass |
| docx Print Layout (D1–D7) | Canvas measurer + hidden DOM `<span>` for ligatures/small caps; re-measure on `document.fonts` `loadingdone` | Main; batch once per relayout (pages are not rebuilt on scroll) |
| pptx | CSS line breaking on main; canvas `measureText` for tracking, tabs, East Asian breaks; embedded / configurable remote (#121) / custom font loads | Main; font-face installation and measure batches stay with the loader |

Long-task and measureText cost numbers are deferred while speed testing is paused. Design intent: keep heavy parse/model/layout off the main thread so apply + one measure batch can stay short; pptx text (P3) is the exception that must stay on main to match upstream CSS breaking.

### Per-painter placement

Module paths are those in the tables above. Parse/model are always in the worker for in-scope formats.

#### xlsx (`nitro-xml-xlsx::render`)

| # | Module | Layout | Measure | Paint | Crosses each frame | M1 |
|---|---|---|---|---|---|---|
| X1 | `grid` | Worker | Owns shared measurer used by X2 (main) | Main | Events in; quadrant transforms + layer order in command buffer | Yes |
| X2 | `cells` | Worker | Main (batched) | Main | Measure SoA when wrapping/fitting; cell DOM ops in frame buffer | Yes |
| X3 | `cond_format` | Worker | None | Main | Data-bar / icon ops in frame buffer | Yes |
| X4 | `gridlines` | Worker | None | Main | Line node reuse ops | Yes |
| X5 | `headers` | Worker | None | Main | Header cell ops | Yes |
| X6 | `selection` | Worker | None | Main | Overlay ops | Yes |
| X7 | `remote_selection` | Worker | None | Main | Overlay ops | Deferred |
| X8 | `drawings` | Worker | None | Main | `<img>` / frame ops; hosts X9/X12 | Yes |
| X9 | `chart` → `nitro-xml-render::chart` | Worker | Main when host measurer | Main (SVG → DOM) | Chart markup in buffer | Yes |
| X10 | `print` | Worker | Browser table layout on main if printed | Main | Print HTML apply | Deferred |
| X11 | `sparkline` | Worker | None | Main (SVG) | Sparkline ops from X2 | Yes |
| X12 | `smartart` → `nitro-xml-render::smartart` | Worker | None | Main | Frame + S2 drawing | Yes |
| X13 | `sheet_tabs` | Worker | None | Main | Tab strip ops | Yes |

#### docx M1 Print Layout (`nitro-xml-docx::render::print`)

| # | Module | Layout | Measure | Paint | Crosses each frame | M1 |
|---|---|---|---|---|---|---|
| D1 | `print/pages` + `print/styles` | Worker | Main (batched; + rare DOM span) | Main | Relayout: full page buffer (not per scroll); `print.css` at mount | Yes |
| D2 | `print/table` | Worker | As D1 | Main | Table ops inside page buffer | Yes |
| D3 | `print/picture` | Worker | None | Main | `<img>` / placeholder ops | Yes |
| D4 | `chart` → `nitro-xml-render::chart` | Worker | Estimate in S1 | Main | Chart markup in buffer | Yes |
| D5 | `print/header_footer` | Worker | As D1 | Main | Decorate-pages ops on relayout | Yes |
| D6 | `print/page_borders` | Worker | None | Main | Border box ops | Yes |
| D7 | `print/line_numbers` | Worker | As D1 | Main | Label ops | Yes |

#### E3 Draft (`nitro-xml-docx::render::draft`)

| # | Module | Layout | Measure | Paint | Crosses each frame | M1 |
|---|---|---|---|---|---|---|
| D8 | `draft/surface` + `draft/styles` | Main (CSS / editing surface) | Main | Main | Draft DOM ops; `document.css` at mount | E3 |
| D9 | `draft/picture` | n/a | None | Main | Picture node views; hosts D4/D11 | E3 |
| D10 | `draft/equation` | Main (MathML) | Main | Main | MathML tree ops | E3 |
| D11 | `draft/smartart` → `nitro-xml-render::smartart` | n/a | None | Main | Cached drawing; `smartart.css` at mount | E3 |

#### pptx (`nitro-xml-pptx::render`)

| # | Module | Layout | Measure | Paint | Crosses each frame | M1 |
|---|---|---|---|---|---|---|
| P1 | `stage` | Worker; text as P3 | As P3 | Main | Stage + child buffers | Yes |
| P2 | `shape` | Worker; text as P3 | As P3 | Main | Shape DOM/SVG + P3 body | Yes |
| P3 | `text` | **Main** (CSS) | **Main** | Main | Dominant slide-change cost; measure for tracking/tabs/EA; font loads on main | Yes |
| P4 | `wordart` | Worker (warp params) | Main | Main | Warp path ops; glyph measure / font bytes on main | Yes |
| P5 | `picture` | Worker (crop/fill/tile math) | Intrinsic size read-back on main (`image-native-size`) | Main | `<img>` / filter ops; `naturalWidth`/`Height` once per load for `a:tile`; duotone/clrChange canvas on main | Yes |
| P6 | `group` | Worker | None | Main | Group transform + children | Yes |
| P7 | `connector` | Worker; text as P3 | As P3 | Main | Connector path + text | Yes |
| P8 | `table` | Worker; cell text as P3 | As P3 | Main | Table DOM + text | Yes |
| P9 | `chart` | Worker | Main (label boxes) | Main | Chart SVG + label measure batch | Yes |
| P10 | `chart3d` | Worker scene prep (if ported) | Estimate | Main WebGL | Separate buffer family | Deferred |
| P11 | `smartart` | Worker | Estimate in worker OK | Main | SmartArt SVG ops | Yes |
| P12 | `smartart3d` | — | Main textures | Main WebGL | — | Deferred |
| P13 | `three_host` | — | None | Main | — | Deferred |
| P14 | `media` | Worker | None | Main | Media element ops | Yes |
| P15 | `ole` | Worker | None | Main | Preview / badge ops | Yes |
| P16 | `activex` | Worker | None | Main | Placeholder ops | Yes |
| P17 | `model3d` | Worker | None | Main (poster only) | Poster `<img>`; WebGL deferred | Yes (poster) |
| P18 | `ink` | Worker | None | Main | Stroke path ops | Yes |
| P19 | `content_part` | Worker | None | Main | Part stroke ops | Yes |
| P20 | `equation` | Worker (OMML→MathML string) | Browser MathML | Main | MathML tree ops | Yes |
| P21 | `zoom` | Worker; labels as P3 | As P3 | Main | Zoom tile ops | Yes |
| P22 | `placeholder` | Worker; text as P3 | As P3 | Main | Placeholder ops | Yes |
| P23 | `comments` | Worker | None | Main | Marker ops | Yes |
| P24 | `slideshow` | — | As P3 | Main | Animation timeline ops | Deferred |
| P25 | `print` | Worker (string assembly) | As P3 if measuring | Worker string; DOM print driver later | Export strings out of worker | Deferred |
| P26 | `raster_export` | Worker plan | Font embed on main | Main canvas | Off view path | Deferred |
| P27 | `svg_export` | Worker (DOM-free) | Optional host measure; else advance-width tables in worker | Worker-only string out | No frame buffer; export API | Deferred |

#### Shared (`nitro-xml-render`)

| # | Module | Layout | Measure | Paint | Crosses each frame | M1 |
|---|---|---|---|---|---|---|
| S1 | `chart/` | Worker (DOM-free markup) | Main if host measurer; else estimate in worker | Main (parse SVG into DOM) via X9/D4/P27 | Markup in buffer or export path | Yes (via hosts) |
| S2 | `smartart/` | None (cached positions) | None | Main | Drawing ops via X12/D11 | Yes (via hosts) |

### Main-thread risk (qualitative)

Design, not measured:

- **X2 scroll** — many recycled cell DOM ops per frame after worker layout; measure round-trips only when wrap/fit needs new widths.
- **D1 relayout** — large one-shot page buffer; scroll should not re-apply pages (inventory: Print Layout scroll makes no page painter calls).
- **P3 slide change** — CSS text layout and DOM apply both on main; the dominant pptx handoff cost by call count in the inventory.

The acceptance idea "no long task over 50 ms while scrolling L-tier fixtures" stays as a future check when speed testing resumes; it is not claimed here.

## Other R0 decisions (recorded for the module map)

- **Editor chrome** uses the same command buffer as document painters.
- **Both chart painters** (S1 and P9) are kept (1:1 rule).
- **Font source (P3/P4):** **#121** landed in #126 — `fontSource` on `initCore` / `openDocument` / `WorkerDocument` (default / mirror / off / resolver); policy and `planWebfontStylesheet` in Rust.
- **P17:** poster image only in M1; WebGL "view in 3D" deferred with P13.
- **JS libraries M1 painters depend on:** DOMPurify (P1), opentype.js (P4), emf-converter → `nitro-xml-render::metafile` (S3); html2canvas only if export returns; three.js only with deferred WebGL rows. EOT embedded fonts: see MTX decision below.
- **Loader budget:** #37 (API already landed); post-apply layout read-back kinds landed with the #33 follow-up.

## Licence decisions (emf-converter / mtx-decompressor)

Recorded for Head of Product's R0 sign-off follow-ups (comment on #33). Full rows: `PROVENANCE.md` and `LICENSE-THIRD-PARTY`.

| Upstream package | Licence | Nitro XML choice | Why |
|---|---|---|---|
| [`emf-converter`](https://github.com/ChristopherVR/emf-converter) (used via `src/core/pptx/core/utils/metafile-convert.ts`) | **Apache-2.0** | Port into `nitro-xml-render::metafile` (inventory **S3**); ported files keep `SPDX-License-Identifier: Apache-2.0` + PROVENANCE rows; crate becomes `MIT AND Apache-2.0` when those files land. | Same licence family as other ooxml-derived ports; on the M1 path for EMF/WMF pictures (P5) and OLE previews (P15). |
| [`mtx-decompressor`](https://github.com/ChristopherVR/mtx-decompressor) (MPL-2.0; used for EOT via `src/core/pptx/core/utils/eot-parser.ts`) | **MPL-2.0** (file-level copyleft) | **Clean-room rewrite** under MIT in a future pptx font module (e.g. `nitro-xml-pptx::fonts::mtx` or `nitro-xml-render::mtx`). Do **not** vendor or port the MPL sources. | `deny.toml` / `scripts/ci/licenses.sh` allow only MIT, Apache-2.0, BSD, ISC, Zlib, Unicode — not MPL-2.0. A file-level MPL module would need an exception and would ship copyleft into the MIT package; clean-room keeps the allow-list green. Spec: MicroType Express / EOT container behaviour, validated differentially against upstream's EOT fixtures when #27 ports embedded fonts. |

## Consequences

- Renderer issues implement modules listed here; they do not invent parallel layouts.
- The `nitro-xml-render` crate exists (`rs/crates/nitro-xml-render/`); command-buffer, measure/layout-read-back types, **S3 `metafile/`** stub, and S1/S2 module stubs live there. Empty `render/` modules under format crates still land with the first painter port PR for each format.
- Painter ports must encode through `nitro-xml-render::command_buffer` and rely on the `#37` loader for apply + batched measure + post-apply layout read-back; no per-element WASM→JS calls.
- Opcode gaps in `docs/boundary.md` remain for painter/loader work as formats land.
- OffscreenCanvas is not part of the M1 port plan.
- #23 and #27 stay blocked until this follow-up PR merges (Head of Product); #121 is already on main (#126).
