<!-- SPDX-License-Identifier: MIT -->
# Changelog

## Unreleased

### Host API additions

- **`getSelection(): Promise<NitroSelection | null>`** on `<nitro-docx>`,
  `<nitro-xlsx>` and `<nitro-pptx>` (#561). Returns format-typed snapshot of the
  current selection (docx paragraph/run/offset, xlsx sheet+range+anchor+values,
  pptx slide/shape). Works in both read-only and edit mode.
- **`dirty: boolean`** getter and **`nitro-change`** event `{ detail: { dirty } }`
  (#562, Part of #548). Fires only when `dirty` flips — once on first user edit
  (`true`) and once after `save()` resets it (`false`). Never fires during load,
  scroll, zoom, selection, or navigation.
- **`save()` no-op guarantee** (#571): `save()` on an unedited document returns
  byte-identical content and fires no `nitro-change` or other host events.
- Types exported as `NitroSelection`, `NitroDocxSelection`, `NitroXlsxSelection`,
  `NitroPptxSelection` in `loader/index.d.ts`.
- Docs: `loader/README.md` and `packages/nitro-xml/README.md` both have a "Host
  API" section with signatures, event tables, and copy-paste examples.

## 0.1.0 — release candidate

- Browser viewing/editing of DOCX, XLSX and PPTX via Rust/WASM and the SSR-safe
  React `NitroViewer`; host-provided persistence via `save(): Promise<Uint8Array>`.
- Cell/format/sheet, paragraph/run/table and shape/slide edits, undo/redo,
  package size errors, opaque macro preservation and worker crash recovery.
- Bundler-recognizable worker URLs fix production Vite worker crashes.
- Published-file allowlists and tarball-only production Vite consumer checks;
  OOXML/LibreOffice validation and an Office hand-check pack generator.
- Known losses: accepted XLSX S / DOCX S–M first-open memory losses; separate
  DOCX L peak-memory regression (1.151×), still unattributed. UX and persisted
  edit coverage limits remain documented.

See [full release notes](docs/releases/0.1.0.md), including the actual React API,
limits, corpus evidence and [M1 measurements](docs/M1-REPORT.md). Publication
awaits Head of Product PASS; this entry does not claim that approval.
