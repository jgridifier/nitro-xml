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
| `rs/crates/nitro-xml-xlsx/src/model.rs` | `src/core/xlsx/model.ts` (style/theme/name types) @ e66aaec | Stage-3 subset of model types with JSON dump helpers for the differential gate. |
| `rs/crates/nitro-xml-xlsx/src/xml_util.rs` | `src/core/xlsx/read/xml-util.ts`, `src/core/xml/xml.ts` (`relAttr`) @ e66aaec | SpreadsheetML helpers over the owned OPC element tree. |
| `rs/crates/nitro-xml-xlsx/src/address.rs` | `src/core/xlsx/address.ts` @ e66aaec | `parseRange` / `parseAddress` for Print_Area lift only. |
| `rs/crates/nitro-xml-xlsx/src/builtin_formats.rs` | `src/core/xlsx/read/builtin-formats.ts @ e66aaec` | Thin wrapper over `numfmt::BUILTIN_NUMBER_FORMATS` plus locale id fallback for style resolution. |
| `rs/crates/nitro-xml-xlsx/src/password_hash.rs` | `src/core/xlsx/read/password-hash.ts @ e66aaec` | Agile hash attribute reader for workbook protection. |
| `rs/crates/nitro-xml-xlsx/src/style_parts.rs` | `src/core/xlsx/read/style-parts.ts @ e66aaec` | Fonts, fills, borders, colours, indexed palette. |
| `rs/crates/nitro-xml-xlsx/src/styles.rs` | `src/core/xlsx/read/styles.ts`, `src/core/xlsx/styles.ts` (`styleKey`, `styleAt`) @ e66aaec | Styles part, xf map, named styles, dxfs; `styleKey` via sorted JSON; `style_at` for display-text (#130). |
| `rs/crates/nitro-xml-xlsx/src/shared_strings.rs` | `src/core/xlsx/read/shared-strings.ts @ e66aaec` | Plain and rich shared strings; phonetics ignored. |
| `rs/crates/nitro-xml-xlsx/src/theme.rs` | `src/core/xlsx/read/theme.ts` (+ minimal DrawingML colour/font scheme) @ e66aaec | SpreadsheetML palette slot order; latin major/minor fonts. |
| `rs/crates/nitro-xml-xlsx/src/workbook.rs` | `src/core/xlsx/read/load.ts`, `workbook-part.ts`, `formula-text.ts` (stripFuturePrefixes), `workbook.ts` (defaults) @ e66aaec | Stage-3 load orchestration, Print_Area lift, sheet stubs, warnings. |
