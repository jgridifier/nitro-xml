# Nitro XML: porting ChristopherVR/ooxml to Rust and WebAssembly

> **This plan is reconciled with [PRODUCT_SPEC.md](PRODUCT_SPEC.md) v3 (full Rust/WASM design lock), which wins wherever they disagree.** Decisions: [§6.1](#61-decisions-current-calls-open-to-change). Board: [Nitro XML project](https://github.com/users/jgridifier/projects/2). Repo: [jgridifier/nitro-xml](https://github.com/jgridifier/nitro-xml). Licence: Nitro XML is MIT-licensed; files ported or derived from ooxml remain under Apache-2.0 (NOTICE, LICENSE-THIRD-PARTY, PROVENANCE.md).

**Project:** Nitro XML · **Repo:** [`jgridifier/nitro-xml`](https://github.com/jgridifier/nitro-xml) · **Rust crates:** `nitro-xml-opc` (shared foundation: `zip`, `xml`, `opc` and `drawingml` modules), `nitro-xml-xlsx`, `nitro-xml-docx`, `nitro-xml-pptx`, `nitro-xml-wasm` and the umbrella crate `nitro-xml` (all unclaimed on crates.io on 2026-10-11) · **npm package:** `nitro-xml` (unclaimed on 2026-10-11).

**Upstream analysed:** `https://github.com/ChristopherVR/ooxml` at commit `e66aaec24adc0b68472eae1fc42aa4598e09c352` (2026-10-11), from a read-only shallow clone. The repo pins this commit as the parity oracle and the source of the painters.

**Core principle (unchanged):** rendering stays fully client-side in the browser.

**Design lock (Jared, 2026-10-11):** "This is a full Rust/WASM project. If something doesn't exist we will build it." Everything that ships is Rust/WASM: package, model, layout, **renderers**, viewer shell, **editor UI** (ribbons, dialogs), **text input, selection and caret**, commands, history and writer. The only JS is a minimal loader (wasm bootstrap, event forwarding and the per-frame command-buffer applier) with its own gated size budget. ProseMirror, the upstream Lit UI and the TS painters are not shipped. The upstream TS engine is **test-only**: correctness oracle, perf baseline and pixel reference. There is no runtime fallback; if wasm cannot run, the component shows an "unsupported browser" message. The goal is the fastest, most efficient, highest-quality OOXML renderer and editor available, beating the original on every format at every size.

---

## 0. Read this first: the upstream is much larger than a "renderer"

The brief describes a browser-client renderer for docx, pptx and xlsx. The upstream repository is a **full Office suite monorepo**. It contains:

- a headless library (`ooxml-core`) that reads, edits, validates and writes Word, Excel, PowerPoint and Visio
- a Lit web-component UI (`ooxml-ui`) with ribbons, editors and dialogs
- bindings for React, Vue, Angular, Svelte, Solid and vanilla JS
- a formula engine (README: 380 to 480+ functions)
- Yjs real-time collaboration, MCP servers, an "OpenTeams" chat app, a Tauri desktop wrapper and an integrated "OOXML Office" PWA

`cloc` counts **about 1.56 million lines of TypeScript code** (excluding blank lines and comments) in 12,989 files. About half of that is tests.

**Original recommendation (superseded):** port only the read and view path. **Current scope (Jared):** the full editor suite for docx, xlsx and pptx, built viewer first (Milestone 1) and then editor (Milestone 2), all in Rust/WASM. Still out: Visio, Teams, MCP, macro execution (VBA preserved), framework bindings beyond the web component; ASK: legacy and encrypted formats (unsupported message in v1, Rust track after E5), collaboration and export (default out). This is far larger than the original 25k to 45k line estimate: the roughly 217k production lines of `src/ui` are now rebuilt in Rust as well, and no credible estimate exists until R0 (PRODUCT_SPEC Q3).

---

## 1. Architecture map

### 1.1 Language, framework, tooling

| Item | What the repo uses |
|---|---|
| Language | TypeScript (strict, with `exactOptionalPropertyTypes` and `noUncheckedIndexedAccess`). The `pptx` area is compiled with relaxed flags while it migrates (`tsconfig.pptx.json`, AGENTS.md). TS `^7.0.2`. |
| Package manager and workspace | Bun workspace with one `bun.lock`. Node >= 22. |
| Build | `tsup` and `tsdown` (core), `node scripts/build.mjs`, Vite for viewers and demos |
| Unit tests | Vitest 5 (tests next to code; `isolate: false`), `fast-check` property tests, `xmllint-wasm` ECMA-376 schema checks for docx |
| Browser tests | Playwright: 472 `*.spec.ts` under `e2e/{docx,xlsx,pptx,visio,teams}` |
| Lint and format | oxlint, oxfmt |
| CI | `.github/workflows/ci.yml` (change-aware plan via `scripts/ci-plan.mjs`, sharded tests, nightly full run) plus release, pages, scorecard, security and other workflows |
| UI tech | Lit 3 web components (`src/ui`), ProseMirror (Word editing surface), Yjs, opentype.js, DOMPurify; three.js for pptx 3D |
| Licence | Apache-2.0 (root `LICENSE`, `NOTICE`, `src/core/THIRD-PARTY-LICENSES`, `PROVENANCE.md`) |

### 1.2 Package layout

```
src/core     ooxml-core: ALL logic, DOM-free (areas: xml, opc, units, color, geometry, drawingml,
             chart, diagram, text, math, docx, xlsx, pptx, visio, collab, crypto, digest, teams, automation…)
src/ui       ooxml-ui: Lit web components; DOM only; product editors under ooxml-ui/<product>
viewers/*    docx, xlsx, pptx, visio, teams: framework bindings, docs sites, viewer MCPs
demos/*      demo apps per product (docx: demo-vanilla; pptx: react/vue/angular/svelte/vanilla…)
e2e/*        Playwright suites and fixtures
apps/office-suite, site/, desktop/ (Tauri), mcp/, scripts/, schemas/ecma-376-transitional
```

Dependency direction (from the README): bindings -> `ooxml-ui` (DOM) -> `ooxml-core` (logic) -> `ole2`, `emf-converter`, `mtx-decompressor`. The core never imports the UI. **This is the seam Nitro XML uses.**

### 1.3 Lines of code (cloc 2.04, code lines only)

"prod" excludes `*.test.*`, `*.spec.*`, `*.generated.*`, `__tests__`, `__fixtures__` and fixture folders.

| Area | All code | Prod | Port relevance |
|---|---:|---:|---|
| `src/core/xml` | 508 | 276 | **port** |
| `src/core/opc` | 3,382 | 1,682 | **port** |
| `src/core/units` / `color` | 185 / 806 | 57 / 379 | **port** |
| `src/core/drawingml` | 2,396 | 1,840 | **port** |
| `src/core/geometry` | 21,445 | 16,917 | **port** (preset shapes) |
| `src/core/chart` | 8,687 | 5,907 | optional (parsing only) |
| `src/core/xlsx` | 65,499 | 45,876 | read path only: `read/` 2,607 · `numfmt/` 1,588 · `layout/` 2,499 · `ui/` (view logic) 10,892. Not `formula/` 13,304, `edit/` 7,491, `write/` 3,908, `collab/` 1,236 |
| `src/core/docx` | 40,864 | 21,613 | read path: top-level modules ~12,040 + `layout/` 2,885. Not `ui/` 5,659 (editor logic) |
| `src/core/pptx` | 322,613 | 146,051 | static-render subset of `core/` (125,415 prod). Not `automation`, `converter`, `cli`, `editor`, `signature-node` |
| `src/core/diagram` (SmartArt) | 31,814 | 18,167 | **rebuild in Rust** (SmartArt rendering from the cached drawing; layout engine later) |
| `src/core/visio` | 73,887 | 40,076 | out of scope |
| `src/ui` (all) | 380,298 | 217,398 | **rebuilt in Rust** (renderers, shell, editor chrome; reference only). docx 27,747 · xlsx 19,775 · pptx 133,260 · visio 27,200 |
| `viewers/pptx` | 583,920 | 335,252 | not ported (framework bindings out of scope; reference only) (React 83.8k, Vue 63.5k, Angular 72.4k, Svelte 55.7k, vanilla 51.1k) |
| `viewers/docx`, `viewers/xlsx` | 4,089 / 5,042 | 3,420 / 3,542 | reference only |
| `e2e` | 70,259 | — | reused as visual references |
| Whole repo (TS) | 1,560,277 | — | |

**Realistic Rust port surface** (read path for three formats plus shared layers): roughly 25k to 45k lines of TS logic, dominated by pptx. This is an estimate, not a count.

### 1.4 Entry points (read path)

- **xlsx:** `loadXlsx(bytes)` in `src/core/xlsx/read/load.ts` (`loadWorkbook` in `xlsx/load/detect.ts` adds legacy `.xls`, CSV and passwords)
- **docx:** `loadDocx(bytes)` in `src/core/docx/parse.ts` (`loadDocument` in `docx/load/detect.ts`)
- **pptx:** `PptxHandler` (`src/core/pptx/core/PptxHandler.ts`, extending `PptxHandlerCore`)
- **docx layout:** `layoutDocumentModel(model, measurer, options)` in `src/core/docx/layout/layout.ts`
- **xlsx view:** `buildItems(input): CellItem[]` in `src/core/xlsx/ui/grid/cell-items.ts`

### 1.5 Parsing pipeline: zip, then XML, then model

1. **Zip:** `jszip` (most areas; e.g. `xlsx/read/package.ts`, `opc/package.ts`) and `fflate` (core dependency). Bounded by `MAX_INPUT_BYTES` (50 MB), `MAX_PARTS` (10,000), `MAX_UNCOMPRESSED_BYTES` (300 MB) and `MAX_PART_BYTES` (200 MB) in `xlsx/read/package.ts`.
2. **OPC:** `src/core/opc` handles `[Content_Types].xml`, relationships, part path resolution, the hyperlink safety policy, properties and signature detection. Writes use string surgery to keep untouched bytes identical.
3. **XML:** two parser families.
    - The shared `xml` area (`parseXml`) uses `@xmldom/xmldom` DOM and rejects DTD and ENTITY declarations. `xml/lite.ts` is a fast reader for large `<sheetData>` that falls back to the full parser.
    - The `pptx` area still uses `fast-xml-parser` objects (about 50 files under `pptx/core/core/runtime`). The roadmap calls moving it to the shared model "One XML model".
4. **Model:**
    - xlsx: `Workbook` with sheets, styles, shared strings and theme. Unknown parts are kept in `workbook.source.parts`.
    - docx: `DocumentModel` (`blocks`, `sections`), with media collected separately.
    - pptx: `PptxData` with a 16-kind `PptxElement` union (text, shape, connector, image, picture, table, chart, SmartArt, OLE, media, group, ink, content part, zoom, 3D model, unknown) and theme, master, layout and placeholder inheritance.

### 1.6 Rendering pipeline

| Format | Layout (DOM-free, in core) | Painting (DOM, in ui) |
|---|---|---|
| xlsx | `xlsx/layout` (grid metrics, axis metrics, viewport, visible cells) and `xlsx/ui` (cell views, colours, conditional formats, chart SVG) | `src/ui/src/xlsx/grid/cell-layer.ts`: a **DOM** painter (pooled `div` per cell, repaint only when the signature changes), SVG for diagonals, sparklines and charts. Canvas only for measuring (`grid/measure.ts`) |
| docx | `docx/layout`: pagination engine (line breaking, widow and orphan control, keep rules, tables, columns, floats, notes). Uses an **injected `TextMeasurer`** (`widthOf`, `lineHeightOf`, `ascentOf`) | Editing surface is **ProseMirror**. Print Layout (read-only, paginated) is DOM, built from `LayoutResult` (`print-layout-view.ts`, `print-layout.ts`). Canvas measurer in `canvas-measurer.ts` |
| pptx | Model and inheritance in core. Shared render logic in `src/ui/src/pptx/render` (1,619 files: charts, animation, text, SmartArt…) | Slides are **HTML, CSS and SVG in the DOM** (pptx README: "Slides are HTML, CSS and SVG in the DOM"). Canvas is used for helpers (measurement, image effects, export rasterisation). WebGL and three.js for 3D. Five framework bindings map shared descriptors to their templates |

### 1.7 Viewer UI and demo apps

- The viewers are Lit custom elements (`<docx-editor>`, `<xlsx-editor>`, the pptx web controls) with thin framework bindings.
- Demo apps: `demos/docx/demo-vanilla`, `demos/xlsx/demo-vanilla`, `demos/pptx/demo-{react,vue,angular,svelte,vanilla,three-parity}`, plus `apps/office-suite` (the integrated PWA, published at `christophervr.github.io/ooxml/`).

### 1.8 External runtime dependencies

- **`ooxml-core`:** `jszip`, `fflate`, `fast-xml-parser`, `@xmldom/xmldom`, `utif`, `emf-converter` (Apache-2.0), `mtx-decompressor` (**MPL-2.0**), `prosemirror-*`, `yjs`, `y-prosemirror`, `y-protocols`, `lib0`. Optional peers: `node-forge`, `xml-crypto`, `@napi-rs/canvas`.
- **`ooxml-ui`:** `lit`, `dompurify`, `opentype.js`, `prosemirror-*`, `yjs`, `zod`, `emf-converter`.
- `@christophervr/ole2` (Apache-2.0) is inlined in the pptx bundle.

### 1.9 Tests and fixtures

- **Test files:** 1,765 in `src/core`, 1,195 in `src/ui`, and 472 Playwright specs.
- **Fixtures:**
  - **docx:** 158 files, 3 KB to 67 KB (median 14 KB)
  - **xlsx:** 11 files, 5 KB to 23 KB
  - **pptx:** 473 files, 4 KB to 36.8 MB. This includes 229 SmartArt gallery decks and large real-world decks of 1.8 MB to 36.8 MB in `e2e/pptx/fixtures`.
- **Gap:** there are no large docx or xlsx files, so the perf track has to generate them.

### 1.10 Activity and issues

- The GitHub repo was created on 2026-10-01 (it imports older viewer repositories with their history) and was last pushed on 2026-10-11. It has 11 stars, **0 open issues**, 15 closed issues and 35 PRs.
- Recent closed issues are mostly xlsx editing semantics (filters, structured references, SUBTOTAL) and pptx fidelity (Morph, line spacing, thin strokes).
- **It is a fast-moving target.** Pin a commit.

---

## 2. Feature coverage per format (from the READMEs and parity docs)

**xlsx** (`viewers/xlsx/README.md`, `docs/xlsx-parity-status.md`, updated 2026-10-08)
- **Renders:**
  - grid with frozen panes
  - fonts, fills, borders, alignment, number formats, merges, column and row sizes
  - conditional formatting (rules, colour scales, data bars, icon sets) and validation lists
  - common charts as SVG (8 families), pictures and sparklines
- **File types:** `.xlsx`, `.xlsm`, `.xltx`, legacy `.xls` and CSV.
- **Gaps:**
  - pivot tables, slicers, external links and Power Query are kept but not shown
  - shapes, SmartArt and form controls show as placeholders (SmartArt is drawn from the cached drawing)
  - no iterative calculation
- **The project's own assessment:** "Full 1:1 Microsoft Excel parity has not been achieved". There is no lossless saving and no whole-workbook pixel equivalence.

**docx** (`docs/word-parity-review.md`, `viewers/docx/docs/outstanding-work.md`)
- **Renders:**
  - paragraphs, runs and character styles, theme colours and fonts
  - numbering and lists
  - sections, breaks, columns, and read-only headers, footers and notes
  - tables (grid, merges, borders, shading, styles), inline pictures, hyperlinks and bookmarks
  - tracked changes and comments
  - charts drawn read-only for bar, column, line, area, pie, doughnut, scatter and radar
  - equations as read-only MathML (Print Layout shows `[Equation]`)
- **Gaps:**
  - Print Layout is read-only and editing is continuous
  - complex typography, floats and Word font metrics are incomplete
  - floating and unsupported drawings show as placeholders
  - floating (`wp:anchor`) charts are not placed in Print Layout
  - bubble, stock, surface and chartex charts show only a labelled frame
  - no general shape or SmartArt editing

**pptx** (`viewers/pptx/README.md`, `docs/guide/limitations.md`)
- **Renders:**
  - 187 preset shapes, custom geometry
  - gradient, pattern and picture fills, shadows, glow, reflection, soft edges, 3D bevels and cameras, WordArt warps
  - tables, SVG charts, SmartArt, EMF and WMF, embedded fonts, ink, audio, video and GLB models
  - transitions and animations
- **Known gaps:**
  - SmartArt re-layout without a cached drawing is approximate (228 of 229 gallery fixtures produce the same shape set as PowerPoint)
  - metal 3D materials have a residual error
  - some WordArt `can` presets are off at certain depths
  - pie `bestFit` label spacing is approximated
  - 3D model camera details are unverified
  - saving an edited slide can still change minor markup
- At runtime, unsupported features are reported in `data.warnings`.

**What this means for Nitro XML:** the TS renderers are mature and tuned against Office. Under the design lock they are rebuilt in Rust, so they become the **pixel reference**: every Rust renderer must re-earn the visual thresholds format by format (pptx effects especially).

---

## 3. Port design

### 3.1 What is built in Rust/WASM (everything that ships)

| Layer | Decision | Notes |
|---|---|---|
| Zip, limits | **Rust** (`nitro-xml-opc::zip`) | Streaming, lazy per part, upstream limits enforced |
| XML | **Rust** (`nitro-xml-opc::xml`) | One streaming pass, no DOM tree; SIMD128 hot loops |
| OPC, DrawingML, charts | **Rust** (`nitro-xml-opc`) | Shared by all formats |
| Models (xlsx, docx, pptx, all element kinds) | **Rust** | Columnar where it matters; lazy per sheet, page and slide |
| Layout (grid metrics, pagination, slide text) | **Rust** | Fonts and text measurement follow upstream (batched); in-wasm shaping is post-port (#93) |
| **Renderers** | **Rust** | Straight one-to-one ports of the upstream painters (canvas stays canvas, DOM stays DOM); only change: one batched draw/DOM handoff per frame. Inventory and mapping at R0 (#33 to #35) |
| Viewer shell (zoom, navigation, find, copy, links, diagnostics) | **Rust** | Stage 6 |
| Editor chrome (ribbons, dialogs, menus, shortcuts) | **Rust** | E0 |
| Text-editing surface (caret, selection, IME, bidi, clipboard, spellcheck, a11y, mobile keyboards) | **Rust** | E-text, before E1 (single-line) and E3 (rich) |
| Commands, undo/redo, lossless writer, formula engine | **Rust** | E1 to E4; formula engine E2 |
| Loader | **JS, minimal** | wasm bootstrap, streaming compile, cached module, pre-warmed worker, event forwarding; own size gate (#37) |
| Upstream TS engine | **Test and bench harness only** | Oracle, perf baseline, pixel reference; CI fails if a shipped artefact imports it (#38) |

### 3.2 The interface between JS and WASM

- **The loader is the only JS.** It instantiates the wasm (streaming compile, cached compiled module, pre-warmed worker), loads a small core module and per-format modules on demand, and forwards batched browser events (input, pointer, wheel, keyboard, composition, resize) into wasm. Its size has its own ceiling (#37).
- **Handles.** The headless API exposes document handles that own the model in linear memory (`open → handle`, queries, commands, `save`, `free`), with a `FinalizationRegistry` safety net.
- **Bulk transfer, never per-cell calls** (#14): struct-of-arrays typed-array views over wasm memory for bulk data, interned string tables, `Uint8Array` views for media.
- **Per-frame command buffer.** Each frame's canvas draw calls or DOM-building operations cross the boundary as one batched handoff applied by the loader; no per-element WASM-to-JS calls in the render path. Text measurement follows upstream, batched per layout pass (#14).
- **Workers:** one worker per document, `ArrayBuffer` transferred. The split follows upstream's drawing technology per painter: DOM-building painters apply on the main thread, canvas painters may use OffscreenCanvas in the worker (R0, #36).
- **Memory:** linear memory never shrinks; one worker per document; upstream limits enforced; crash recovery reloads in a fresh worker (viewer) or replays the op journal (editor).
- **Threads:** none by default (no SharedArrayBuffer, no COOP/COEP needed by embedders). Only the perf harness serves COOP/COEP.

### 3.3 Rust crates considered

All versions and licences are from the crates.io API on 2026-10-11.

| Crate | Version / licence | Use |
|---|---|---|
| `zip` (zip-rs/zip2) | 9.0.1 / MIT | **Use**, `default-features=false`, deflate only (no C deps) |
| `miniz_oxide` / `flate2` | 0.9.1 / MIT, Zlib or Apache-2.0; 1.1.10 / MIT or Apache-2.0 | Deflate backend |
| `quick-xml` | 0.42.0 / MIT | **Use** as the streaming parser (`NsReader`) |
| `roxmltree` | 0.21.1 / MIT or Apache-2.0 | Optional read-only tree for small parts |
| `serde`, `serde_json` | 1.0.x / MIT or Apache-2.0 | Model serialisation, parity dumps |
| `wasm-bindgen`, `js-sys`, `web-sys` | 0.2.129 / 0.3.106, MIT or Apache-2.0 | Boundary |
| `serde-wasm-bindgen` | 0.6.5 / MIT | Candidate transport (last release 2024-02) |
| `tsify` | 0.5.8 / MIT or Apache-2.0 | `.d.ts` generation (maintained again; `tsify-next` was a fork) |
| `wasm-pack` | 0.15.0 / MIT or Apache-2.0 | Build (now under the `wasm-bindgen` org) |
| `wasm-bindgen-test`, `insta` | 0.3.79 / 1.49.0 (Apache-2.0) | Tests, snapshots |
| `unicode-linebreak`, `unicode-segmentation` | Apache-2.0; MIT or Apache-2.0 | Line breaking (only where it matches upstream behaviour) |
| `docx-rs` | 0.4.22 / MIT | **Not adopted.** Its own model, writer-oriented; parity with the upstream `DocumentModel` would need a translation layer as big as a port |
| `calamine` | 0.36.1 / MIT | **Not adopted.** Data-oriented reader ("simple enough" files); not the styles, conditional formats and view data the grid needs. Useful as a cross-check oracle for cell values |
| `umya-spreadsheet` | 3.1.0 / MIT | Not adopted (full read and write model; heavy for WASM). Possible reference |
| `ooxmlsdk` | 0.13.2 / MIT or Apache-2.0 | Generated schema types modelled on the .NET Open XML SDK. **Worth a 1-day spike** for docx and pptx typed parts. Risk: very young (about 30k downloads), and large generated code may bloat the WASM binary |
| `rustybuzz`, `swash`, `parley`, `cosmic-text` | MIT and/or Apache-2.0 | Deferred (in-WASM shaping) |
| `tiny-skia` / `resvg` | BSD-3-Clause / MIT or Apache-2.0 | Not needed (painting stays DOM) |

**Licence constraints:**
- **Nitro XML is dual-licensed `MIT OR Apache-2.0`** (the Rust ecosystem convention; Jared asked for MIT). The repo ships `LICENSE-MIT` and `LICENSE-APACHE`; Cargo metadata and `package.json` say `license = "MIT OR Apache-2.0"`.
- Upstream is Apache-2.0, so ported logic is a derivative work. Apache-2.0 §4 allows distributing derivative works under other terms, provided its conditions are met: `NOTICE` crediting upstream (ChristopherVR/ooxml) is carried forward, copyright notices are kept, and changes are stated in `PROVENANCE.md` (one entry per ported or derived module, mirroring upstream's own convention).
- `mtx-decompressor` is MPL-2.0 (file-level copyleft). It is not in scope for v1 and the licence check does not allow MPL.
- jszip is dual MIT or GPL-3.0 and is used under MIT. It is not ported; Rust uses `zip`.
- `font-advance-widths*.generated.ts` holds glyph widths measured in PowerPoint through COM (NOTICE). It is data under Apache-2.0, but **ask upstream** before relying on it.
- The CI licence check (#31) allows only MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib and Unicode licences, for crates and npm production dependencies.

### 3.4 Test strategy

1. **Golden fixtures:** `fixtures/manifest.json` with sha256, tier, features and licence (#4). Large docx and xlsx fixtures are generated with fixed seeds.
2. **Parity against the TS oracle (#5):**
    - Run the pinned `ooxml-core` and dump a normalised "view IR": `CellItem[]` for xlsx viewports, `LayoutResult` for docx, `PptxElement` trees for pptx.
    - Diff it against the Rust output, with float rounding and an allow-list that needs a reason per entry.
    - For docx, both sides use upstream's deterministic `createFakeMeasurer()`, so fonts never cause a diff.
3. **Ported unit tests:** the upstream Vitest suites for each ported module (xml, opc, numfmt, layout…) become Rust tests, which gives cheap, high-signal coverage.
4. **Visual diffs (#45):** the TS engine renders references (test-only) at DPR 1 and 2 plus print-to-PDF; the Rust renderer must stay within `pixelmatch` thresholds (xlsx ≤0.1% per viewport; docx ≤0.5% per page with the same page count; pptx ≤0.5% per slide). Same Chromium build and fonts.
5. **Renderer and editing-surface gates:** accessibility (axe plus an AX-tree screen-reader script, #42), selection and caret parity (#43), bidi (#44, #57), CJK IME (#56), spellcheck (#58), crash recovery (#30, #62), and the ship check that no shipped artefact imports TS (#38).
6. **Fuzzing:** cargo-fuzz on zip, XML, OPC and each format entry point (#30). Port the upstream `*.fuzz.test.ts` cases.
7. **Performance gate:** see §4.1. Every stage gate includes it.
8. **Editor gates (M2):** lossless round trip with validators (#48, #49), edit round trip (#50), undo/redo parity (#51), edit-path perf (#52); see PRODUCT_SPEC §5.

---

## 4. Staged plan

**Gates (PRODUCT_SPEC §5).** Every stage from Stage 3 must pass: view-IR parity, visual at DPR 1 and 2 plus print-to-PDF, **unsupported content at 0 for the format at its stage gate**, accessibility, bidi, selection and caret, and the perf gate. **Perf timing rule:** every commit posts numbers; until a format has a full open-to-first-paint path in Rust the gate is regression-versus-main only; from that commit on, **WASM must beat TS at every tier** (S, M, L, XL), at 1x and 4x CPU, on first paint (including wasm fetch, compile and instantiate), cold parse, scroll/view-query latency and peak memory. "Beats" = lower median **and** the 95% bootstrap CI of the WASM/TS ratio entirely below 1.0; a tie fails; never relaxed. Foundation PRs (zip, XML, OPC) are tracked, not gated against TS. Bundle and loader ceilings are fixed at R0 and only go down.

Sizes: **S** is part of one agent session; **M** is about one full session; **L** is several sessions and should be split at claim time.

**Milestone 1, viewer**

| Stage | Name | Issues | Acceptance |
|---|---|---|---|
| 0 | Scaffolding: workspace, wasm build, npm skeleton, CI scripts, fixtures, parity harness, licence and ship checks | #1 to #5, #31, #32, #38 | Builds natively and to wasm32; CI green; corpus verified; self-diff passes; MIT licence with Apache-2.0 text, NOTICE and PROVENANCE |
| 1 | Perf-eval track, TS baseline, scoreboard harness | #6 to #10, #39 | Harness, history, gate machinery and scoreboard page live; BASELINE.md frozen |
| 2 | Shared OPC (streaming zip and XML), boundary, visual harness, parse race | #11 to #14, #40, #45 | Part-graph parity; zip-bomb and DTD tests; transport decided with numbers |
| **R0** | **Renderer port record (Jared)** | #33 (Jared), #34 to #37, #46 | Painter inventory per format (canvas vs DOM, what it draws, Rust module); module mapping; worker vs main-thread split; loader API and size budget; signed off by Jared. **No renderer issue is claimable before #33 merges** |
| 3 | xlsx viewer in Rust | #15 to #19, #41 to #44 | All viewer gates green for xlsx; unsupported content 0; xlsx beats-TS gate armed (#19) |
| 4 | docx viewer in Rust | #20 to #23 | Same page counts; all viewer gates; docx gate armed (#23) |
| 5 | pptx viewer in Rust, all element kinds, no per-slide fallback | #24 to #28 | All viewer gates; pptx gate armed (#27) |
| 6 | Viewer shell in Rust (+ Jared's viewer edits) | #29 | Shell passes accessibility, selection and keyboard scripts on all target browsers |
| 7 | Hardening, crash recovery, M1 report (**M1 gate**) | #30 | Fuzzing clean; crash recovery tested; memory ceiling enforced; M1 report |

**Post-port redesign (Jared), backlog, not claimable:** DOM-vs-canvas comparison (#92), in-wasm text shaping (#93), metric-compatible open fonts (#94); labelled `blocked-redesign`.

**Milestone 2, editor** (labelled `blocked-m2` until the M1 gate passes and Jared confirms the budget)

| Stage | Name | Issues | Acceptance |
|---|---|---|---|
| E0 | Editor harnesses, validators, TS editor baseline, Rust editor chrome | #47 to #54 | Harnesses run TS-vs-TS; validators in CI; TS losses recorded; chrome passes accessibility |
| E-text | Rust text-editing surface: single-line (before E1), IME, bidi, spellcheck, a11y, mobile, rich text (before E3), crash recovery | #55 to #62 | IME, bidi, selection and caret, clipboard, spellcheck and accessibility gates pass on desktop and mobile smoke tests |
| E1 | xlsx value and format edits, sheet ops, undo/redo, lossless save, open-edit-save race | #63 to #70 | Gates (a) to (e); reopen passes viewer gates |
| E2 | Rust xlsx formula engine | #71 to #77 | Function coverage equals TS; gates (a) to (e) |
| E3 | docx editing | #78 to #83 | Gates (a) to (e) |
| E4 | pptx editing | #84 to #88 | Gates (a) to (e) |
| E5 | Write-path hardening and final report | #89 to #91 | Save fuzzing clean; 1,000-op memory gate; final report |

**Parallelism:**
- Stage 1 needs only the fixture corpus (#4), so the TS baseline is ready before the first Rust parser lands. The parse race (#40) starts in the foundation stage.
- R0 inventory (#34) and loader budget (#37) start early (after #1 and #2/#14); mapping (#35) and the worker split (#36) follow the inventory; model work in Stages 3 to 5 (#15 to #17, #20, #21, #24 to #26) can proceed before R0 closes, renderer work cannot.
- The integration issues (#19, #23, #27) are the serial points and arm each format's hard perf gate.

### 4.1 Performance-eval track (Stage 1, Jared's requirement)

- **Harness:**
  - A local static server sets COOP and COEP headers, which `performance.measureUserAgentSpecificMemory` needs.
  - Minimal host pages mount the upstream TS renderers (test-only reference and baseline) and the Nitro XML components. A `?engine=ts|wasm` switch selects which one is measured.
  - A Playwright (Chromium) runner opens a CDP session per page. Runs are fully offline.
- **Corpus:** size tiers S (<100 KB), M (<1 MB), L (<10 MB) and XL (>=10 MB) per format, with at least 3 files per tier. Large docx and xlsx are generated (xlsx up to 1M cells; docx up to 500 pages). pptx uses upstream's 1.8 MB to 36.8 MB decks, once their redistribution licence is confirmed.
- **Metrics:**
  - parse time (`bytes-ready` to `parse-end`)
  - time to first paint, including wasm fetch, streaming compile and instantiate (double rAF)
  - scroll/view-query latency and time to interactive
  - full render time
  - peak JS heap: CDP `Runtime.getHeapUsage` sampled every 10 ms, plus `measureUserAgentSpecificMemory` and post-GC retained heap
  - WASM linear memory (`memory.buffer.byteLength`)
  - JS bundle and `.wasm` size (raw, gzip, brotli)
  - main-thread long tasks: count and total blocking time
- **Method:**
  - cold runs use a fresh context with no HTTP, V8 or WASM code cache; warm runs reuse one context after a discarded warm-up
  - enough iterations for a meaningful 95% bootstrap CI of the WASM/TS ratio (start at 10 cold and 20 warm, raise as needed)
  - report the median, p95 and the CI
  - CPU throttle at 1x and 4x (a mobile proxy)
- **History:**
  - `perf/history.jsonl` lives on a `perf-data` branch, with one line per (commit, engine, fixture, mode, env) and `{median, p95, min, max}` per metric
  - main-branch runs append; PR runs only compare
- **Gate:**
  - **Regression vs main** (always): timings +10% **and** +5 ms; memory +10% **and** +2 MB; brotli size +3% **and** +10 KB; TBT +15% **and** +20 ms; baseline = median of the last 5 main runs. `perf-regression-accepted` with a written reason can override this part only.
  - **WASM beats TS** (per format, armed by #19, #23, #27; never relaxed): every tier, 1x and 4x, first paint, cold parse, scroll/view-query and peak memory; lower median and 95% CI of the ratio below 1.0; a tie fails. Editing (keystroke-to-paint, IME commit-to-paint, undo, save) gets the same rule in M2.
  - **No performance-based fallback of any kind.** A loss is a red gate fixed by design (streaming compile, cached module, pre-warmed worker, small core plus lazy per-format modules).
  - **Bundle and startup:** per-format initial-payload ceiling (brotli, including any fonts and data shipped) and loader ceiling, fixed at R0, ratchet down only; time to interactive gated.
- **Dashboard and public scoreboard:** a static page (inline SVG, no chart library) on Pages, navy, black and white, published from every main commit: TS and WASM on the same axes, plus the parse, render-to-first-paint and open-edit-save races against pinned competitors (#39 to #41, #70): SheetJS, ExcelJS, calamine, umya-spreadsheet, docx-preview, mammoth.js, docx-rs, PPTXjs, the LibreOffice WASM build, other Rust/WASM viewers found during vetting, and always upstream ooxml TS. Entries that do less are shown as "not comparable"; losses are published as prominently as wins.

---

## 5. Issue breakdown

**94 issues** on GitHub (#1 to #94), generated from `issues.json` (`gen_issues.py`). Plan numbers equal GitHub issue numbers; each issue has a `Depends on #X` line. Milestone 2 issues carry `blocked-m2`; post-port redesign issues (stage PP) carry `blocked-redesign`.

- **Agent fit:** Codex for well-specified mechanical ports with ready test suites; Claude Code for design-heavy or cross-cutting work; Cursor for CI, browser integration and dashboards; OpenCode (free NVIDIA models) for small, well-bounded tasks; a human **Developer** owns the scoreboard races; **Jared** owns the R0 renderer decision (#33).
- **Every issue** follows AGENTS.md (claim, branch `issue-<N>-<slug>`, PR with `Closes #N`, handoff comment).

| # | Title | Stage | Size | Agent | Depends on | Parallel |
|---|---|---|---|---|---|---|
| 1 | Stage 0: Bootstrap Cargo workspace and pin upstream | 0 | S | Codex | - | no (first issue) |
| 2 | Stage 0: wasm-pack build, release profile and npm package skeleton | 0 | S | Codex | 1 | yes, with #3 and #4 after #1 |
| 3 | Stage 0: CI for Rust and WASM (edit scripts/ci/rust.sh and wasm.sh; ask for any YAML change) | 0 | S | Cursor cloud | 1 | yes, with #2 and #4 after #1 |
| 4 | Stage 0: Fixture corpus manifest and generated size tiers | 0 | M | Claude Code | 1 | yes, with #2 and #3 after #1 |
| 5 | Stage 0: Parity harness (TS oracle vs Rust, view IR diff) | 0 | M | Claude Code | 1, 4 | after #1 and #4; in parallel with Stage 1 |
| 6 | Stage 1 (perf): Local perf harness, server and Playwright/CDP runner | 1 | M | Claude Code | 4 | yes; needs only #4, so it can run alongside #2, #3 and #5 |
| 7 | Stage 1 (perf): Metric instrumentation (latency, memory, long tasks, sizes) | 1 | M | Cursor cloud | 6 | after #6 |
| 8 | Stage 1 (perf): JSON-lines history keyed by commit and CI regression gate (edit scripts/ci/perf.sh; ask for any YAML change) | 1 | M | Codex | 7 | after #7 |
| 9 | Stage 1 (perf): Public scoreboard dashboard on GitHub Pages, navy/black/white (edit scripts/ci/pages.sh; ask for any YAML change) | 1 | S | Cursor cloud | 8 | after #8; in parallel with #10 |
| 10 | Stage 1 (perf): Capture TypeScript baseline and publish baseline report | 1 | S | Cursor cloud | 7, 8 | after #7 and #8 |
| 11 | Stage 2: nitro-xml-opc::zip: bounded zip reader | 2 | S | Codex | 1 | yes, with #12 and #14 |
| 12 | Stage 2: nitro-xml-opc::xml: strict streaming XML layer with namespaces | 2 | M | Codex | 1 | yes, with #11 and #14 |
| 13 | Stage 2: nitro-xml-opc: content types, relationships, part resolution, safe hrefs | 2 | M | Codex | 11, 12 | after #11 and #12 |
| 14 | Stage 2: JS/WASM boundary conventions, bulk transfer, worker, event and per-frame command-buffer protocol | 2 | M | Claude Code | 2 | yes, after #2 |
| 15 | Stage 3 (xlsx): Workbook part, shared strings, styles and theme | 3 | M | Codex | 13 | yes, with #17, #20, #24 and #25 |
| 16 | Stage 3 (xlsx): Streaming worksheet reader (sheetData, merges, cols, rows) | 3 | M | Codex | 15 | after #15 |
| 17 | Stage 3 (xlsx): Number format engine port | 3 | M | Codex | 1 | yes, independent after #1 |
| 18 | Stage 3 (xlsx): Grid layout and one-to-one Rust port of the grid painter (cells, merges, frozen panes, conditional formats) | 3 | L | Claude Code | 16, 17, 14, 33 | after #16, #17, #14 and the R0 record #33 |
| 19 | Stage 3 (xlsx): End-to-end Rust xlsx viewer (open-to-first-paint), engine API for sheet tabs, find, copy and hyperlinks; arm the xlsx perf gate | 3 | M | Cursor cloud | 18, 5, 10, 37 | after #18, #5, #10 and #37 |
| 20 | Stage 4 (docx): Styles, numbering, theme and settings | 4 | M | Codex | 13 | yes, with Stage 3 and #24/#25 after #13 |
| 21 | Stage 4 (docx): Body parser (paragraphs, runs, tables, pictures, sections, headers and footers, notes) | 4 | L | Claude Code | 20 | after #20 |
| 22 | Stage 4 (docx): Pagination engine port (fonts and text measurement follow upstream, batched) | 4 | L | Claude Code | 21, 14, 33 | after #21, #14 and the R0 record #33 |
| 23 | Stage 4 (docx): Rust port of the Print Layout painter, end to end; arm the docx perf gate | 4 | M | Cursor cloud | 22, 5, 10, 37 | after #22, #5, #10 and #37 |
| 24 | Stage 5 (pptx): Presentation, slide, layout, master and theme parsing with placeholder inheritance | 5 | L | Claude Code | 13 | yes, with #25 after #13 |
| 25 | Stage 5 (pptx): DrawingML fills, lines, colour transforms and preset geometry | 5 | L | Codex | 13 | yes, with #24 after #13 |
| 26 | Stage 5 (pptx): Text bodies, paragraphs, runs and tables model | 5 | M | Codex | 24, 25 | after #24 and #25 |
| 27 | Stage 5 (pptx): Rust port of the slide painters covering all element kinds (no per-slide fallback); arm the pptx perf gate | 5 | L | Cursor cloud | 26, 28, 5, 10, 37 | after #26, #28, #5, #10 and #37 |
| 28 | Stage 5: Chart model and Rust port of the chart painters (shared by xlsx, docx and pptx) | 5 | M | Codex | 25, 33 | after #25 and the R0 record #33 |
| 29 | Stage 6: Viewer shell in Rust (zoom, navigation, find UI, copy, hyperlinks, diagnostics) plus Jared's viewer edits | 6 | M | Cursor cloud | 19 | after #19; can start while Stages 4 and 5 run |
| 30 | Stage 7: Hardening, fuzzing, crash recovery, security limits and M1 report (the M1 gate) | 7 | M | Claude Code | 23, 27, 29 | after #23, #27 and #29 |
| 31 | Stage 0: CI licence check (cargo-deny and npm): allow only MIT, Apache-2.0, BSD, ISC, Zlib and Unicode | 0 | S | Codex | 1 | yes, with #2, #3 and #4 after #1 |
| 32 | Stage 0: Source real Excel-authored (and Word/PowerPoint) fixtures with permissive or public-domain licences | 0 | M | Cursor cloud | 4 | yes, with #2, #3 and #5 after #4 |
| 33 | R0: Renderer port record (painter inventory, Rust module mapping, worker split, loader budget), signed off by Jared | R0 | S | Jared | 34, 35, 36, 37 | after #34, #35, #36 and #37 |
| 34 | R0 (a): Upstream painter inventory per format (canvas vs DOM, what it draws, Rust module) | R0 | M | Claude Code | 1 | yes, after #1; in parallel with #36 and #37 |
| 35 | R0 (b): Map upstream painters to Rust modules | R0 | S | Claude Code | 34 | after #34 |
| 36 | R0 (c): Worker vs main-thread split per painter | R0 | M | Codex | 34, 14 | after #34 and #14 |
| 37 | R0 (d): Loader API and loader size budget (minimal JS loader skeleton, per-frame command-buffer applier, size gate) | R0 | M | Cursor cloud | 2, 14 | after #2 and #14; in parallel with #34 to #36 |
| 38 | Stage 0: CI check that fails if any shipped artefact imports the TS engine | 0 | S | OpenCode | 1 | yes, after #1 |
| 39 | Scoreboard (a): race harness and competitor pinning (licence, upkeep, fastest setup, public method) | 1 | M | Developer | 6, 4 | after #6; in parallel with #7 to #10 |
| 40 | Scoreboard (b): parse race (starts in the foundation stage) | 2 | M | Developer | 39, 13 | after #39; begins as soon as #11 to #13 parse packages |
| 41 | Scoreboard (c): render-to-first-paint race | 3 | M | Developer | 39, 19 | after #39 and #19; extended at #23 and #27 |
| 42 | Stage 3: Accessibility gate (axe-core plus screen-reader AX-tree script) | 3 | M | Cursor cloud | 6 | after #6; harness can be built against the TS reference pages |
| 43 | Stage 3: Selection and caret parity gate | 3 | S | Codex | 5, 6 | after #5 and #6 |
| 44 | Stage 3: Bidi/RTL gate (Arabic and Hebrew fixtures, visual and logical-order copy) | 3 | M | Claude Code | 5, 32 | after #5 and #32 |
| 45 | Stage 2: Visual gate harness (pixelmatch at DPR 1 and 2, plus print-to-PDF) | 2 | M | Cursor cloud | 5, 6 | after #5 and #6 |
| 46 | R0: Bundle and startup gate (per-format KB ceilings, time-to-interactive) | R0 | S | Codex | 7, 33 | after #7 and the R0 record #33 |
| 47 | E0: Editor architecture ADR (commands, transactions, history, writer, journal) | E0 | M | Claude Code | 30, 33 | first M2 issue |
| 48 | E0: Validators in CI (Open XML SDK tool and pinned LibreOffice) via setup-validators.sh | E0 | M | Cursor cloud | 3, 30 | in parallel with #47 |
| 49 | E0: Lossless round-trip harness (gate a), TS-vs-TS, plus the inventory of what TS loses on save | E0 | M | Claude Code | 48, 5, 4 | after #48 |
| 50 | E0: Edit-script harness (gate b): edits/*.json and the TS runner | E0 | M | Claude Code | 49 | after #49 |
| 51 | E0: Undo/redo parity harness (gate c) | E0 | M | Codex | 50 | after #50 |
| 52 | E0: TS editor perf baseline and edit-path metrics (gate d) | E0 | M | Cursor cloud | 8, 10, 30 | in parallel with #47 to #51 |
| 53 | E0: Rust editor chrome framework part 1: command registry, ribbon and keyboard shortcuts | E0 | M | Claude Code | 47, 29 | after #47 and #29 |
| 54 | E0: Rust editor chrome framework part 2: dialogs, menus, context menus and focus management | E0 | M | Cursor cloud | 53 | after #53 |
| 55 | E-text: Single-line text-editing surface (caret, selection, clipboard, keyboard) for the cell editor and formula bar | E-text | M | Claude Code | 53, 43 | after #53; must land before E1 |
| 56 | E-text: IME composition (CJK) gate | E-text | M | Claude Code | 55 | after #55 |
| 57 | E-text: Bidirectional text editing gate (caret movement and selection in RTL and mixed text) | E-text | M | Claude Code | 55, 44 | after #55 and #44 |
| 58 | E-text: Spellcheck integration gate | E-text | M | Cursor cloud | 55 | after #55 |
| 59 | E-text: Accessibility of the editing surface (caret and selection exposed, live regions) | E-text | M | Cursor cloud | 55, 42 | after #55 and #42 |
| 60 | E-text: Mobile virtual keyboards and selection handles (iOS and Android) | E-text | M | Cursor cloud | 55, 56 | after #55 and #56 |
| 61 | E-text: Rich multi-paragraph text surface (runs, paragraphs, tables) for Word and slide text | E-text | L | Claude Code | 55, 56, 57, 59 | after #55 to #59; must land before E3 |
| 62 | E-text: Crash recovery via op journal replayed into a fresh worker (gate e) | E-text | M | Codex | 47, 30 | after #47 |
| 63 | E1: Command, transaction and change-set core (shared) | E1 | M | Claude Code | 47 | after #47 |
| 64 | E1: Undo/redo history matching the TS editor (undo/redo gate) | E1 | M | Codex | 63, 51 | after #63 and #51 |
| 65 | E1: Lossless OPC package writer (lossless save gate) | E1 | M | Codex | 13, 63, 49 | after #13, #63 and #49 |
| 66 | E1 (xlsx): Cell value, number-format and style edits | E1 | M | Codex | 63, 16 | after #63 and #16 |
| 67 | E1 (xlsx): Row and column size and hide; sheet add, rename, delete and reorder | E1 | M | Codex | 66 | after #66 |
| 68 | E1 (xlsx): Workbook writer with lossless save, fullCalcOnLoad and stale dependents | E1 | M | Codex | 65, 66 | after #65 and #66 |
| 69 | E1 (xlsx): Rust xlsx editor UI (cell editor, formula bar, ribbon wiring, stale marking); arm edit perf gate | E1 | M | Cursor cloud | 55, 64, 67, 68, 54 | after #55, #64, #67 and #68 |
| 70 | Scoreboard (d): open-edit-save race | E1 | M | Developer | 39, 68 | after #39 and #68 |
| 71 | E2 (xlsx): Formula parser, AST and reference model | E2 | M | Codex | 66 | after #66 |
| 72 | E2 (xlsx): Dependency graph and recalculation scheduler | E2 | M | Claude Code | 71 | after #71 |
| 73 | E2 (xlsx): Function library part 1: math, trigonometry and statistics | E2 | M | Codex | 71 | after #71; in parallel with #74 and #75 |
| 74 | E2 (xlsx): Function library part 2: logical, text, information, date and time | E2 | M | OpenCode | 71 | after #71; in parallel with #73 and #75 |
| 75 | E2 (xlsx): Function library part 3: lookup and reference, financial, engineering, dynamic arrays | E2 | M | Codex | 71 | after #71; in parallel with #73 and #74 |
| 76 | E2 (xlsx): Insert and delete rows and columns with reference shifting | E2 | M | Codex | 72 | after #72 |
| 77 | E2 (xlsx): Function-coverage sweep and recalculation in the editor | E2 | M | Cursor cloud | 72, 73, 74, 75, 76, 69 | after #72 to #76 and #69 |
| 78 | E3 (docx): Text, character and paragraph formatting, styles and lists on the rich surface | E3 | M | Claude Code | 61, 63, 22 | after #61 and #63 |
| 79 | E3 (docx): Tables, pictures, sections, headers and footers | E3 | M | Codex | 78 | after #78 |
| 80 | E3 (docx): Lossless document writer | E3 | M | Codex | 65, 78 | after #65 and #78 |
| 81 | E3 (docx): Incremental re-pagination after edits | E3 | M | Claude Code | 22, 78 | after #22 and #78 |
| 82 | E3 (docx): Tracked changes (accept/reject) and comments (add/reply/resolve) | E3 | M | Claude Code | 78 | after #78 |
| 83 | E3 (docx): Rust docx editor UI (ribbon wiring, Print Layout editing, find and replace); arm edit perf gate | E3 | M | Cursor cloud | 79, 80, 81, 82 | after #79 to #82 |
| 84 | E4 (pptx): Shape edits (move, resize, rotate, add, delete, duplicate, z-order, group, fill and line) | E4 | M | Codex | 63, 27 | after #63 and #27 |
| 85 | E4 (pptx): Text and table edits on the rich surface | E4 | M | Codex | 84, 61 | after #84 and #61 |
| 86 | E4 (pptx): Slide add, delete, duplicate and reorder; speaker notes and thumbnails | E4 | M | Codex | 84 | after #84 |
| 87 | E4 (pptx): Lossless presentation writer | E4 | M | Claude Code | 65, 84 | after #65 and #84 |
| 88 | E4 (pptx): Rust pptx editor UI; arm edit perf gate | E4 | M | Cursor cloud | 85, 86, 87 | after #85, #86 and #87 |
| 89 | E5: Save-path fuzzing (random command sequences, undo invariants, save and reopen) | E5 | M | Claude Code | 77, 83, 88 | after #77, #83 and #88 |
| 90 | E5: Long-session memory: 1,000-op sessions within the memory gate | E5 | M | Cursor cloud | 77, 83, 88 | after #77, #83 and #88; in parallel with #89 |
| 91 | E5: Final report (viewer and editor, TS and competitors) | E5 | S | Claude Code | 89, 90 | after #89 and #90 |
| 92 | Post-port redesign: DOM-vs-canvas renderer comparison | PP | L | Jared | 30 | not claimable until Jared opens the redesign milestone |
| 93 | Post-port redesign: in-wasm text shaping and measurement | PP | M | Jared | 30 | not claimable until Jared opens the redesign milestone |
| 94 | Post-port redesign: metric-compatible open fonts (Carlito, Caladea) and font fallback | PP | M | Jared | 30 | not claimable until Jared opens the redesign milestone |

---

## 6. Risks and open questions for Jared

**Top risks**

1. **Scope mismatch.** The upstream is a 1.56M-line editor suite, not a small renderer. Unless the port is limited to the read and view path for three formats, it will not fit a limited credit budget. pptx alone is about 146k production lines in core.
2. **Moving upstream.** The repo was created on 2026-10-01, is pushed daily and is mid-refactor (the pptx XML model migration, DrawingML extraction). Pin a commit and re-sync on purpose, each time with parity re-runs.
3. **Renderer fidelity.** The TS renderers are tuned against Office captures, and the Rust renderers must re-earn the pixel thresholds; pptx effects are hardest. Mitigation: parity harness, visual diffs at DPR 1 and 2 plus print, per-format gates. The biggest new risk is the Rust text-editing surface (IME, bidi, spellcheck, accessibility, mobile keyboards), now its own stage (E-text).
4. **Text measurement across the boundary.** Fonts and measurement follow upstream for now, so measurement requests must be batched per layout pass (#14) or they erase WASM gains. In-wasm shaping and open fonts (Carlito, Caladea) are Jared's post-port redesign (#93, #94).
5. **Small-file startup is a hard gate.** Instantiation and boundary costs can lose to V8 on 3 to 60 KB files; with no fallback, the answer is design (streaming compile, cached module, pre-warmed worker, small core module), never routing to TS.
6. **Memory behaviour.** Linear memory never shrinks. Use one worker per document and enforce the limits.
7. **Bundle size.** wasm plus any fonts and data shipped must stay under per-format ceilings smaller than TS's payload (#46); track it from day one.
8. **Fixture licensing.** Several large pptx samples look like third-party downloads, and their licence is unknown.

**Open questions for Jared** (from PRODUCT_SPEC v3 §7; reply "defaults" to accept all)

1. **Renderer technology: decided.** Port upstream's approach as-is (batched per frame); you redesign after the conversion (post-port milestone, #92 to #94).
2. **Threads:** default none (no SharedArrayBuffer or COOP/COEP); a threaded build only on request.
3. **Budget:** the scope is far beyond the original estimate; default M1 first, M2 `blocked-m2` until the M1 gate and your confirmation.
4. **iOS Safari:** default full target with functional, accessibility and IME smoke tests; perf gated in Chromium.
5. **CI cadence:** default PRs run S/M tiers at 1x plus parity, round trip, accessibility and IME; main and nightly run the full corpus at 1x and 4x, validators and the scoreboard.
6. **Viewer edits** (#29): which ones?
7. **ASK items:** legacy and encrypted formats, docx review features, speaker notes and thumbnails, print, collaboration, export.

### 6.1 Decisions (current calls, open to change)

| # | Decision | Current call |
|---|---|---|
| D1 | Repo shape | New repo `jgridifier/nitro-xml`, not a fork; upstream pinned at `e66aaec24adc0b68472eae1fc42aa4598e09c352` as oracle and algorithm source |
| D2 | Licence | MIT; files ported or derived from ooxml remain Apache-2.0 (NOTICE, LICENSE-THIRD-PARTY, PROVENANCE.md, SPDX headers) |
| D3 | TS engine | Test and bench harness only, permanently; never shipped; no runtime fallback |
| D4 | Scope | Full editor suite, viewer first (M1), editor second (M2) |
| D5 | Format order | xlsx, then docx, then pptx, in both milestones |
| D6 | Perf | Measured in Chromium; WASM must beat TS everywhere (§4.1) |
| D7 | Perf CI cadence | Small corpus on every PR; full corpus nightly and on demand; every commit posts numbers |
| D8 | Legacy and encrypted files | Unsupported-format message in v1; Rust track after E5 (ASK) |
| D9 | Renderer | Straight one-to-one ports of upstream painters, batched per frame; R0 record (#33) signed off by Jared; redesign post-port (#92 to #94) |
| D10 | Viewer edits | #29 placeholder |
| D11 | Crate layout | `nitro-xml-opc` (zip, XML, OPC, DrawingML, charts), `nitro-xml-xlsx`, `nitro-xml-docx`, `nitro-xml-pptx`, `nitro-xml-wasm`, umbrella `nitro-xml`; npm `nitro-xml` |
| D12 | Threads | None by default |

## 7. Files

- `docs/PLAN.md` (this file)
- `docs/index.html` (mobile-readable version, published on GitHub Pages)
- `docs/PRODUCT_SPEC.md` (v3, the source of the gates)
- GitHub issues #1 to #94 in `jgridifier/nitro-xml` (generated from `issues.json`)
