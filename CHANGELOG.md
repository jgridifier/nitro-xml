<!-- SPDX-License-Identifier: MIT -->
# Changelog

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
