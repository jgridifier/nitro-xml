<!-- SPDX-License-Identifier: MIT -->
# Changelog

## Unreleased

### Host API additions

- **Real text-range selection in Word body and PowerPoint text boxes** (#561).
  `getSelection()` now returns `collapsed: false` with `text` and `start`/`end`
  offsets for drag, Shift+Arrow, double-click (word), and triple-click (paragraph)
  selections in `<nitro-docx>`. In `<nitro-pptx>`, dragging or using Shift+Arrow
  inside a text box fills `textRange: { text, start, end }` and sets `collapsed: false`.
  Typing or paste replaces the selection; Backspace/Delete removes it; Undo restores.
  Visible highlight overlay uses `Range.getClientRects()` with no simulated selection.
  The caret-only / clicked-shape-only restriction from v0.2.0 is lifted.

## 0.2.0 — 2026-11-20

- `getSelection()` payloads (Part of #561). XLSX returns the stored values of
  the selected range (numbers stay numbers, capped at 10,000 cells with
  `truncated`). PPTX returns the clicked shape's id, name and text. Reading the
  selection goes straight to the worker, so it never repaints or marks the
  document dirty. The host-API browser harness now runs
  `position_reload_readonly.py` on CI's bundled Chromium, and checks that a live
  readonly round trip keeps unsaved edits in the saved package (#565).
- Host API browser checks now require real focus, committed edits and exact
  selection payloads; missing events or selection data fail the run (#561, #562).
  Fixed the demo host's zero-height editing surface and malformed chat heading.
  At that point text ranges, XLSX values and PPTX shape fields were incomplete;
  XLSX values and PPTX shape fields are now filled (above), and text ranges
  remain open in #561. No runtime API change.

### Known limits (v0.2.0)

- In Word and PowerPoint, `getSelection()` returns the caret or the clicked shape
  only, with `collapsed: true` and no selected text range. Text-range selection
  is tracked in #561.
- In multi-page Word documents, typed text is saved but does not repaint (#593).
- The host-API browser checks (`scripts/ci/host_api_e2e.sh`) do not run in CI yet;
  the `ci.yml` job is pending. For this release they were run once on the
  tagged commit and passed (see #575).

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
- Add host position capture/navigation and open-at-position, in-place reload, and
  live readonly toggling with `nitro-mode` on DOCX/XLSX/PPTX elements.

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
