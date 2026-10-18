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
| `rs/crates/nitro-xml-opc/src/zip/mod.rs` | `src/core/xlsx/read/package.ts @ e66aaec` | Rust rewrite of readZipParts limits and SourceIndex case-insensitive lookup over the `zip` crate instead of JSZip; lazy per-part inflation; typed errors. `stream_part` stops inflate early; `parts_read` / `touched_parts` record which entries were inflated. |
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
| `rs/crates/nitro-xml-opc/src/opc/package.rs` | `src/core/xlsx/read/package.ts` (SourceIndex) and `src/core/opc/package.ts` @ e66aaec | `Package::open` over `ZipPackage` with format allow-list (PRODUCT_SPEC.md); properties/signatures listed only. Exposes `stream_part`, `parts_read` and `touched_parts` for the xlsx first-view path. |
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
| `rs/crates/nitro-xml-xlsx/src/model.rs` | `src/core/xlsx/model.ts` (style/theme/name types) @ e66aaec | Stage-3 subset plus Stage-16 `ActiveWorksheet` dump shape; `stringify_json` for JS number text parity. Optional `dimension` (omitted from JSON) carries lazy-view grid extent. |
| `rs/crates/nitro-xml-xlsx/src/xml_util.rs` | `src/core/xlsx/read/xml-util.ts`, `src/core/xml/xml.ts` (`relAttr`) @ e66aaec | SpreadsheetML helpers over the owned OPC element tree. |
| `rs/crates/nitro-xml-xlsx/src/address.rs` | `src/core/xlsx/address.ts` @ e66aaec | `parseRange` / `parseAddress`, `column_label`; `format_address` / `format_range` for shell selection (#43). |
| `rs/crates/nitro-xml-xlsx/src/builtin_formats.rs` | `src/core/xlsx/read/builtin-formats.ts @ e66aaec` | Thin wrapper over `numfmt::BUILTIN_NUMBER_FORMATS` plus locale id fallback for style resolution. |
| `rs/crates/nitro-xml-xlsx/src/password_hash.rs` | `src/core/xlsx/read/password-hash.ts @ e66aaec` | Agile hash attribute reader for workbook protection. |
| `rs/crates/nitro-xml-xlsx/src/style_parts.rs` | `src/core/xlsx/read/style-parts.ts @ e66aaec` | Fonts, fills, borders, colours, indexed palette. |
| `rs/crates/nitro-xml-xlsx/src/styles.rs` | `src/core/xlsx/read/styles.ts`, `src/core/xlsx/styles.ts` (`styleKey`, `styleAt`) @ e66aaec | Styles part, xf map, named styles, dxfs; `styleKey` via sorted JSON; `style_at` for display-text (#130). |
| `rs/crates/nitro-xml-xlsx/src/edit/style_patch.rs` | `src/core/xlsx/styles.ts` (`StylePatch`, `patchStyle`, `internStyle`, `applyStylePatch`) @ e66aaec | Style patch merge + intern for cell format edits (#66); JSON patch parse for edit scripts. |
| `rs/crates/nitro-xml-xlsx/src/edit/cells.rs` | `src/core/xlsx/edit/cell-values.ts`, `format.ts`, `context.ts` (`ensureCell`, `pruneCell`, `baseStyleId`) @ e66aaec | `writeInput` / `writeValue` / range style patch over `DumpCell` store; registry runners for #66. |
| `rs/crates/nitro-xml-xlsx/src/edit/structure.rs` | `src/core/xlsx/edit/sheets.ts`, `dimensions.ts`, `columns.ts`, sheet-name helpers in `workbook.ts` @ e66aaec | Sheet add/rename/delete/reorder + row/col size/hide (#67); formula sheet-ref rewrite deferred to formula transform port. |
| `rs/crates/nitro-xml-xlsx/src/edit/shift.rs` | `src/core/xlsx/formula/shift-formula.ts`, `src/core/xlsx/edit/structure.ts` (`insertRows` / `deleteRows` / `insertColumns` / `deleteColumns` / `applyAxisShift`), `range-math.ts`, `shift-sheet.ts`, `shift-formulas.ts`, `insert-guard.ts` @ e66aaec | Row/column insert and delete with formula reference shifting on the E1 worksheet (cells, merges, columns, row info, freeze, selection, hyperlinks, comments, validation ranges, defined names). Tables, conditional formats, drawings, charts, auto-filter, print area, and preserved extLst XML are TODO(#76). Template cache omitted; each rewrite tokenizes again (upstream's fresh path). |
| `rs/crates/nitro-xml-xlsx/src/shared_strings.rs` | `src/core/xlsx/read/shared-strings.ts @ e66aaec` | Plain and rich shared strings; phonetics ignored. |
| `rs/crates/nitro-xml-xlsx/src/theme.rs` | `src/core/xlsx/read/theme.ts` (+ minimal DrawingML colour/font scheme) @ e66aaec | SpreadsheetML palette slot order; latin major/minor fonts. |
| `rs/crates/nitro-xml-xlsx/src/workbook.rs` | `src/core/xlsx/read/load.ts`, `workbook-part.ts`, `workbook.ts` (defaults) @ e66aaec | Stage-3 load orchestration, Print_Area lift, sheet stubs, warnings; Stage-16 lazy `parse_worksheet` for the active sheet only. `load_workbook_shell` is the eager part shared with the first-view path; `load_workbook` still materialises shared strings and the active sheet. |
| `rs/crates/nitro-xml-xlsx/src/cells.rs` | `src/core/xlsx/cells.ts @ e66aaec` | Columnar parallel arrays + side maps instead of per-cell heap objects; numeric pool stores `js_number_string` text for gate dumps. |
| `rs/crates/nitro-xml-xlsx/src/worksheet.rs` | `src/core/xlsx/read/worksheet.ts @ e66aaec` | One worksheet part: split sheetData, shell parse (merges, cols, rows, sheetView), attach to workbook model. |
| `rs/crates/nitro-xml-xlsx/src/sheet_data.rs` | `src/core/xlsx/read/sheet-data.ts @ e66aaec` | Cell read path, formulas (shared/array/dynamic), warning strings; dump via `store_to_dump_cells`. |
| `rs/crates/nitro-xml-xlsx/src/sheet_data_source.rs` | `src/core/xlsx/read/sheet-data-source.ts @ e66aaec` | `split_sheet_data` string split; inner XML wrapped for tree parse (no full-sheet DOM). |
| `rs/crates/nitro-xml-xlsx/src/sheet_props.rs` | `src/core/xlsx/read/sheet-props.ts @ e66aaec` | Sheet format, default row/col sizes, column defs, row info, sheet view / freeze / selection. |
| `rs/crates/nitro-xml-xlsx/src/dates.rs` | `src/core/xlsx/read/dates.ts @ e66aaec` | `iso_to_serial` via `numfmt::date_to_serial` and 1900 leap-day quirks. |
| `rs/crates/nitro-xml-xlsx/src/metadata.rs` | `src/core/xlsx/read/metadata.ts @ e66aaec` | Dynamic-array metadata part for spill anchors. |
| `rs/crates/nitro-xml-xlsx/src/formula_text.rs` | `src/core/xlsx/read/formula-text.ts @ e66aaec` | `strip_future_prefixes` on read; `add_future_prefixes` writes `_xlfn.` / `_xlfn._xlws.` (spill/`@` storage still plain text). |
| `rs/crates/nitro-xml-xlsx/src/formula_translate.rs` | `src/core/xlsx/formula/transform.ts`, `tokenizer.ts`, `lexemes.ts`, `ref-text.ts @ e66aaec` | `translate_formula` for shared-formula slaves (relative A1 shift). |
| `rs/crates/nitro-xml-xlsx/src/formula_internal.rs` | `src/core/xlsx/formula/tokenizer.ts`, `lexemes.ts @ e66aaec` | Minimal tokenizer/rewriter used only by `formula_translate.rs` (not a full formula engine). |
| `rs/crates/nitro-xml-xlsx/src/formula/mod.rs` | `src/core/xlsx/formula/ast.ts`, `tokenizer.ts`, `parser.ts`, `transform.ts`, `engine.ts` @ e66aaec | Module root, #71 JSON dump, and calc-engine re-exports. |
| `rs/crates/nitro-xml-xlsx/src/formula/ast.rs` | `src/core/xlsx/formula/ast.ts` @ e66aaec | AST, sheet prefix, structured ref, function-name normalization. JSON key order matches `JSON.stringify`. |
| `rs/crates/nitro-xml-xlsx/src/formula/lexer.rs` | `src/core/xlsx/formula/tokenizer.ts`, `lexemes.ts` @ e66aaec | Tokenizer. UTF-16 offsets; error codes matched longest-first; 15-digit number literals. |
| `rs/crates/nitro-xml-xlsx/src/formula/parser.rs` | `src/core/xlsx/formula/parser.ts` @ e66aaec | Pratt parser. Whitespace intersection, parenthesized union, 64-call limit. No evaluation. |
| `rs/crates/nitro-xml-xlsx/src/formula/refs.rs` | `src/core/xlsx/formula/structured.ts`, `ref-text.ts`, `referencedRanges` / `referenceSpans` in `transform.ts` @ e66aaec | Structured refs, `format_ref_spec`, direct ranges and character spans. Shift/rename rewriters stay on #72+. |
| `rs/crates/nitro-xml-xlsx/src/formula/graph.rs` | `src/core/xlsx/formula/graph.ts`, `scc.ts`, `graph-index.ts`, `area-index.ts`, function-name/volatile flags from `functions/registry.ts` @ e66aaec | Static precedents, Tarjan order, column/area indexes. Function bodies are not ported; 481 names and 8 volatile flags are a static table. |
| `rs/crates/nitro-xml-xlsx/src/formula/calc.rs` | `src/core/xlsx/formula/engine.ts`, `engine-core.ts`, `engine-host.ts`, `engine-prepare.ts`, `engine-order.ts`, `engine-types.ts`, `engine-util.ts`, plus the scalar parts of `evaluator.ts`, `operators.ts`, `coerce.ts`, `text-number.ts`, `references.ts`, `values.ts`, `store.ts`, `spill.ts`, `date-serial.ts`, and `call.ts` @ e66aaec | Recalc scheduler and evaluator (literals, refs, arithmetic, compare, concat, names, structured refs, spills, catalogued function calls). Unknown names are `#NAME?`; unsupported formulas with a cached value stay cached. `replace_book` / `sync_input` / `export_calculated` copy the edit host in and formula values out (#77); no `shift_cells` / `rename_sheet`. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/mod.rs` | `src/core/xlsx/formula/functions/registry.ts`, `call.ts` @ e66aaec | Math & Trig, Statistical, Logical, Text, Information, Date & Time, Lookup & Reference, Financial, Engineering, and Database. `registered_functions` lists all 481 catalog names. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/spec.rs` | `src/core/xlsx/formula/functions/types.ts` @ e66aaec | `FunctionSpec`, parameter kinds, and lazy (`IF` / `LET` / `LAMBDA`) function pointers. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/value.rs` | `src/core/xlsx/formula/values.ts` @ e66aaec | Scalars, errors, matrices, and `LAMBDA` closures. A bare lambda result is `#CALC!`. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/text_num.rs` | `src/core/xlsx/formula/text-number.ts`, `date-serial.ts` (`ymdToSerial`, `serialToYmd`, `weekdayOf`, `serialSeconds`, `monthStartSerial`) @ e66aaec | General number text, date-serial coercion, and the 1900/1904 calendar used by date functions. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/coerce.rs` | `src/core/xlsx/formula/coerce.ts` @ e66aaec | `toNumber` / `toText` / `toBool` and 15-digit comparison. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/ops.rs` | `src/core/xlsx/formula/operators.ts` @ e66aaec | Arithmetic, comparisons, odd roots, array broadcast. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/host.rs` | `src/core/xlsx/formula/context.ts`, `call.ts` (`forEach` / `toMatrix` / `callLambda`), `references.ts` (`areaToMatrix`) @ e66aaec | Host slice the function families read, including lazy evaluation and lambda calls. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/helpers.rs` | `src/core/xlsx/formula/functions/helpers.ts` @ e66aaec | Numeric coercion, aggregate number collection, COUNTIF criteria. A lambda scalar is `#VALUE!`. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/criteria.rs` | `src/core/xlsx/formula/functions/criteria.ts` @ e66aaec | SUMIF / COUNTIF / AVERAGEIF / MAXIFS / MINIFS. The criteria index is skipped; a scan matches. A lambda criteria value is `#CALC!`. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/stats_core.rs` | `src/core/xlsx/formula/functions/stats-core.ts` @ e66aaec | Means, ranks, percentiles, correlation, Lanczos gamma. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/dist.rs` | `src/core/xlsx/formula/functions/dist-core.ts`, `distributions-f.ts` @ e66aaec | Gamma/beta/normal/t/chi/F and discrete probabilities. `erf` is shared with the Engineering `ERF` family. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/math.rs` | `src/core/xlsx/formula/functions/math.ts`, `math-sum.ts`, `math-mod.ts`, `math-combin.ts`, `math-more.ts`, `matrix.ts`, and ROMAN/ARABIC/BASE/DECIMAL in `engineering.ts` @ e66aaec | Math & Trig, including matrix functions and the four engineering-file names that are Math & Trig. `round_half_digits` is shared with `FIXED` / `DOLLAR`. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/stat.rs` | `src/core/xlsx/formula/functions/statistical.ts`, `statistical-count.ts`, `statistical-spread.ts`, `statistical-more.ts`, `statistical-shape.ts`, `hypothesis-tests.ts`, `regression.ts`, `regression-core.ts`, `distributions.ts`, `distributions-more.ts`, `distributions-f.ts` @ e66aaec | Statistical functions, hypothesis tests, regression, and distribution names. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/eval.rs` | `src/core/xlsx/formula/evaluator.ts`, `call.ts`, `engine-util.ts`, `references.ts`, `spill.ts` @ e66aaec | Call/evaluate path for the sweep, including lazy arguments, `LET` scope, and `LAMBDA` invocation. The harness clock is 2026-10-03 12:00 and honors `date1904`. Defined names, spill refs, and dependency-array spill stay on the calc engine. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/logical.rs` | `src/core/xlsx/formula/functions/logical.ts`, `lambda.ts` @ e66aaec | Logical functions: `AND` through `XOR`, `IF` / `IFS` / `SWITCH`, `LET`, `LAMBDA`, and `MAP` / `REDUCE` / `SCAN` / `BYROW` / `BYCOL` / `MAKEARRAY`. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/text.rs` | `src/core/xlsx/formula/functions/text.ts`, `text-case.ts`, `text-convert.ts`, `text-extra.ts`, `text-regex.ts` @ e66aaec | Text functions from `ARRAYTOTEXT` through `VALUETOTEXT`, including the `*B` aliases and `REGEX*`. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/info.rs` | `src/core/xlsx/formula/functions/info.ts` @ e66aaec | Information functions from `CELL` through `TYPE`. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/datetime.rs` | `src/core/xlsx/formula/functions/datetime.ts`, `workdays.ts`, `date-calc.ts` @ e66aaec | Date & Time functions from `DATE` through `YEARFRAC`, including `NETWORKDAYS` / `WORKDAY` and the 1904 date system. `serial_arg`, `add_months`, `days360` and `year_frac` are shared with financial functions. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/lookup.rs` | `src/core/xlsx/formula/functions/lookup.ts`, `lookup-core.ts`, `reference.ts`, `address-r1c1.ts`, `address.ts` (`quoteSheetName`) @ e66aaec | Lookup & Reference from `ADDRESS` through `XMATCH`, except the dynamic-array names in `array.rs`. `ANCHORARRAY` and spill refs are `#REF!` until the calc engine (`#72`). |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/array.rs` | `src/core/xlsx/formula/functions/array.ts`, `array-shape.ts` @ e66aaec | Dynamic-array Lookup & Reference names: `FILTER`, `SORT`, `SEQUENCE`, `UNIQUE`, and the shape helpers `TAKE` through `EXPAND`. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/financial.rs` | `src/core/xlsx/formula/functions/financial.ts`, `financial-core.ts`, `depreciation.ts`, `amorlinc.ts`, `coupons.ts`, `bonds.ts`, `securities.ts` @ e66aaec | Financial functions from `ACCRINT` through `YIELDMAT`. An omitted required `AMORLINC` slot is not `#N/A` (the evaluator has no `missingRequiredError`). |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/engineering.rs` | `src/core/xlsx/formula/functions/engineering.ts`, `complex.ts`, `convert.ts`, `distributions.ts` (`ERF` family) @ e66aaec | Engineering functions from `BIN2DEC` through `OCT2HEX`, including complex numbers, `CONVERT` and `ERF`. `ROMAN` / `ARABIC` / `BASE` / `DECIMAL` stay in `math.rs`. |
| `rs/crates/nitro-xml-xlsx/src/formula/fns/database.rs` | `src/core/xlsx/formula/functions/database.ts` @ e66aaec | `DAVERAGE` through `DVARP`. Criteria cells use `make_criteria`. A field reference is `#VALUE!` because the parameter is `any` and `scalar` rejects refs, matching upstream. |
| `rs/crates/nitro-xml-opc/src/drawingml/mod.rs` | `src/core/drawingml/index.ts` @ e66aaec | Module root re-exporting the DrawingML model API used by format crates. |
| `rs/crates/nitro-xml-opc/src/drawingml/types.rs` | `src/core/drawingml/types.ts` @ e66aaec | DrawingML model types (serde for gate dumps). |
| `rs/crates/nitro-xml-opc/src/drawingml/dom.rs` | `src/core/drawingml/dom.ts`, `src/core/xml/xml.ts` @ e66aaec | DrawingML DOM helpers over `nitro-xml-opc` `Element`. |
| `rs/crates/nitro-xml-opc/src/drawingml/color_primitives.rs` | `src/core/color/color-primitives.ts` @ e66aaec | Shared colour math for DrawingML resolution. |
| `rs/crates/nitro-xml-opc/src/drawingml/color_linear.rs` | `src/core/color/color-linear.ts` @ e66aaec | sRGB ↔ linear-light channel conversion for tint/shade. |
| `rs/crates/nitro-xml-opc/src/drawingml/color.rs` | `src/core/drawingml/drawing-color.ts` @ e66aaec | Rust port over the owned XML tree; `DrawingColorTheme` trait instead of a plain object callback. |
| `rs/crates/nitro-xml-opc/src/drawingml/ordered_color_transforms.rs` | `src/core/drawingml/ordered-color-transforms.ts` @ e66aaec | Document-order colour transforms; same fractional-channel semantics as upstream. |
| `rs/crates/nitro-xml-opc/src/drawingml/fill.rs` | `src/core/drawingml/drawing-fill.ts` @ e66aaec | Fill/line readers; deterministic `sourceXml` via `serialize.rs` instead of DOM `XMLSerializer`. |
| `rs/crates/nitro-xml-opc/src/drawingml/geometry_parse.rs` | `src/core/drawingml/drawing-geometry.ts` @ e66aaec | Frame, transform and geometry readers over `Element`. |
| `rs/crates/nitro-xml-opc/src/drawingml/theme_model.rs` | `src/core/drawingml/theme-model.ts` @ e66aaec | Theme colour/font scheme types and default colour map. `ThemeFormatScheme` is the `a:fmtScheme` fill/line/background matrix (effect styles deferred). |
| `rs/crates/nitro-xml-opc/src/drawingml/theme.rs` | `src/core/drawingml/theme.ts`, `PptxHandlerRuntimeThemeFormatScheme.ts` @ e66aaec | Theme part and colour-map parsing over the owned XML tree. Parses `a:fmtScheme` fillStyleLst / lnStyleLst / effectStyleLst / bgFillStyleLst (#27). |
| `rs/crates/nitro-xml-opc/src/drawingml/theme_color.rs` | `src/core/drawingml/theme-color.ts` @ e66aaec | Scheme colour resolution bridge to `resolve_drawing_color`. |
| `rs/crates/nitro-xml-opc/src/drawingml/guide_eval.rs` | `src/core/geometry/guide-formula-eval.ts` @ e66aaec | OOXML guide formula parser and evaluator. |
| `rs/crates/nitro-xml-opc/src/drawingml/guide_api.rs` | `src/core/geometry/guide-formula-api.ts` @ e66aaec | Built-in variables and ordered guide evaluation. |
| `rs/crates/nitro-xml-opc/src/drawingml/ooxml_arc.rs` | `src/core/geometry/ooxml-arc.ts` @ e66aaec | `a:arcTo` → SVG arc conversion (3-decimal rounding). |
| `rs/crates/nitro-xml-opc/src/drawingml/preset.rs` | `src/core/geometry/preset-shape-evaluator.ts`, `preset-text-rect-table.ts` @ e66aaec | Preset geometry loaded from pre-generated JSON tables; same formula and arc rules as upstream. |
| `rs/crates/nitro-xml-opc/src/drawingml/units.rs` | `src/core/units/units.ts` @ e66aaec | EMU/pixel helpers used by geometry readers. |
| `rs/crates/nitro-xml-docx/src/styles.rs` | `src/core/docx/paragraph-styles.ts`, `character-styles.ts` @ e66aaec | Style catalogs, `resolveParagraphFormatting`, `resolveStyleNumbering`. |
| `rs/crates/nitro-xml-docx/src/numbering.rs` | `src/core/docx/numbering-parse.ts`, `numbering-format.ts`, `numbering-text.ts` @ e66aaec | Numbering catalog, list labels, format helpers. |
| `rs/crates/nitro-xml-docx/src/theme.rs` | `src/core/docx/theme.ts`, `diagram-theme.ts` @ e66aaec | Theme/settings colour mapping and `diagramColorTheme` over shared DrawingML theme types (#25). |
| `rs/crates/nitro-xml-docx/src/default_styles.rs` | `src/core/docx/default-styles.ts` @ e66aaec | Built-in `DEFAULT_STYLES_XML` equivalent via `default_styles_xml()`. |
| `rs/crates/nitro-xml-docx/src/run_properties.rs` | `src/core/docx/run-properties.ts` @ e66aaec | `parseRunProperties` for style catalogs. |
| `rs/crates/nitro-xml-docx/src/simple_types.rs` | `src/core/docx/simple-types.ts` @ e66aaec | Shared WML simple-type parsers. |
| `rs/crates/nitro-xml-docx/src/paragraph_alignment.rs` | `src/core/docx/paragraph-alignment.ts` @ e66aaec | `parseJustification` / `alignFromJustification`. |
| `rs/crates/nitro-xml-docx/src/borders.rs` | `src/core/docx/table-borders.ts` @ e66aaec | Paragraph/table border and shading parse subset (#21 adds `parse_table_borders`). |
| `rs/crates/nitro-xml-docx/src/settings.rs` | `src/core/docx/document-parts.ts`, `page-setup-model.ts`, `settings.ts` @ e66aaec | `parse_settings`, page setup read/validate, `parse_track_review` (#21). |
| `rs/crates/nitro-xml-docx/src/ligatures.rs` | `src/core/docx/ligatures.ts` @ e66aaec | Word 2010 `w14:ligatures` on `w:rPr`. |
| `rs/crates/nitro-xml-pptx/src/presentation.rs` | `src/core/pptx/core/core/runtime/PptxHandlerRuntimeLoadSession.ts`, `PptxHandlerRuntimeLoadPipeline.ts`, `PptxHandlerRuntimeSlideMasters.ts`, `PptxHandlerRuntimeSlideUtils.ts` (`extractSlideNotes`), `notes-body-shapes.ts`, `PptxHandlerRuntimeMediaData.ts` (`decodeTiffToPngBlob`), `utils/svg-stretch-fill.ts` @ e66aaec | Presentation open, slide list, sldSz/notesSz, master/layout chain, lazy slide parse, issue-24 JSON dump. Speaker-notes text is the notes-body placeholder, else non-placeholder shapes (#86). Blip relationships become `data:` URLs at load (#27); TIFF → PNG; SVG `preserveAspectRatio="none"`. OLE VML preview rels resolve against the `vmlDrawing` part, not the slide. SmartArt `diagramDrawing` is taken from the data part's `.rels`, else from the owner slide/layout/master (stem `data1` → `drawing1` when several drawings are listed). |
| `rs/crates/nitro-xml-pptx/src/slide.rs` | `PptxHandlerRuntimeSpTreeParsing.ts`, `ShapeParsing.ts`, `PictureParsing.ts`, `GroupParsing.ts`, `PptxGraphicFrameParser.ts`, `PptxHandlerRuntimeBackgroundParsing.ts`, `PptxHandlerRuntimeThemeRefResolution.ts`, `PptxHandlerRuntimeImageEffects.ts` (`extractSvgBlipRelId`), `utils/vml-ole-preview.ts` @ e66aaec | Element tree; slide→layout→master solid/linear/blip background; `fillRef`/`lnRef`/`bgRef` resolved against the theme style matrix (`phClr`); picture `r:embed` including media posters and `asvg:svgBlip`/`a16:svgBlip` embeds; OLE graphic frames with VML `v:imagedata` preview rels; chart and SmartArt graphic frames as elements (#27). Image-fill custom geometry path serialization and `rotWithShape` attribute parsing (#27). |
| `rs/crates/nitro-xml-pptx/src/inheritance.rs` | `merge-xml-objects.ts`, `PptxHandlerRuntimePlaceholderLookup.ts`, `placeholder-style-family.ts` @ e66aaec | Placeholder match + layout-over-master merge (empty override must not clobber). |
| `rs/crates/nitro-xml-pptx/src/theme.rs` | `PptxHandlerRuntimeThemeLoading.ts` @ e66aaec | Theme dump via `drawingml::parse_theme` / `theme_slot_hex` (#25). Format scheme kept for paint, skipped in the issue-24 dump. |
| `rs/crates/nitro-xml-pptx/src/text.rs` | `PptxHandlerRuntimeShapeParagraphContentParsing.ts`, `PptxHandlerRuntimeBulletParsing.ts`, `ruby-element-parsing.ts` @ e66aaec | Shape text flattening for issue-24 dump parity (#26). |
| `rs/crates/nitro-xml-pptx/src/auto_number.rs` | `auto-number-format.ts`, `auto-number-scripts.ts` @ e66aaec | Auto-number bullet markers (#26). |
| `rs/crates/nitro-xml-pptx/src/level_style.rs` | `PptxHandlerRuntimePlaceholderStyles.ts`, `PptxHandlerRuntimeMasterElements.ts` @ e66aaec | Master/layout list-style bullets (#26). |
| `rs/crates/nitro-xml-pptx/src/placeholder_defaults.rs` | `PptxHandlerRuntimePlaceholderLookup.ts`, `PptxHandlerRuntimePlaceholderDefaults.ts` @ e66aaec | Placeholder level-style lookup (#26). |
| `rs/crates/nitro-xml-pptx/src/ink.rs` | `PptxGraphicFrameParser.ts` (`parseAinkTraceText`, `decodeAinkInk`) @ e66aaec | `aink:ink` / `aink:trace` decode for ink graphic frames (#27). |
| `rs/crates/nitro-xml-pptx/src/render/ink.rs` | `dom/elements/ink.ts`, `dom/elements/ink-stroke-svg.ts` @ e66aaec | Ink SVG path strokes in the slide painter (#27). |
| `rs/crates/nitro-xml-pptx/src/table.rs` | `PptxGraphicFrameParser.ts`, `core/builders/PptxTableDataParser.ts`, `render/table-style-fill.ts` @ e66aaec | Table graphic frame row count (#26). Cell text, spans, grid widths, row heights, solid fills, Medium Style 2 - Accent 1 built-in fallback, and package `tableStyles.xml` resolution via `table_style.rs` (#27). |
| `rs/crates/nitro-xml-pptx/src/table_style.rs` | `table-style-entry-parse.ts`, `table-style-fill-parse.ts`, `table-style-border-parse.ts`, `render/table-style.ts`, `render/table-style-borders.ts` @ e66aaec | Parse `ppt/tableStyles.xml` into a GUID map; layer fills, text, and borders per ECMA-376 section order for table painting (#27). |
| `rs/crates/nitro-xml-pptx/src/table_style_builtins.rs`, `table_style_builtins.json` | `render/table-style-builtins.ts`, `render/table-style-builtins-data.ts` @ e66aaec | PowerPoint built-in gallery table styles (74 GUIDs): catalogue payload verbatim, accent-family substitution, conversion to `TableStyleEntry`; replaces the hard-coded Medium Style 2 approximation (#27). |
| `rs/crates/nitro-xml-pptx/src/render/stage.rs` | `src/ui/src/pptx/dom/slide-stage.ts`, `src/ui/src/pptx/render/slide-background.ts` @ e66aaec | P1 stage + z-order; resolved solid/linear/blip slide background (`background-image:url(...)`) (#27); reading-surface `role`/`aria-roledescription`/`aria-label` (#42). Selection outline call (#43) is original. |
| `rs/crates/nitro-xml-pptx/src/render/shape.rs` | `dom/elements/text-shape.ts`, `render/visual-3d-materials.ts`, `render/stroke-outline.ts`, `render/svg-stroke-pattern-paint.ts`, `render/svg-gradient-paint.ts` @ e66aaec | P2 shape fill, stroke border, `box-shadow` from outerShdw (#27). Theme `effectRef` material overlay and inset shadow layering (#27). SVG pattern/gradient `a:ln` paint servers for centred outlines (#27). |
| `rs/crates/nitro-xml-pptx/src/visual_3d_materials.rs` | `src/ui/src/pptx/render/visual-3d-materials.ts` @ e66aaec | Metal/matte/softmetal CSS constants for theme `effectStyleLst/a:sp3d/@prstMaterial` (#27). |
| `rs/crates/nitro-xml-pptx/src/visual_3d_bevel.rs` | `src/ui/src/pptx/render/visual-3d.ts`, `visual-3d-bevel-light.ts` @ e66aaec | Theme `effectStyleLst/a:sp3d` bevel → inset `box-shadow` (#27). |
| `rs/crates/nitro-xml-pptx/src/outer_shadow.rs` | `render/visual-effects.ts`, `PptxShapeEffectStyleExtractor.ts` @ e66aaec | Parse `a:outerShdw` → CSS `box-shadow` (#27). |
| `rs/crates/nitro-xml-pptx/src/render/prst_geom.rs` | `render/shape-geometry-cascade.ts`, `core/geometry/shape-geometry.ts`, `geometry/preset-clip-paths-core.ts` @ e66aaec | Preset geometry CSS: ellipse `50%`, roundRect radius, arrow clip-paths (#27). |
| `rs/crates/nitro-xml-pptx/src/render/text.rs` | `dom/elements/text-block.ts`, `render/text-body-layout.ts`, `render/text-run-style.ts` @ e66aaec | P3 text body; `bodyPr` anchor + `pPr` align via flex; first-run `font-family` / bold / `-webkit-text-stroke` (#27). `a:rPr/@baseline` super/subscript CSS via `vertical-align` + font-size scale (#27). |
| `rs/crates/nitro-xml-pptx/src/east_asian_breaks.rs` | `render/text-east-asian-breaks.ts` @ e66aaec | Hanging punctuation + `eaLnBrk="0"` break opportunities for CJK paint (#27). |
| `rs/crates/nitro-xml-pptx/src/paragraph_trailing_space.rs` | `render/paragraph-trailing-space.ts` @ e66aaec | Trim trailing spaces on right/center-aligned paragraphs before paint (#27). |
| `rs/crates/nitro-xml-pptx/src/render/shape_autofit_resize.rs` | `src/ui/src/pptx/render/shape-autofit-resize.ts` @ e66aaec | `a:spAutoFit` shape height from post-apply `ScrollHeight` layout readback (#27). |
| `rs/crates/nitro-xml-pptx/src/render/font_substitution.rs` | `src/core/pptx/core/utils/font-substitution.ts` @ e66aaec | CSS `font-family` stacks via `getSubstituteFontFamily` (subset map + default sans chain) (#27). |
| `rs/crates/nitro-xml-pptx/src/render/field_substitution.rs` | `src/ui/src/pptx/render/text-field-substitution.ts`, `field-context.ts` @ e66aaec | `a:fld` slidenum/slidetitle/header/footer substitution at paint (#27). |
| `rs/crates/nitro-xml-pptx/src/text_layout.rs` | `text-body-properties-parser.ts`, `text-body-layout.ts`, `text-run-style.ts` @ e66aaec | Parse `a:bodyPr/@anchor`, insets, paragraph `algn` for paint. `a:rPr/@baseline` super/subscript shift parsing (BASELINE_FONT_SCALE = 0.65 matching upstream). |
| `rs/crates/nitro-xml-pptx/src/unicode_script.rs` | `unicode-script-detection.ts` @ e66aaec | Per-script font segmentation for run paint (#27). |
| `rs/crates/nitro-xml-pptx/src/text_body_rect.rs` | `text-body-rect.ts` @ e66aaec | Preset/custGeom text rectangle as extra padding on the text body. |
| `rs/crates/nitro-xml-pptx/src/verified_text_rect_presets.rs` | `verified-text-rect-presets.ts` @ e66aaec | COM-verified preset allowlist for geometry text rectangles. |
| `rs/crates/nitro-xml-pptx/src/render/smartart.rs` | `src/ui/src/pptx/dom/elements/smartart-svg.ts` @ e66aaec | P11 calls S2 cached-drawing SVG at the element box, with the slide theme scheme (#27). |
| `rs/crates/nitro-xml-render/src/smartart/mod.rs` | `src/ui/src/pptx/render/smartart-drawing.ts` (`projectDrawingShapes` / `resolveShapeKind`), `src/ui/src/pptx/dom/elements/smartart-svg.ts` @ e66aaec | S2/P11 cached drawing: preset + custGeom SVG paths, ellipse/roundRect, keep textless connectors; empty noFill placeholders still omitted. Layout fallback / 3D deferred. |
| `rs/crates/nitro-xml-render/src/smartart/circular_arrow.rs` | `src/core/geometry/adjustment-aware-shapes.ts` (`buildCircularArrow`) @ e66aaec | TS SmartArt paint silhouette for `circularArrow` (polygon sample, not ECMA `arcTo`) (#27). |
| `rs/crates/nitro-xml-render/src/smartart/text_wrap_estimate.rs` | `src/ui/src/pptx/render/text-wrap-estimate.ts` @ e66aaec | Estimated glyph advances + `wrapTextByEstimatedWidth` for SmartArt label wrap; `drawingTextBaseLineHeightRatio` CJK 1.33 line box (#27). |
| `rs/crates/nitro-xml-pptx/src/render/picture.rs` | `dom/elements/image.ts` @ e66aaec | P5 picture frame; img `src` and CSS background from the resolved blip data URL, including media posters (#27). Frame/image transforms, custom geometry clip-path, and rotWithShape counter-transform for image-fill (#27).  |
| `rs/crates/nitro-xml-pptx/src/render/image_fill_counter_transform.rs` | `src/ui/src/pptx/render/image-fill-counter-transform.ts @ e66aaec` | Counter-transform for image-fill `rotWithShape=0`: frame inverse (scaleY/scaleX/rotate) applied to img to keep content upright while the shape silhouette flips/rotates (#27). |
| `rs/crates/nitro-xml-pptx/src/render/ole.rs` | `dom/elements/ole.ts` @ e66aaec | P15 OLE preview `<img>` when `previewImageData` resolved; otherwise the `[OLE]` stub (#27). |
| `rs/crates/nitro-xml-pptx/src/render/group.rs` | `dom/elements/group.ts` @ e66aaec | P6 group (#27). |
| `rs/crates/nitro-xml-pptx/src/render/connector.rs` | `dom/elements/connector.ts` @ e66aaec | P7 connector SVG stub (#27). |
| `rs/crates/nitro-xml-pptx/src/render/connector_geometry.rs` | `src/core/pptx/core/geometry/connector-geometry.ts`, `connector-preset-geometry.ts` @ e66aaec | Bent/curved connector SVG paths via DrawingML preset eval (#27). |
| `rs/crates/nitro-xml-pptx/src/render/connector_markers.rs` | `src/ui/src/pptx/render/connector-markers.ts` @ e66aaec | Connector headEnd/tailEnd SVG `<marker>` defs (#27). |
| `rs/crates/nitro-xml-pptx/src/render/table.rs` | `dom/elements/table.ts` @ e66aaec | P8 HTML table: colgroup percentages, skip `hMerge`/`vMerge`, cell text, fill, and border (#27). |
| `rs/crates/nitro-xml-docx/src/body.rs` | `src/core/docx/parse.ts`, `block-parser.ts`, `document-parts.ts` @ e66aaec | `read_package`, block container parse, document parts orchestration; no save path. |
| `rs/crates/nitro-xml-docx/src/run.rs` | `src/core/docx/block-parser.ts` (`parseRun`) @ e66aaec | Run parse for body/HF/notes. |
| `rs/crates/nitro-xml-docx/src/table.rs` | `src/core/docx/parse-table.ts`, `write-table.ts` (`canEditTableStructure`) @ e66aaec | Table/cell parse and structureEditable. |
| `rs/crates/nitro-xml-docx/src/drawing.rs` | `src/core/docx/drawing.ts`, `diagram.ts` (host) @ e66aaec | Inline/anchor pictures, diagram/chart hosts; #28 deep chartSpace deferred. |
| `rs/crates/nitro-xml-docx/src/chart_host.rs` | `src/core/docx/chart.ts` (`parseChartGraphic` + light title/kind peek for layout labels) @ e66aaec | Full `chartSpace` parse remains #28. |
| `rs/crates/nitro-xml-docx/src/sections.rs` | `src/core/docx/sections.ts` @ e66aaec | Raw section geometry and header/footer refs. |
| `rs/crates/nitro-xml-docx/src/notes.rs` | `src/core/docx/notes.ts` @ e66aaec | Footnote/endnote part parse. |
| `rs/crates/nitro-xml-docx/src/revisions.rs` | `src/core/docx/parse-revisions.ts` @ e66aaec | Revision wrappers and `collectParagraphRuns`. |
| `rs/crates/nitro-xml-docx/src/breaks.rs` | `src/core/docx/breaks.ts` @ e66aaec | Page/column break classification. |
| `rs/crates/nitro-xml-docx/src/hyperlink.rs` | `src/core/docx/hyperlink.ts` @ e66aaec | Hyperlink and simple HYPERLINK field parse. |
| `rs/crates/nitro-xml-docx/src/equation.rs` | `src/core/docx/equation.ts` @ e66aaec | OMML equation runs. |
| `rs/crates/nitro-xml-docx/src/field_runs.rs` | `src/core/docx/field-runs.ts`, `field-flags.ts` @ e66aaec | Complex field result metadata and flags. |
| `rs/crates/nitro-xml-docx/src/comments.rs` | `src/core/docx/comments.ts` @ e66aaec | comments.xml / commentsExtended.xml. |
| `rs/crates/nitro-xml-docx/src/edit/revisions.rs` | `src/core/docx/revision-commands.ts` @ e66aaec | Model-level accept/reject (one/all) on `DocxHost` for edit/undo parity (#82). |
| `rs/crates/nitro-xml-docx/src/edit/restore_format.rs` | `src/core/docx/restore-run-format.ts`, `restore-paragraph-format.ts` @ e66aaec | Reject formatting revisions via snapshot XML on the edit host (#82). |
| `rs/crates/nitro-xml-docx/src/edit/comments.rs` | `src/core/docx/ui/comment-commands.ts` (list ops) @ e66aaec | Model-level add/reply/resolve/delete; PM `addComment` replaced with run `commentIds` (#82). |
| `rs/crates/nitro-xml-docx/src/warnings.rs` | `src/core/docx/parse-warnings.ts`, `parse.ts`, `diagram-document.ts` @ e66aaec | Document and model warning assembly; diagram warning text. |
| `rs/crates/nitro-xml-docx/src/tab_stops.rs` | `src/core/docx/tab-stops.ts` @ e66aaec | `w:pPr/w:tabs` parse for direct paragraph properties. |
| `rs/crates/nitro-xml-docx/src/watermark.rs` | `src/core/docx/watermark.ts` (`parseWatermarkShape`) @ e66aaec | VML text watermark shapes in `w:pict`. |
| `rs/crates/nitro-xml-docx/src/write/mod.rs` | `src/core/docx/save.ts` @ e66aaec | PackageWriter save entry, identity shortcut, dirty `word/document.xml` replace (#80). |
| `rs/crates/nitro-xml-docx/src/write/apply.rs` | `src/core/docx/write.ts` (`applyModel`) @ e66aaec | Block apply + page `sectPr` margins; HF/notes/comments TODO(#80). |
| `rs/crates/nitro-xml-docx/src/write/paragraph.rs` | `src/core/docx/write-paragraph.ts` @ e66aaec | Paragraph rewrite for edit scripts. |
| `rs/crates/nitro-xml-docx/src/write/run.rs` | `src/core/docx/write-run.ts` @ e66aaec | Run text and direct formatting write. |
| `rs/crates/nitro-xml-docx/src/write/table.rs` | `src/core/docx/write-table.ts` @ e66aaec | Table structure + cell paragraph rewrite. |
| `rs/crates/nitro-xml-docx/src/write/inline.rs` | `src/core/docx/write-inline.ts` (`collectInlineSlots`) @ e66aaec | Plain `w:r` slot collection subset. |
| `rs/crates/nitro-xml-docx/src/write/xml_escape.rs` | `src/core/xlsx/write/xml-out.ts` @ e66aaec | `_xHHHH_` text escapes shared with xlsx writer. |
| `rs/crates/nitro-xml-xlsx/src/sheet_access.rs` | `src/core/xlsx/cells.ts` @ e66aaec | `getCell` / `usedRange` / `mergeAt` over `ActiveWorksheet` `DumpCell` vec + `CellIndex`. |
| `rs/crates/nitro-xml-xlsx/src/drawing.rs` | `src/core/xlsx/read/drawing.ts` @ e66aaec | Spreadsheet drawing part (`xdr:wsDr`) → `DrawingObject` list (image/chart/unsupported); SmartArt deferred. |
| `rs/crates/nitro-xml-xlsx/src/view/anchors.rs` | `src/core/xlsx/layout/anchors.ts` @ e66aaec | Two-cell / one-cell / absolute anchors → plane pixel boxes at metrics zoom. |
| `rs/crates/nitro-xml-xlsx/src/view/units.rs` | `src/core/xlsx/layout/units.ts` @ e66aaec | Column/row pixel conversions; Rust `f64`/`i32` types. |
| `rs/crates/nitro-xml-xlsx/src/view/axis_metrics.rs` | `src/core/xlsx/layout/axis-metrics.ts` @ e66aaec | Sparse axis metrics; same prefix-sum algorithm. |
| `rs/crates/nitro-xml-xlsx/src/view/metrics.rs` | `src/core/xlsx/layout/metrics.ts` @ e66aaec | `createGridMetrics` over `ActiveWorksheet`. `dimension`, when set, extends `last_row` / `last_col` so a viewport-sized cell list does not shrink the grid. |
| `rs/crates/nitro-xml-xlsx/src/view/tint.rs` | `src/core/xlsx/layout/tint.ts` @ e66aaec | Excel HLS tint; local `parse_hex` instead of shared color crate. |
| `rs/crates/nitro-xml-xlsx/src/view/colors.rs` | `src/core/xlsx/layout/colors.ts`, `src/core/chart/render/theme-palette.ts` @ e66aaec | `resolveColor` / `themeColor` / indexed palette. |
| `rs/crates/nitro-xml-xlsx/src/view/style_view.rs` | `src/core/xlsx/layout/style-view.ts` @ e66aaec | Font/fill/border view resolution. |
| `rs/crates/nitro-xml-xlsx/src/view/cell_view.rs` | `src/core/xlsx/layout/cell-view.ts` @ e66aaec | `cellView` without CF evaluator; sparkline via `sparklineViewAt`. |
| `rs/crates/nitro-xml-xlsx/src/sparklines.rs` | `src/core/xlsx/read/sparklines.ts`, `model-sparklines.ts` @ e66aaec | `x14:sparklineGroup` read + `extLst` preserve. |
| `rs/crates/nitro-xml-xlsx/src/view/sparkline_view.rs` | `src/core/xlsx/layout/sparkline-view.ts` @ e66aaec | Unit-box sparkline layout for view-IR. |
| `rs/crates/nitro-xml-xlsx/src/view/sparkline_data.rs` | `src/core/xlsx/layout/sparkline-data.ts` @ e66aaec | `sparklineViewAt` + range value read. |
| `rs/crates/nitro-xml-xlsx/src/view/sheet_queries.rs` | `src/core/xlsx/layout/sheet-queries.ts` @ e66aaec | `overflowExtent` only (selection stats deferred). |
| `rs/crates/nitro-xml-xlsx/src/view/cell_paint.rs` | `src/core/xlsx/ui/grid/cell-paint.ts` @ e66aaec | CSS helpers (`cssFont`, borders, alignment, fill paint). |
| `rs/crates/nitro-xml-xlsx/src/view/cell_items.rs` | `src/core/xlsx/ui/grid/cell-items.ts` @ e66aaec | `buildItems` / `CellItem` geometry. |
| `rs/crates/nitro-xml-xlsx/src/view/number_fit.rs` | `src/core/xlsx/ui/grid/number-fit.ts` @ e66aaec | `fitNumber` via `format_value` width option. |
| `rs/crates/nitro-xml-xlsx/src/view/css_font_family.rs` | `src/core/text/css-font-family.ts` @ e66aaec | Font fallback stacks for measure/CSS. |
| `parity/visual/xlsx-grid-css.js` | `src/ui/src/xlsx/grid/styles.ts` (`GRID_CSS`) @ e66aaec | Visual-gate stylesheet for `<nitro-xlsx>` host (#150); not shipped in the loader. |
| `rs/crates/nitro-xml-xlsx/src/render/drawings.rs` | `src/ui/src/xlsx/grid/drawings.ts` @ e66aaec | `.xg-drawings` layer: unsupported placeholders + chart SVG; image object-URL paint deferred to loader. |
| `rs/crates/nitro-xml-xlsx/src/render/grid.rs` | `src/ui/src/xlsx/grid/grid-view.ts`, `grid-a11y.ts` @ e66aaec | Frame shell + `role=grid`/aria counts/live region (#42); full quadrants TODO(#19). |
| `rs/crates/nitro-xml-xlsx/src/render/cells.rs` | `src/ui/src/xlsx/grid/cell-layer.ts` @ e66aaec | Cell DOM command buffer (`paintCell` parity). |
| `rs/crates/nitro-xml-xlsx/src/cf/mod.rs` | `src/core/xlsx/read/conditional.ts`, `layout/cf-evaluator.ts`, `ui/grid/icon-sets.ts` @ e66aaec | Module root for CF read/eval/icon SVG (#150 soft). |
| `rs/crates/nitro-xml-xlsx/src/cf/read.rs` | `src/core/xlsx/read/conditional.ts` @ e66aaec | `cellIs` / `expression` / `colorScale` / `dataBar` / `iconSet` (+ Other stub). |
| `rs/crates/nitro-xml-xlsx/src/cf/eval.rs` | `src/core/xlsx/layout/cf-evaluator.ts`, `cf-helpers.ts`, `cf-values.ts`, `data-bar-geometry.ts` @ e66aaec | Icon/colour-scale/data-bar + `cellIs` stopIfTrue + dxf merge; full formula CF deferred. |
| `rs/crates/nitro-xml-xlsx/src/cf/expr.rs` | `src/core/xlsx/layout/cf-evaluator.ts` (literal / relative-ref path) @ e66aaec | Minimal CF expression eval for soft-corpus formulas (`MOD(ROW…)`, `$H$4="…"`, `OR`, `<>""`). |
| `rs/crates/nitro-xml-xlsx/src/cf/icon_sets.rs` | `src/core/xlsx/ui/grid/icon-sets.ts` @ e66aaec | SVG bodies for built-in icon sets. |
| `rs/crates/nitro-xml-xlsx/src/render/cond_format.rs` | `src/core/xlsx/ui/grid/icon-sets.ts`, cell-layer data bars @ e66aaec | Data-bar/colour-scale paint stub; icons paint in `cells`. |
| `rs/crates/nitro-xml-xlsx/src/render/gridlines.rs` | `src/ui/src/xlsx/grid/gridlines.ts` @ e66aaec | Per-row/column 1px gridline DOM nodes. |
| `rs/crates/nitro-xml-xlsx/src/render/headers.rs` | `src/ui/src/xlsx/grid/headers.ts`, `pane-chrome.ts` @ e66aaec | Row/column header + corner DOM; `role=rowheader|columnheader` (#42). |
| `rs/crates/nitro-xml-xlsx/src/render/geometry.rs` | `src/core/xlsx/ui/grid/geometry.ts` @ e66aaec | Header sizes plus `colAtX` / `rowAtY` / `hit` / `clampedHit` / `scrollToReveal`. |
| `rs/crates/nitro-xml-xlsx/src/render/viewport.rs` | `src/core/xlsx/layout/viewport.ts` @ e66aaec | `visibleCells` for grid paint culling. |
| `rs/crates/nitro-xml-xlsx/src/render/selection.rs` | `src/ui/src/xlsx/grid/selection-layer.ts` @ e66aaec | Selection overlay: range outline + active cell (shades/handle deferred). |
| `rs/crates/nitro-xml-xlsx/src/render/selection_ops.rs` | `src/core/xlsx/ui/grid/selection-ops.ts`, `src/core/xlsx/ui/selection.ts` (`rangeRef`), `src/ui/src/xlsx/grid/grid-commands.ts` (`currentRegionOrAll`), `src/core/xlsx/edit/filter.ts` (`currentRegion`) @ e66aaec | Merge expansion, select/extend, A1 selection refs, current region. Header state kept. |
| `rs/crates/nitro-xml-xlsx/src/render/navigate.rs` | `src/core/xlsx/layout/navigate.ts`, `src/ui/src/xlsx/grid/grid-selection.ts` @ e66aaec | Arrow navigation (hidden size 0 skipped) and merge far-edge departure. |
| `rs/crates/nitro-xml-xlsx/src/view/types.rs` | `src/core/xlsx/layout/types.ts` @ e66aaec | `CellView` / `CellItem` view types + JSON dump shape. |
| `rs/crates/nitro-xml-xlsx/src/view/viewport.rs` | `src/core/xlsx/layout/viewport.ts` @ e66aaec | View-IR bound helpers only (full viewport deferred #19). |
| `rs/crates/nitro-xml-xlsx/src/comments.rs` | `src/core/xlsx/read/comments.ts` @ e66aaec | Legacy comment cell refs for `hasComment`. |
| `rs/crates/nitro-xml-xlsx/src/hyperlinks.rs` | `src/core/xlsx/read/worksheet.ts` @ e66aaec | Internal + external hyperlink targets for `title`. |
| `rs/crates/nitro-xml-opc/src/drawingml/text.rs` | `src/core/drawingml/drawing-text.ts` @ e66aaec | Text body reader for chart `txPr` / rich text; `DrawingTextRunProperties` for `defaultProperties`. |
| `rs/crates/nitro-xml-opc/src/chart/mod.rs` | `src/core/chart/index.ts` @ e66aaec | Chart module root; read path only (no writers). |
| `rs/crates/nitro-xml-opc/src/chart/model.rs` | `src/core/chart/model*.ts`, `model-series.ts` @ e66aaec | Chart IR types (serde) aligned with upstream JSON dumps. |
| `rs/crates/nitro-xml-opc/src/chart/parse_space.rs` | `src/core/chart/parse-space.ts` @ e66aaec | `c:chartSpace` parser; chart XML uses `ParseOptions::sort_attributes = false`. |
| `rs/crates/nitro-xml-opc/src/chart/parse_util.rs` | `src/core/chart/parse-util.ts` @ e66aaec | Shared parse helpers, `extLst` / `spPr` / text body wiring. |
| `rs/crates/nitro-xml-opc/src/chart/parse_plot.rs` | `src/core/chart/parse-plot.ts` @ e66aaec | Plot area and chart groups read path. |
| `rs/crates/nitro-xml-opc/src/chart/parse_series.rs` | `src/core/chart/parse-series.ts` @ e66aaec | Series read path. |
| `rs/crates/nitro-xml-opc/src/chart/parse_chrome.rs` | `src/core/chart/parse-chrome.ts` @ e66aaec | Legend, axes, surface chrome read path. |
| `rs/crates/nitro-xml-opc/src/chart/parse_text.rs` | `src/core/chart/parse-text.ts` @ e66aaec | Title, data labels, number formats read path. |
| `rs/crates/nitro-xml-opc/src/chart/xml_serialize.rs` | `src/core/xml/xml.ts`, `src/core/chart/xml-fragment.ts` @ e66aaec | `buildXml`-compatible fragment serialization and `stripDeclarations`. |
| `rs/crates/nitro-xml-opc/src/chart/data_cache.rs` | `src/core/chart/data-cache.ts` @ e66aaec | Cached chart values from `strCache` / `numCache`. |
| `rs/crates/nitro-xml-opc/src/chart/manual_layout.rs` | `src/core/chart/manual-layout.ts` @ e66aaec | Manual layout values reader. |
| `rs/crates/nitro-xml-opc/src/chart/dom.rs` | `src/core/chart/parse-util.ts` (DOM helpers) @ e66aaec | Chart-namespace child lookup over `Element`. |
| `rs/crates/nitro-xml-render/src/chart/mod.rs` | `src/core/chart/render/render-space.ts` @ e66aaec | S1 entry (`render_chart_space_svg`, `paint_chart_space`); in-crate painter default (#28). |
| `rs/crates/nitro-xml-render/src/chart/render/mod.rs` | `src/core/chart/render/` @ e66aaec | S1 painter module root (`chart-rust-s1` feature). |
| `rs/crates/nitro-xml-render/src/chart/render/render_space.rs` | `src/core/chart/render/render-space.ts` @ e66aaec | S1 entry. |
| `rs/crates/nitro-xml-render/src/chart/render/svg.rs` | `src/core/chart/render/chart-svg.ts` @ e66aaec | S1 SVG document. |
| `rs/crates/nitro-xml-render/src/chart/render/svg_util.rs` | `src/core/chart/render/chart-svg-util.ts` @ e66aaec | SVG helpers. |
| `rs/crates/nitro-xml-render/src/chart/render/svg_cartesian.rs` | `src/core/chart/render/chart-svg-cartesian.ts` @ e66aaec | Cartesian painter; P9 plot layout + axis scale for `bar3D` extrusion (#27). |
| `rs/crates/nitro-xml-render/src/chart/render/chart_auto_title.rs` | `src/ui/src/pptx/render/chart-auto-title.ts` @ e66aaec | P9 `bar3D` auto title text (`CHART_AUTO_TITLE`) (#27). |
| `rs/crates/nitro-xml-render/src/chart/render/chart_axis_nice.rs` | `src/core/chart/axis-nice.ts` @ e66aaec | Plot-height-aware value-axis bounds for P9 `bar3D` layout (#27). |
| `rs/crates/nitro-xml-render/src/chart/render/chart_plot_layout.rs` | `src/ui/src/pptx/render/chart-view-model-layout.ts`, `chart-title-band.ts`, `chart-legend-placement.ts` @ e66aaec | P9 plot inset bands for `bar3D` extrusion (#27). |
| `rs/crates/nitro-xml-render/src/chart/render/svg_bars.rs` | `src/core/chart/render/chart-svg-bars.ts` @ e66aaec | Bar series; `bar3D` extrusion faces + picture pattern fills (#27). |
| `rs/crates/nitro-xml-render/src/chart/render/chart_3d_depth.rs` | `src/ui/src/pptx/render/chart-3d-depth.ts` @ e66aaec | Oblique `bar3D` top/side extrusion from `c:view3D` (#27). |
| `rs/crates/nitro-xml-render/src/chart/render/chart_bar3d_face_picture.rs` | `src/ui/src/pptx/render/chart-bar3d-face-picture.ts` @ e66aaec | `bar3D` untargeted extrusion face picture sampling; first-pixel colour (#27). |
| `rs/crates/nitro-xml-render/src/chart/render/png_first_pixel.rs` | `src/core/pptx/core/utils/image-first-pixel-png.ts` @ e66aaec | Synchronous PNG pixel (0,0) decode (no DOM) for `bar3D` untargeted face picture fallback; zlib-inflates IDAT via `flate2` (#27). |
| `rs/crates/nitro-xml-render/src/chart/render/chart_picture_fill.rs` | `src/ui/src/pptx/render/chart-datapoint-picture-fills.ts`, `chart-picture-pattern-def.ts` @ e66aaec | `c:dPt`/`c:ser` blip picture fills as SVG patterns (#27). |
| `rs/crates/nitro-xml-render/src/chart/render/chart_palette_shade.rs` | `src/ui/src/pptx/render/chart-style-palette.ts` (`shade`, `tint`) @ e66aaec | 3D bar extrusion face lighten/darken (#27). |
| `rs/crates/nitro-xml-render/src/chart/render/svg_radial.rs` | `src/core/chart/render/chart-svg-radial.ts` @ e66aaec | Pie/doughnut/radar. |
| `rs/crates/nitro-xml-render/src/chart/render/svg_legend.rs` | `src/core/chart/render/chart-svg-legend.ts` @ e66aaec | Legend layout. |
| `rs/crates/nitro-xml-render/src/chart/render/svg_appearance.rs` | `src/core/chart/render/chart-svg-appearance.ts` @ e66aaec | Chart area fills and gradients. |
| `rs/crates/nitro-xml-render/src/chart/render/svg_text_metrics.rs` | `src/core/chart/render/chart-svg-text-metrics.ts` @ e66aaec | Text width / fit. |
| `rs/crates/nitro-xml-render/src/chart/render/svg_lines.rs` | `src/core/chart/render/chart-svg-cartesian.ts` @ e66aaec | Line/area series. |
| `rs/crates/nitro-xml-render/src/chart/render/svg_scatter.rs` | `src/core/chart/render/chart-svg-cartesian.ts` @ e66aaec | Scatter plot. |
| `rs/crates/nitro-xml-render/src/chart/render/chart_view.rs` | `src/core/chart/render/chart-view.ts` @ e66aaec | View model. |
| `rs/crates/nitro-xml-render/src/chart/render/summary.rs` | `src/core/chart/render/summary.ts` @ e66aaec | Summary types. |
| `rs/crates/nitro-xml-render/src/chart/render/summary_from_space.rs` | `src/core/chart/render/summary-from-space.ts` @ e66aaec | Summary from `ChartSpace`. |
| `rs/crates/nitro-xml-render/src/chart/render/chart_scale.rs` | `src/core/chart/render/chart-scale.ts` @ e66aaec | Axis nice scale. |
| `rs/crates/nitro-xml-render/src/chart/render/chart_value_axis.rs` | `src/core/chart/render/chart-value-axis.ts` @ e66aaec | Value axis. |
| `rs/crates/nitro-xml-render/src/chart/render/chart_colors.rs` | `src/core/chart/render/chart-colors.ts` @ e66aaec | Theme scheme colours. |
| `rs/crates/nitro-xml-render/src/chart/render/color_palettes.rs` | `src/core/chart/color-palettes.ts` @ e66aaec | Office chart palettes. |
| `rs/crates/nitro-xml-render/src/chart/render/chart_palettes.json` | `src/core/chart/color-palettes.ts` @ e66aaec | Embedded palette data. |
| `rs/crates/nitro-xml-render/src/chart/render/theme_palette.rs` | `src/core/chart/render/theme-palette.ts` @ e66aaec | Theme slots. |
| `rs/crates/nitro-xml-render/src/chart/render/bar_cluster_geometry.rs` | `src/core/chart/bar-cluster-geometry.ts` @ e66aaec | Bar cluster sizing. |
| `rs/crates/nitro-xml-render/src/chart/render/chart_spacing.rs` | `src/core/chart/render/chart-spacing.ts` @ e66aaec | Bar gap/overlap. |
| `rs/crates/nitro-xml-render/src/chart/render/chart_appearance.rs` | `src/core/chart/render/chart-appearance.ts` @ e66aaec | Style resolution. |
| `rs/crates/nitro-xml-render/src/chart/render/built_in_text_style.rs` | `src/core/chart/built-in-text-style.ts` @ e66aaec | Built-in style 102 defaults. |
| `rs/crates/nitro-xml-render/src/chart/render/style_resolve.rs` | `src/core/chart/style-definition.ts` @ e66aaec | Resolved style entries. |
| `rs/crates/nitro-xml-render/src/chart/render/chart_title_text.rs` | `src/core/chart/render/chart-title-text.ts` @ e66aaec | Rich title IR from formatting `textBody` when present; S1 returns `None` without formatting (#28). |
| `rs/crates/nitro-xml-render/src/chart/render/drawing_color_css.rs` | `src/core/drawingml/drawing-color-css.ts` @ e66aaec | CSS colour string. |
| `rs/crates/nitro-xml-render/src/chart/render/drawing_shadow.rs` | `src/core/drawingml/drawing-shadow.ts` @ e66aaec | Outer shadow SVG. |
| `rs/crates/nitro-xml-render/src/chart/render/gradient_definition.rs` | `src/core/chart/gradient-definition.ts` @ e66aaec | Gradient defs. |
| `rs/crates/nitro-xml-render/src/chart/render/gradient_markup.rs` | `src/core/chart/gradient-markup.ts` @ e66aaec | Gradient SVG markup. |
| `rs/crates/nitro-xml-render/src/chart/render/gradient_stop_edit.rs` | `src/core/chart/gradient-stop-edit.ts` @ e66aaec | Stop ordering. |
| `rs/crates/nitro-xml-render/src/chart/render/gradient_coincident_stops.rs` | `src/core/chart/gradient-coincident-stops.ts` @ e66aaec | Coincident stops. |
| `rs/crates/nitro-xml-render/src/chart/render/sigma_gradient_stops.rs` | `src/core/color/sigma-gradient-stops.ts` @ e66aaec | Sigma profile stops. |
| `rs/crates/nitro-xml-render/src/chart/render/sigma_factors.rs` | `src/core/color/native-gradient-curve.ts` @ e66aaec | Office sigma factors table. |
| `rs/crates/nitro-xml-render/src/chart/render/css_font_family.rs` | `src/core/text/css-font-family.ts` @ e66aaec | Font stacks. |
| `rs/crates/nitro-xml-render/src/chart/render/manual_layout_resolve.rs` | `src/core/chart/manual-layout.ts` @ e66aaec | Manual layout rects. |
| `rs/crates/nitro-xml-render/src/chart/render/svg_title_text.rs` | `src/core/chart/render/chart-svg-title-text.ts` @ e66aaec | Rich / wrapped title SVG (`wrapStyledRuns`, #28). |
| `rs/crates/nitro-xml-render/src/chart/render/wrap_styled_runs.rs` | `src/core/text/wrap-styled-runs.ts` @ e66aaec | Format-neutral styled-run wrap (grapheme-aware `breakWords`). |
| `rs/crates/nitro-xml-render/src/chart/oracle_svg.rs` | — | MIT host-only bridge to `chart_ref.ts` for parity until Rust S1 lands. |
| `rs/crates/nitro-xml-docx/src/run_formatting.rs` | `src/core/docx/run-formatting.ts` @ e66aaec | `resolve_run_formatting` for layout adapter. |
| `rs/crates/nitro-xml-docx/src/theme_color.rs` | `src/core/docx/theme-color.ts` @ e66aaec | `resolve_theme_color_reference` for themed run colours in layout. |
| `rs/crates/nitro-xml-docx/src/review_document_formatting.rs` | `src/core/docx/review-document-formatting.ts` @ e66aaec | Review projection before layout (#22). |
| `rs/crates/nitro-xml-docx/src/review_formatting_display.rs` | `src/core/docx/review-formatting-display.ts` @ e66aaec | `is_run_hidden_for_review` for adapter. |
| `rs/crates/nitro-xml-docx/src/fields.rs` | `src/core/docx/field-runs.ts` (field name helpers) @ e66aaec | `field_name`, `is_field_locked`, `date_field_result` for layout runs. |
| `rs/crates/nitro-xml-docx/src/note_format.rs` | `src/core/docx/note-format.ts` @ e66aaec | Footnote/endnote label numbering for layout notes. |
| `rs/crates/nitro-xml-docx/src/table_styles.rs` | `src/core/docx/table-styles.ts` @ e66aaec | `parse_table_style_catalog` from `styles.xml` table styles. |
| `rs/crates/nitro-xml-docx/src/resolve_table.rs` | `src/core/docx/resolve-table.ts` @ e66aaec | `resolve_table_style_formatting` for conditional table styles. |
| `rs/crates/nitro-xml-docx/src/table_visuals.rs` | `src/core/docx/table-visuals.ts` @ e66aaec | `resolve_cell_visuals` layered with table styles + direct borders. |
| `rs/crates/nitro-xml-docx/src/layout/units.rs` | `src/core/docx/layout/units.ts` @ e66aaec | Twips/pt/px helpers for layout. |
| `rs/crates/nitro-xml-docx/src/layout/measure.rs` | `src/core/docx/layout/measure.ts` @ e66aaec | `TextMeasurer`, `create_fake_measurer`, `MeasureCache` / `CachedMeasurer`, `BatchedMeasurer` (#14 batch + span probes). |
| `rs/crates/nitro-xml-docx/src/layout/fonts.rs` | `src/core/docx/layout/fonts.ts` @ e66aaec | `css_font_stack` (wired in follow-up). |
| `rs/crates/nitro-xml-docx/src/layout/layout_ligatures.rs` | `src/core/docx/layout/ligatures.ts` @ e66aaec | Layout `ligature_css` helper. |
| `rs/crates/nitro-xml-docx/src/layout/text_breaks.rs` | `src/core/docx/layout/text-breaks.ts` @ e66aaec | UAX#14-lite line break tokens; `unicode-segmentation` for graphemes. |
| `rs/crates/nitro-xml-docx/src/layout/input.rs` | `src/core/docx/layout/input.ts` @ e66aaec | Layout input serde types. |
| `rs/crates/nitro-xml-docx/src/layout/result.rs` | `src/core/docx/layout/result.ts` @ e66aaec | `LayoutResult` and page/box types. Paragraph/table `kind` is `serde(skip)` so the `LayoutBlockBox` tag owns JSON `kind` and column blocks round-trip. |
| `rs/crates/nitro-xml-docx/src/layout/keep_rules.rs` | `src/core/docx/layout/keep-rules.ts` @ e66aaec | Widow/orphan and contextual spacing rules. |
| `rs/crates/nitro-xml-docx/src/layout/tab_stops.rs` | `src/core/docx/layout/tab-stops.ts` @ e66aaec | Tab placement during line layout. |
| `rs/crates/nitro-xml-docx/src/layout/util.rs` | `src/core/docx/expect-defined.ts` @ e66aaec | `expect_defined` helper used by layout. |
| `rs/crates/nitro-xml-docx/src/layout/wrap.rs` | `src/core/docx/layout/wrap.ts` @ e66aaec | Float wrap exclusions and line boxes. |
| `rs/crates/nitro-xml-docx/src/layout/line_metrics.rs` | `src/core/docx/layout/line-metrics.ts` @ e66aaec | Line height/baseline metrics. |
| `rs/crates/nitro-xml-docx/src/layout/paragraph_tokens.rs` | `src/core/docx/layout/paragraph-tokens.ts` @ e66aaec | Run tokenization and indents. |
| `rs/crates/nitro-xml-docx/src/layout/paragraph_fragments.rs` | `src/core/docx/layout/paragraph-fragments.ts` @ e66aaec | Fragment build/align. |
| `rs/crates/nitro-xml-docx/src/layout/paragraph_layout.rs` | `src/core/docx/layout/paragraph-layout.ts` @ e66aaec | Single-paragraph line breaking. |
| `rs/crates/nitro-xml-docx/src/layout/table_layout.rs` | `src/core/docx/layout/table-layout.ts` @ e66aaec | Table row/cell stack and split. |
| `rs/crates/nitro-xml-docx/src/layout/page_cursor.rs` | `src/core/docx/layout/page-cursor.ts` @ e66aaec | Lazy page/column cursor. |
| `rs/crates/nitro-xml-docx/src/layout/flow_paragraph.rs` | `src/core/docx/layout/flow-paragraph.ts` @ e66aaec | Paginate paragraphs. |
| `rs/crates/nitro-xml-docx/src/layout/flow_table.rs` | `src/core/docx/layout/flow-table.ts` @ e66aaec | Paginate tables (`&dyn TextMeasurer`). |
| `rs/crates/nitro-xml-docx/src/layout/floats.rs` | `src/core/docx/layout/floats.ts` @ e66aaec | Float positioning pass. |
| `rs/crates/nitro-xml-docx/src/layout/balance_columns.rs` | `src/core/docx/layout/balance-columns.ts` @ e66aaec | Column balance helpers (partial wire-up). |
| `rs/crates/nitro-xml-docx/src/layout/section_flow.rs` | `src/core/docx/layout/section-flow.ts` @ e66aaec | Section block flow; `step` resumes after each block for lazy pagination. |
| `rs/crates/nitro-xml-docx/src/layout/page_flow.rs` | `src/core/docx/layout/page-flow.ts` @ e66aaec | Multi-section layout orchestration (`&dyn TextMeasurer`). |
| `rs/crates/nitro-xml-docx/src/layout/adapt_drop_cap.rs` | `src/core/docx/layout/adapt-drop-cap.ts` @ e66aaec | Drop-cap folding. |
| `rs/crates/nitro-xml-docx/src/layout/adapt_floats.rs` | `src/core/docx/layout/adapt-floats.ts` @ e66aaec | Float metadata on paragraphs. |
| `rs/crates/nitro-xml-docx/src/layout/adapt_notes.rs` | `src/core/docx/layout/adapt-notes.ts` @ e66aaec | Footnote/endnote layout paragraphs. |
| `rs/crates/nitro-xml-docx/src/layout/adapt_paragraph_box.rs` | `src/core/docx/layout/adapt-paragraph-box.ts` @ e66aaec | Paragraph border/shading for layout. |
| `rs/crates/nitro-xml-docx/src/layout/adapt_table.rs` | `src/core/docx/layout/adapt-table.ts` @ e66aaec | Table block adapter. |
| `rs/crates/nitro-xml-docx/src/layout/review_display.rs` | `src/core/docx/layout/review-display.ts` @ e66aaec | Layout review options. |
| `rs/crates/nitro-xml-docx/src/layout/adapter.rs` | `src/core/docx/layout/adapter.ts` @ e66aaec | `DocumentModel` → layout input. |
| `rs/crates/nitro-xml-docx/src/layout/layout.rs` | `src/core/docx/layout/layout.ts` @ e66aaec | `layout_document` / `layout_document_model` take `&dyn TextMeasurer`; measured-cache loop. |
| `rs/crates/nitro-xml-docx/src/layout/page_fields.rs` | `src/core/docx/layout/page-fields.ts` @ e66aaec | PAGE/NUMPAGES field helpers, used by Print Layout headers and footers. |
| `rs/crates/nitro-xml-docx/src/render/print/pages.rs` | `src/ui/src/docx/print-layout.ts` @ e66aaec | Print Layout pages, columns, paragraphs and footnotes as one command buffer per frame. |
| `rs/crates/nitro-xml-docx/src/render/print/styles.rs` | `src/ui/src/docx/styles/print.css` @ e66aaec | `.dve-print-*` stylesheet injected once per frame. |
| `rs/crates/nitro-xml-docx/src/render/print/table.rs` | `src/ui/src/docx/print-table.ts` @ e66aaec | Table, row and cell DOM. |
| `rs/crates/nitro-xml-docx/src/render/print/picture.rs` | `src/ui/src/docx/print-picture.ts` @ e66aaec | Inline and floating pictures; chart SVG when a chart part is present. |
| `rs/crates/nitro-xml-docx/src/render/print/header_footer.rs` | `src/ui/src/docx/print-header-footer.ts` @ e66aaec | Header, footer, watermark and floating header pictures. |
| `rs/crates/nitro-xml-docx/src/render/print/page_borders.rs` | `src/ui/src/docx/print-page-borders.ts` and `src/core/docx/ui/table-render.ts` (`cssBorderSide`) @ e66aaec | Page border box. |
| `rs/crates/nitro-xml-docx/src/render/print/line_numbers.rs` | `src/ui/src/docx/print-line-numbers.ts` @ e66aaec | Line-number labels. |
| `rs/crates/nitro-xml-docx/src/render/print/options.rs` | `src/ui/src/docx/print-layout.ts` (`PrintLayoutOptions`) @ e66aaec | Painter inputs besides `LayoutResult`. |
| `rs/crates/nitro-xml-wasm/src/chrome/shortcuts.rs` | `src/ui/src/xlsx/keyboard.ts` @ e66aaec | Rust `SHORTCUTS` table and `matches_keys` / `find_shortcut` / `format_keys`; platform flag passed in (no `navigator` in unit tests). |
| `rs/crates/nitro-xml-wasm/src/chrome/ribbon.rs` | `src/ui/src/xlsx/ribbon/tabs/home.ts`, `home-groups.ts`, `ribbon.ts` (`layoutSmall`) @ e66aaec | Declarative Home tab groups as static HTML; galleries/menus stubbed (TODO #54). `layoutSmall` stacks + dialog launchers + `data-label` for edit-chrome pixel parity (#150). |
| `parity/visual/ribbon-overflow.mjs` | `src/ui/src/ribbon/overflow.ts` @ e66aaec | Fold-only panel fit for edit-chrome visual host (loader budget; product ResizeObserver attach deferred). |
| `rs/crates/nitro-xml-wasm/src/chrome/ribbon_icons.rs` | `src/core/xlsx/ui/commands/icons.ts` (`SHEET_ICONS`), `ribbon/icon-paths.ts` (caret/launcher) @ e66aaec | 20×20 sheet glyphs (stroke 1.5) + 24×24 caret/launcher for Home edit-chrome parity (#150). |
| `rs/crates/nitro-xml-wasm/src/chrome/ribbon_skin.css` | `src/ui/src/xlsx/theme/ribbon.css` @ e66aaec | Embedded Home ribbon skin for WASM chrome HTML (#150). |
| `rs/crates/nitro-xml-wasm/src/text/clipboard.rs` | `src/core/xlsx/edit/clipboard-text.ts`, `clipboard-html.ts` (`escapeHtml`) @ e66aaec | TSV quote/parse/toTsv and `escape_html` / plain HTML table for the single-line text surface (#55); styled multi-cell `toHtml` remains `TODO(#69)`. |
| `rs/crates/nitro-xml-wasm/src/chrome/focus.rs` | `src/ui/src/dialog/focus.ts`, Tab trap in `dialog/dialog.ts` @ e66aaec | Focus trap index math + restore stack for modal dialogs/menus (#54). |
| `rs/crates/nitro-xml-wasm/src/chrome/dialog.rs` | `src/ui/src/dialog/dialog.ts`, `src/ui/src/xlsx/dialogs/frame.ts` @ e66aaec | Modal dialog frame HTML (`role=dialog`, `aria-modal`, OK/Cancel); concrete bodies stay follow-ups. |
| `rs/crates/nitro-xml-wasm/src/chrome/menu.rs` | `src/ui/src/menu/menu-model.ts` (+ context-menu / menu-button behaviour) @ e66aaec | `next_enabled_index`, `type_ahead_index`, `clamp_flyout_position`, menu/context-menu HTML (#54). |
| `rs/crates/nitro-xml-opc/src/drawingml/edit.rs` | `src/core/pptx/core/utils/xfrm-emu-resolution.ts` @ e66aaec | `is_xfrm_emu_unchanged` / `resolve_xfrm_emu` (JS `Math.round`). Fill and line patch helpers in the same file are original. |
| `rs/crates/nitro-xml-pptx/src/edit/shapes.rs` | `src/core/pptx/editor/render/element-operations.ts`, `remap-text.ts`, `editor-mutations.ts`, `src/core/pptx/core/utils/group-ops.ts` @ e66aaec | Z-order, duplicate offset, plain-text remap, group/ungroup bbox. Command runners and the editable host are original (#84). |
| `rs/crates/nitro-xml-xlsx/src/write/xml_out.rs` | `src/core/xlsx/write/xml-out.ts` @ e66aaec | Escapes, `el` / `t_element`, and `relative_target`. |
| `rs/crates/nitro-xml-xlsx/src/write/style_xml.rs` | `src/core/xlsx/write/style-xml.ts` @ e66aaec | Font, fill, border, alignment, protection, and numFmt fragments. |
| `rs/crates/nitro-xml-xlsx/src/write/shared_strings.rs` | `src/core/xlsx/write/shared-strings.ts` @ e66aaec | Shared-string table built while sheets are written (`count` / `uniqueCount`). |
| `rs/crates/nitro-xml-xlsx/src/write/theme.rs` | `src/core/xlsx/write/theme.ts` @ e66aaec | Carry an unchanged theme; otherwise the default Office theme. Palette slot order stays spreadsheet `lt1, dk1, …`. |
| `rs/crates/nitro-xml-xlsx/src/write/styles.rs` | `src/core/xlsx/write/styles.ts` @ e66aaec | `StyleWriter` for cell xfs. Source numFmt ids and dxf/tableStyles markup are copied. |
| `rs/crates/nitro-xml-xlsx/src/write/sheet_data.rs` | `src/core/xlsx/write/sheet-data.ts` @ e66aaec | Row-major `sheetData`. Dynamic-array formulas are `t="array"` anchors with `aca="false"` and the XLDAPR `cm` index. Cell formulas get `add_future_prefixes` on write. |
| `rs/crates/nitro-xml-xlsx/src/write/sheet_props.rs` | `src/core/xlsx/write/sheet-props.ts` @ e66aaec | Subset: dimension, sheetViews, sheetFormat, cols, mergeCells. |
| `rs/crates/nitro-xml-xlsx/src/write/workbook_part.rs` | `src/core/xlsx/write/workbook-part.ts` @ e66aaec | Workbook part. `fullCalcOnLoad="1"` is optional so the negative control can omit it. |
| `rs/crates/nitro-xml-xlsx/src/write/dynamic_array.rs` | `src/core/xlsx/write/dynamic-array.ts`, `src/core/xlsx/formula/array-context.ts` @ e66aaec | XLDAPR `metadata.xml` constant and `needs_array_evaluation`. Call parameter kinds are not consulted (no function registry); unknown calls only propagate child `needs`. |
| `rs/crates/nitro-xml-xlsx/src/write/worksheet.rs` | `src/core/xlsx/write/worksheet.ts` @ e66aaec | Worksheet shell in `CT_Worksheet` order. Unmodelled children are copied from the source part; carried `sqref`s follow the edit shift log. |
| `rs/crates/nitro-xml-pptx/src/write/slide_xml.rs` | `src/core/pptx/core/core/runtime/PptxHandlerRuntimeSaveElementWriter.ts`, `PptxHandlerRuntimeSaveRunProperties.ts`, `src/core/pptx/core/core/builders/PptxElementTransformUpdater.ts`, `PptxShapeIdValidator.ts`, `src/core/pptx/core/builders/factories/TextShapeXmlFactory.ts` @ e66aaec | Dirty-slide rewrite: `resolve_xfrm_emu` writeback, plain text / rPr, solid/none fill, line, new-shape XML, two-pass shared-node shape-id repair. Package save entry (`write/mod.rs`) is original (#87). |
| `rs/crates/nitro-xml-xlsx/src/editor/formula_bar.rs` | `src/ui/src/xlsx/formula-bar/index.ts`, `formula-input.ts`, `styles.ts` @ e66aaec | Name box, cancel/enter/fx and formula field as HTML. The field is a Rust text surface (`role="textbox"`), not a `<textarea>`. Read-only returns an empty fragment. |
| `rs/crates/nitro-xml-xlsx/src/editor/input_text.rs` | `src/core/xlsx/edit/input-text.ts` @ e66aaec | Formula-bar text (`=formula`, dates, percents, quote-prefix). 15-digit rounding uses `log10` + ryu instead of `Number.prototype.toPrecision`. |
| `rs/crates/nitro-xml-xlsx/src/editor/commands.rs` | `src/core/xlsx/ui/commands/font.ts`, `alignment.ts`, `number.ts` @ e66aaec | Home ribbon ids that map onto `applyStyle` / `setCellInput` / sheet ops. Dialogs, borders, and conditional formats stay unmapped. |
| `rs/crates/nitro-xml-xlsx/src/editor/session_calc.rs` | `src/core/xlsx/edit/session-calc.ts` @ e66aaec | Session calculator: incremental `recalculate_from` after cell edits; full `invalidate` + `recalculate_all` after structural edits because the engine has no `shift_cells` / `rename_sheet`. Formula values are written back onto `XlsxHost`. |
| `rs/crates/nitro-xml-pptx/src/inkml.rs` | `src/core/pptx/core/utils/inkml-content-part.ts`, `inkml-trace-decode.ts`, `inkml-ink-space.ts` @ e66aaec | InkML `p:contentPart` decode: difference-encoded traces, nested brushes with `cm` widths, normalise onto element box. |
| `rs/crates/nitro-xml-pptx/src/render/content_part.rs` | `src/ui/src/pptx/dom/elements/contentpart.ts` @ e66aaec | Content-part ink SVG painter (`pptxv-contentpart-svg`). |
| `rs/crates/nitro-xml-pptx/src/edit/slides.rs` | `src/core/pptx/editor/render/slide-operations.ts`, `src/core/pptx/core/builders/sdk/Presentation-slides.ts`, `viewers/pptx/packages/svelte/src/viewer/editor/editor-slide-ops.ts` @ e66aaec | Blank-slide insert, delete (keep one), duplicate-after, reorder. Ids are deterministic (`added-slide-N`, `dup-slide-N`) so the model snapshot matches Rust; package rels/media/content types stay on the TS save reconciler (#86). |
| `rs/crates/nitro-xml-pptx/src/edit/text.rs` | `src/core/pptx/editor/render/bullet-toggle.ts`, `src/ui/src/pptx/render/text-advanced.ts` (`autoFitModePatch`) @ e66aaec | Run style, paragraph align, bullets and autofit on slide text (#85). |
| `rs/crates/nitro-xml-pptx/src/edit/table.rs` | `src/core/pptx/editor/render/table-layout.ts`, `table-cell-edit.ts` @ e66aaec | Slide table cell text and row/column insert/delete (#85). |

| `rs/crates/nitro-xml-pptx/src/render/pattern_tile.rs` | `src/ui/src/pptx/render/fill-style.ts` (`getPatternTile`) @ e66aaec | OOXML `a:pattFill` tile markup for SVG pattern stroke paint (#27). |
| `rs/crates/nitro-xml-pptx/src/render/compound_line.rs` | `src/ui/src/pptx/render/connector-style.ts` (`getCompoundLineOffsets`, `getCompoundLineWidths`) @ e66aaec | OOXML `a:ln/@cmpd` (sng|dbl|thickThin|thinThick|tri) offsets and strand widths for multi-path compound stroke rendering (#27). |
| `rs/crates/nitro-xml-pptx/src/render/wordart.rs` | `src/ui/src/pptx/render/text-warp.ts` @ e66aaec | Fixture-scoped path-based text warp presets (priorities 1–4): archUp/Down, circle, inflate/deflate, curveUp/Down, canUp/Down, slantUp/Down, fadeRight/Left/Up/Down, cascadeUp/Down. Envelope preset path fallbacks for paint (#27). |
| `rs/crates/nitro-xml-pptx/src/render/wordart_preset_definitions.rs` | `src/ui/src/pptx/render/text-warp-preset-definitions.ts` @ e66aaec | Spec-transcribed OOXML `prstTxWarp` guide formulas for WordArt envelope presets (inflate/deflate/can). Definitions include adjustments, guide lists, and top/bottom curve segment descriptions for 10 presets (#27). |
| `rs/crates/nitro-xml-pptx/src/render/wordart_preset_sampler.rs` | `src/ui/src/pptx/render/text-warp-preset-sampler.ts` @ e66aaec | Guide-formula sampler for spec-transcribed preset top/bottom paths. Uses `evaluate_guides` to resolve `adj`-dependent curves exactly as PowerPoint. Segments include line/quad/cubic/arc sampling with closed-form elliptical arc to x-coordinate solving (#27). |
| `rs/crates/nitro-xml-pptx/src/render/wordart_envelope_curves.rs` | `src/ui/src/pptx/render/text-warp-envelope-curves.ts` @ e66aaec | Two-curve envelope maths for glyph-envelope presets (inflate/deflate/can); calls sample_warp_preset_curve FIRST (real OOXML guide formulas), then falls back to hump/tripleLobe formulas for robustness (#27). |
| `rs/crates/nitro-xml-pptx/src/render/wordart_envelope_map.rs` | `src/ui/src/pptx/render/text-warp-envelope-map.ts` @ e66aaec | PowerPoint's arc-length point mapping for two-curve envelope presets; arc-length normalization with 512-sample tables (#27). |
| `rs/crates/nitro-xml-pptx/src/render/wordart_envelope_affine.rs` | `src/ui/src/pptx/render/text-warp-glyph-slicing.ts` @ e66aaec | Affine / piecewise-affine fallback for envelope glyphs; least-squares fit with slicing when tolerance exceeded (#27). |
| `rs/crates/nitro-xml-pptx/src/render/wordart_envelope_outline_warp.rs` | `src/ui/src/pptx/render/text-warp-glyph-outline.ts` @ e66aaec | Per-point warp of traced glyph contours through the two-curve envelope map; builds SVG `path` `d` for envelope WordArt paint (#27). |
| `packages/nitro-xml/loader/lazy/glyph-outline.js` | (original host; algorithm mirrors `text-warp-glyph-trace-contour.ts` marching-squares + Douglas-Peucker at TRACE_SIZE 512) | Offscreen-canvas glyph contour tracing for `OutlineBatch`; dynamic-`import()`ed only when `outlineCount > 0` (excluded from first-load loader budget) (#27). |
| `rs/crates/nitro-xml-pptx/src/visual_3d_camera_homography_math.rs` | `src/ui/src/pptx/render/visual-3d-camera-homography-math.ts` @ e66aaec | Generic "unit square → quadrilateral" projective homography solver; same Heckbert closed form for 3D camera parametric mapping (#27). |
| `rs/crates/nitro-xml-pptx/src/visual_3d_camera_parametric.rs` | `src/ui/src/pptx/render/visual-3d-camera-parametric.ts` @ e66aaec | Parametric camera homography for explicit `a:camera/a:rot` override; 27-point COM grid fit for orthographic projection with cross-term y-foreshortening (#27). |
| `rs/crates/nitro-xml-pptx/src/visual_3d_camera_homography.rs` | `src/ui/src/pptx/render/visual-3d-camera-homography.ts` @ e66aaec | Homography → matrix3d CSS transform with per-element size rescaling; transform-origin: 0 0 pairing (#27). |
| `rs/crates/nitro-xml-pptx/src/render/chart_vm.rs` | `src/ui/src/pptx/render/chart-cartesian*.ts`, `chart-view-model-layout.ts`, `chart-axis-render.ts`, `chart-category-axis.ts`, `chart-legend-*.ts`, `chart-style-palette.ts`, `chart-view-model-dom.ts` @ e66aaec | pptx chart view-model engine, flat clustered-column subset (#27); the legend painter is shared with `chart_vm_pie.rs`. Unported features return `None` and fall back to the S1 painter. |
| `rs/crates/nitro-xml-pptx/src/render/chart_vm_pie.rs` | `src/ui/src/pptx/render/chart-view-model-pie.ts`, `chart-view-model-points.ts` (`computePieLayout`, `computePieSlices`, `computePieSlicePath`, `PIE_AUTO_INSET_PX`), `chart-title-band.ts`, `chart-legend-placement.ts`, `chart-view-model-dom.ts` @ e66aaec | pptx pie / doughnut view-model with square `viewBox` + `xMidYMid meet` (#27). `legendPos=r` keeps the UI builder's bottom-centred row. Data labels, `c:dPt` fills, explosion and manual layouts return `None` and fall back to S1. |
| `rs/crates/nitro-xml-pptx/src/table_cell_text.rs` | `src/core/pptx/core/core/builders/table-cell-runs.ts`, `table-cell-paragraphs.ts`, `table-cell-text-style-helpers.ts`, `src/ui/src/pptx/render/table-cell-paragraphs.ts`, `table-cell-padding.ts` @ e66aaec | Rich table cell text: runs, paragraph blocks, first-run cell style, tcMar padding (#27). |
