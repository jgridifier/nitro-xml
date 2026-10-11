# Provenance

Nitro XML is MIT-licensed. **Files ported or derived from ooxml remain under Apache-2.0** (see NOTICE and LICENSE-THIRD-PARTY). This file states, for every such file, where it came from and what changed, following upstream's own `PROVENANCE.md` convention. It is required by Apache-2.0 §4(b) ("You must cause any modified files to carry prominent notices stating that You changed the files").

## Upstream

| Item | Value |
|---|---|
| Repository | https://github.com/ChristopherVR/ooxml |
| Licence | Apache-2.0 |
| Pinned commit | `e66aaec24adc0b68472eae1fc42aa4598e09c352` (2026-10-11) |
| Role | Algorithm source for the Rust port; the TS engine runs **only in the test and bench harness** as the correctness reference and speed baseline. No TypeScript from upstream ships in Nitro XML. |

## Rules

- A Rust file that ports or derives logic from an upstream file starts with:
  ```rust
  // SPDX-License-Identifier: Apache-2.0
  // Ported from ChristopherVR/ooxml@e66aaec <upstream path(s)>; changes listed in PROVENANCE.md.
  ```
- New original files start with `// SPDX-License-Identifier: MIT`.
- Add one row below per ported file in the same PR. The licence check fails if an `Apache-2.0` file has no row.
- Test fixtures copied from upstream are listed in `fixtures/manifest.json` with their own licence; fixtures of unknown licence are never committed.

## Related third-party packages (not yet ported)

These are upstream JS dependencies of M1 painters. Decisions live in `docs/adr/0002-renderer-port.md` (licence decisions) and `LICENSE-THIRD-PARTY`.

| Package | Licence | Role in upstream | Nitro XML decision |
|---|---|---|---|
| [`emf-converter`](https://github.com/ChristopherVR/emf-converter) | Apache-2.0 | EMF/WMF → data URL for P5/P15 (`metafile-convert.ts`) | Port into `nitro-xml-render::metafile` (inventory **S3**). Ported files will be Apache-2.0 with PROVENANCE rows in the PR that adds them. |
| [`mtx-decompressor`](https://github.com/ChristopherVR/mtx-decompressor) | MPL-2.0 | EOT / MicroType Express decompress (`eot-parser.ts`) | **Clean-room MIT rewrite** when embedded fonts land. Do not vendor or port MPL sources — not on the `deny.toml` / `scripts/ci/licenses.sh` allow-list. |

## Ported files

| Nitro XML file | Upstream source (path @ commit) | Changes |
|---|---|---|
| `rs/crates/nitro-xml-opc/src/zip/mod.rs` | `src/core/xlsx/read/package.ts @ e66aaec` | Rust rewrite of readZipParts limits and SourceIndex case-insensitive lookup over the `zip` crate instead of JSZip; lazy per-part inflation; typed errors. |
| `rs/crates/nitro-xml-opc/src/zip/error.rs` | `src/core/xlsx/read/package.ts @ e66aaec` | Typed `ZipError` kinds in place of upstream `Error` messages; same limit categories as readZipParts. |
| `rs/crates/nitro-xml-opc/src/xml/mod.rs` | `src/core/xml/xml.ts`, `lite.ts`, `namespaces.ts @ e66aaec` | Module root for the streaming XML layer; re-exports parse, tree and namespace constants. |
| `rs/crates/nitro-xml-opc/src/xml/parse.rs` | `src/core/xml/xml.ts`, `lite.ts @ e66aaec` | `parseXml` behaviour over `quick-xml` `NsReader`: DTD/ENTITY rejection, single root, undeclared-prefix and duplicate-attr errors; owned tree instead of DOM. |
| `rs/crates/nitro-xml-opc/src/xml/decode.rs` | `src/core/xml/lite.ts @ e66aaec` | Predefined/numeric entity decode, XML EOL folding and attribute-value whitespace normalisation from the lite reader. |
| `rs/crates/nitro-xml-opc/src/xml/namespaces.rs` | `src/core/xml/namespaces.ts @ e66aaec` | `NS` URI constants generated as Rust `ns::*` string constants. |
| `rs/crates/nitro-xml-opc/src/fonts/google_webfonts.rs` | `src/ui/src/pptx/render/google-webfonts.ts @ e66aaec` | Rust port of `GOOGLE_FONTS_CSS2_BASE`, axis/display constants, `buildGoogleFontsFragment` and `buildGoogleFontsHref` (encodeURIComponent-compatible). Catalogue lookup, local-install check and DOM `<link>` injection remain for the pptx painter (#27). Configurable base / fetch-off is original (#121) in `fonts/source.rs`. |
| `rs/crates/nitro-xml-opc/src/opc/content_types.rs` | `src/core/opc/content-types.ts @ e66aaec` | Rust rewrite over the owned XML tree; `BTreeMap` for stable dump order of defaults/overrides. |
| `rs/crates/nitro-xml-opc/src/opc/relationships.rs` | `src/core/opc/relationships.ts @ e66aaec` | `IndexMap` preserves relationship insertion order for `target_of_type`; same resolve/rels-path rules. |
| `rs/crates/nitro-xml-opc/src/opc/rel_types.rs` | `src/core/opc/relationship-types.ts @ e66aaec` | URI constants as Rust `pub const` strings. |
| `rs/crates/nitro-xml-opc/src/opc/safe_href.rs` | `src/core/opc/safe-href.ts @ e66aaec` | Same store/open policy and obfuscation hardening; `Option<&str>` instead of null/undefined. |
| `rs/crates/nitro-xml-opc/src/opc/signature.rs` | `src/core/opc/signature/detection.ts @ e66aaec` | Listing-only port of `isSignaturePart` / `detectDigitalSignatures` (no XML-DSig verify). |
| `rs/crates/nitro-xml-opc/src/opc/package.rs` | `src/core/xlsx/read/package.ts` (SourceIndex) and `src/core/opc/package.ts` @ e66aaec | `Package::open` over `ZipPackage` with format allow-list (PRODUCT_SPEC.md); properties/signatures listed only. |
| `rs/crates/nitro-xml-xlsx/src/numfmt/mod.rs` | `src/core/xlsx/numfmt/index.ts @ e66aaec` | Rust module exports including input parsing (#130). |
| `rs/crates/nitro-xml-xlsx/src/numfmt/input.rs` | `src/core/xlsx/numfmt/input.ts @ e66aaec` | `parse_cell_input` / number & fraction entry; `ERROR_CODES` kept local (no full workbook `CellValue` object). |
| `rs/crates/nitro-xml-xlsx/src/numfmt/input_date.rs` | `src/core/xlsx/numfmt/input-date.ts @ e66aaec` | Date/time typed entry over `date_to_serial` / `UtcDateTime`; local year via `localtime_r` on unix. |
| `rs/crates/nitro-xml-xlsx/src/display_text.rs` | `src/core/xlsx/display-text.ts @ e66aaec` | Formatted display text over `WorkbookSlice` + a minimal `Cell` view (columnar store is #16). |
| `rs/crates/nitro-xml-xlsx/src/numfmt/types.rs` | `src/core/xlsx/numfmt/types.ts @ e66aaec`; `CellValue` / `isCellError` from `src/core/xlsx/model.ts @ e66aaec` | Enums for tokens and a local `CellValue` (no workbook model). |
| `rs/crates/nitro-xml-xlsx/src/numfmt/decimal.rs` | `src/core/xlsx/numfmt/decimal.ts @ e66aaec` | 15-digit reduction matches JavaScript `toExponential(14)` (half away from zero). Integer `String()` matches JS for elapsed counts and fraction parts. |
| `rs/crates/nitro-xml-xlsx/src/numfmt/tokenizer.rs` | `src/core/xlsx/numfmt/tokenizer.ts @ e66aaec` | Same section split, colours, conditions, locale brackets and date-token normalisation. |
| `rs/crates/nitro-xml-xlsx/src/numfmt/number.rs` | `src/core/xlsx/numfmt/number.ts @ e66aaec` | Same placeholders, grouping, scaling commas, percent and engineering exponents. |
| `rs/crates/nitro-xml-xlsx/src/numfmt/fraction.rs` | `src/core/xlsx/numfmt/fraction.ts @ e66aaec` | Same continued-fraction (no semiconvergents) and mixed-fraction blanking. |
| `rs/crates/nitro-xml-xlsx/src/numfmt/date.rs` | `src/core/xlsx/numfmt/date.ts @ e66aaec` | 1900 leap-day serial 60, 1904 system, elapsed counts. UTC fields replace `Date`. |
| `rs/crates/nitro-xml-xlsx/src/numfmt/general.rs` | `src/core/xlsx/numfmt/general.ts @ e66aaec` | General format, 11-character width. |
| `rs/crates/nitro-xml-xlsx/src/numfmt/compile.rs` | `src/core/xlsx/numfmt/compile.ts @ e66aaec` | Same section choice. Cache is thread-local rather than one process-wide `Map`. |
| `rs/crates/nitro-xml-xlsx/src/numfmt/format.rs` | `src/core/xlsx/numfmt/format.ts @ e66aaec` | `format_value` / `is_date_format`, including width fitting and `#` overflow. |
| `rs/crates/nitro-xml-xlsx/src/numfmt/builtins.rs` | `src/core/xlsx/numfmt/builtins.ts @ e66aaec` | en-US built-in ids, aliases and ribbon presets. |
| `rs/crates/nitro-xml-xlsx/src/numfmt/decimals.rs` | `src/core/xlsx/numfmt/decimals.ts @ e66aaec` | Increase / Decrease Decimal on a format code. |
| `rs/crates/nitro-xml-xlsx/src/model.rs` | `src/core/xlsx/model.ts` (style/theme/name types) @ e66aaec | Stage-3 subset plus Stage-16 `ActiveWorksheet` dump shape; `stringify_json` for JS number text parity. |
| `rs/crates/nitro-xml-xlsx/src/xml_util.rs` | `src/core/xlsx/read/xml-util.ts`, `src/core/xml/xml.ts` (`relAttr`) @ e66aaec | SpreadsheetML helpers over the owned OPC element tree. |
| `rs/crates/nitro-xml-xlsx/src/address.rs` | `src/core/xlsx/address.ts` @ e66aaec | `parseRange` / `parseAddress`, `column_label` for formula translation. |
| `rs/crates/nitro-xml-xlsx/src/builtin_formats.rs` | `src/core/xlsx/read/builtin-formats.ts @ e66aaec` | Thin wrapper over `numfmt::BUILTIN_NUMBER_FORMATS` plus locale id fallback for style resolution. |
| `rs/crates/nitro-xml-xlsx/src/password_hash.rs` | `src/core/xlsx/read/password-hash.ts @ e66aaec` | Agile hash attribute reader for workbook protection. |
| `rs/crates/nitro-xml-xlsx/src/style_parts.rs` | `src/core/xlsx/read/style-parts.ts @ e66aaec` | Fonts, fills, borders, colours, indexed palette. |
| `rs/crates/nitro-xml-xlsx/src/styles.rs` | `src/core/xlsx/read/styles.ts`, `src/core/xlsx/styles.ts` (`styleKey`, `styleAt`) @ e66aaec | Styles part, xf map, named styles, dxfs; `styleKey` via sorted JSON; `style_at` for display-text (#130). |
| `rs/crates/nitro-xml-xlsx/src/shared_strings.rs` | `src/core/xlsx/read/shared-strings.ts @ e66aaec` | Plain and rich shared strings; phonetics ignored. |
| `rs/crates/nitro-xml-xlsx/src/theme.rs` | `src/core/xlsx/read/theme.ts` (+ minimal DrawingML colour/font scheme) @ e66aaec | SpreadsheetML palette slot order; latin major/minor fonts. |
| `rs/crates/nitro-xml-xlsx/src/workbook.rs` | `src/core/xlsx/read/load.ts`, `workbook-part.ts`, `workbook.ts` (defaults) @ e66aaec | Stage-3 load orchestration, Print_Area lift, sheet stubs, warnings; Stage-16 lazy `parse_worksheet` for the active sheet only. |
| `rs/crates/nitro-xml-xlsx/src/cells.rs` | `src/core/xlsx/cells.ts @ e66aaec` | Columnar parallel arrays + side maps instead of per-cell heap objects; numeric pool stores `js_number_string` text for gate dumps. |
| `rs/crates/nitro-xml-xlsx/src/worksheet.rs` | `src/core/xlsx/read/worksheet.ts @ e66aaec` | One worksheet part: split sheetData, shell parse (merges, cols, rows, sheetView), attach to workbook model. |
| `rs/crates/nitro-xml-xlsx/src/sheet_data.rs` | `src/core/xlsx/read/sheet-data.ts @ e66aaec` | Cell read path, formulas (shared/array/dynamic), warning strings; dump via `store_to_dump_cells`. |
| `rs/crates/nitro-xml-xlsx/src/sheet_data_source.rs` | `src/core/xlsx/read/sheet-data-source.ts @ e66aaec` | `split_sheet_data` string split; inner XML wrapped for tree parse (no full-sheet DOM). |
| `rs/crates/nitro-xml-xlsx/src/sheet_props.rs` | `src/core/xlsx/read/sheet-props.ts @ e66aaec` | Sheet format, default row/col sizes, column defs, row info, sheet view / freeze / selection. |
| `rs/crates/nitro-xml-xlsx/src/dates.rs` | `src/core/xlsx/read/dates.ts @ e66aaec` | `iso_to_serial` via `numfmt::date_to_serial` and 1900 leap-day quirks. |
| `rs/crates/nitro-xml-xlsx/src/metadata.rs` | `src/core/xlsx/read/metadata.ts @ e66aaec` | Dynamic-array metadata part for spill anchors. |
| `rs/crates/nitro-xml-xlsx/src/formula_text.rs` | `src/core/xlsx/read/formula-text.ts @ e66aaec` | `strip_future_prefixes` moved from `workbook.rs`. |
| `rs/crates/nitro-xml-xlsx/src/formula_translate.rs` | `src/core/xlsx/formula/transform.ts`, `tokenizer.ts`, `lexemes.ts`, `ref-text.ts @ e66aaec` | `translate_formula` for shared-formula slaves (relative A1 shift). |
| `rs/crates/nitro-xml-xlsx/src/formula_internal.rs` | `src/core/xlsx/formula/tokenizer.ts`, `lexemes.ts @ e66aaec` | Minimal tokenizer/rewriter used only by `formula_translate.rs` (not a full formula engine). |
| `rs/crates/nitro-xml-opc/src/drawingml/mod.rs` | `src/core/drawingml/index.ts` @ e66aaec | Module root re-exporting the DrawingML model API used by format crates. |
| `rs/crates/nitro-xml-opc/src/drawingml/types.rs` | `src/core/drawingml/types.ts` @ e66aaec | DrawingML model types (serde for gate dumps). |
| `rs/crates/nitro-xml-opc/src/drawingml/dom.rs` | `src/core/drawingml/dom.ts`, `src/core/xml/xml.ts` @ e66aaec | DrawingML DOM helpers over `nitro-xml-opc` `Element`. |
| `rs/crates/nitro-xml-opc/src/drawingml/color_primitives.rs` | `src/core/color/color-primitives.ts` @ e66aaec | Shared colour math for DrawingML resolution. |
| `rs/crates/nitro-xml-opc/src/drawingml/color_linear.rs` | `src/core/color/color-linear.ts` @ e66aaec | sRGB ↔ linear-light channel conversion for tint/shade. |
| `rs/crates/nitro-xml-opc/src/drawingml/color.rs` | `src/core/drawingml/drawing-color.ts` @ e66aaec | Rust port over the owned XML tree; `DrawingColorTheme` trait instead of a plain object callback. |
| `rs/crates/nitro-xml-opc/src/drawingml/ordered_color_transforms.rs` | `src/core/drawingml/ordered-color-transforms.ts` @ e66aaec | Document-order colour transforms; same fractional-channel semantics as upstream. |
| `rs/crates/nitro-xml-opc/src/drawingml/fill.rs` | `src/core/drawingml/drawing-fill.ts` @ e66aaec | Fill/line readers; deterministic `sourceXml` via `serialize.rs` instead of DOM `XMLSerializer`. |
| `rs/crates/nitro-xml-opc/src/drawingml/geometry_parse.rs` | `src/core/drawingml/drawing-geometry.ts` @ e66aaec | Frame, transform and geometry readers over `Element`. |
| `rs/crates/nitro-xml-opc/src/drawingml/theme_model.rs` | `src/core/drawingml/theme-model.ts` @ e66aaec | Theme colour/font scheme types and default colour map. |
| `rs/crates/nitro-xml-opc/src/drawingml/theme.rs` | `src/core/drawingml/theme.ts` @ e66aaec | Theme part and colour-map parsing over the owned XML tree. |
| `rs/crates/nitro-xml-opc/src/drawingml/theme_color.rs` | `src/core/drawingml/theme-color.ts` @ e66aaec | Scheme colour resolution bridge to `resolve_drawing_color`. |
| `rs/crates/nitro-xml-opc/src/drawingml/guide_eval.rs` | `src/core/geometry/guide-formula-eval.ts` @ e66aaec | OOXML guide formula parser and evaluator. |
| `rs/crates/nitro-xml-opc/src/drawingml/guide_api.rs` | `src/core/geometry/guide-formula-api.ts` @ e66aaec | Built-in variables and ordered guide evaluation. |
| `rs/crates/nitro-xml-opc/src/drawingml/ooxml_arc.rs` | `src/core/geometry/ooxml-arc.ts` @ e66aaec | `a:arcTo` → SVG arc conversion (3-decimal rounding). |
| `rs/crates/nitro-xml-opc/src/drawingml/preset.rs` | `src/core/geometry/preset-shape-evaluator.ts`, `preset-text-rect-table.ts` @ e66aaec | Preset geometry loaded from pre-generated JSON tables; same formula and arc rules as upstream. |
| `rs/crates/nitro-xml-opc/src/drawingml/units.rs` | `src/core/units/units.ts` @ e66aaec | EMU/pixel helpers used by geometry readers. |
| `rs/crates/nitro-xml-docx/src/styles.rs` | `src/core/docx/paragraph-styles.ts`, `character-styles.ts` @ e66aaec | Style catalogs, `resolveParagraphFormatting`, `resolveStyleNumbering`. |
| `rs/crates/nitro-xml-docx/src/numbering.rs` | `src/core/docx/numbering-parse.ts`, `numbering-format.ts`, `numbering-text.ts` @ e66aaec | Numbering catalog, list labels, format helpers. |
| `rs/crates/nitro-xml-docx/src/theme.rs` | `src/core/docx/theme.ts`, `diagram-theme.ts` @ e66aaec | Theme/settings colour mapping and `diagramColorTheme` over shared DrawingML theme types (#25). |
| `rs/crates/nitro-xml-docx/src/settings.rs` | `src/core/docx/document-parts.ts`, `page-setup-model.ts` @ e66aaec | `parse_settings`, page setup read/validate. |
| `rs/crates/nitro-xml-docx/src/default_styles.rs` | `src/core/docx/default-styles.ts` @ e66aaec | Built-in `DEFAULT_STYLES_XML` equivalent via `default_styles_xml()`. |
| `rs/crates/nitro-xml-docx/src/run_properties.rs` | `src/core/docx/run-properties.ts` @ e66aaec | `parseRunProperties` for style catalogs. |
| `rs/crates/nitro-xml-docx/src/simple_types.rs` | `src/core/docx/simple-types.ts` @ e66aaec | Shared WML simple-type parsers. |
| `rs/crates/nitro-xml-docx/src/paragraph_alignment.rs` | `src/core/docx/paragraph-alignment.ts` @ e66aaec | `parseJustification` / `alignFromJustification`. |
| `rs/crates/nitro-xml-docx/src/borders.rs` | `src/core/docx/table-borders.ts` @ e66aaec | Paragraph border and shading parse subset. |
| `rs/crates/nitro-xml-docx/src/ligatures.rs` | `src/core/docx/ligatures.ts` @ e66aaec | Word 2010 `w14:ligatures` on `w:rPr`. |
| `rs/crates/nitro-xml-pptx/src/presentation.rs` | `src/core/pptx/core/core/runtime/PptxHandlerRuntimeLoadSession.ts`, `PptxHandlerRuntimeLoadPipeline.ts`, `PptxHandlerRuntimeSlideMasters.ts` @ e66aaec | Presentation open, slide list, sldSz/notesSz, master/layout chain, lazy slide parse, issue-24 JSON dump. |
| `rs/crates/nitro-xml-pptx/src/slide.rs` | `PptxHandlerRuntimeSpTreeParsing.ts`, `ShapeParsing.ts`, `PictureParsing.ts`, `GroupParsing.ts` (+ graphic-frame table) @ e66aaec | Element tree for ported kinds; transforms/geometry via DrawingML (#25); document-order spTree walk. |
| `rs/crates/nitro-xml-pptx/src/inheritance.rs` | `merge-xml-objects.ts`, `PptxHandlerRuntimePlaceholderLookup.ts`, `placeholder-style-family.ts` @ e66aaec | Placeholder match + layout-over-master merge (empty override must not clobber). |
| `rs/crates/nitro-xml-pptx/src/theme.rs` | `PptxHandlerRuntimeThemeLoading.ts` @ e66aaec | Theme dump via `drawingml::parse_theme` / `theme_slot_hex` (#25). |
