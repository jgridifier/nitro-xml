<!-- SPDX-License-Identifier: MIT -->
# Changelog

## Unreleased

- **docx repaint after edit**: typed text now appears in the rendered page of a
  multi-page Word document immediately after typing (#593). The root cause was
  `refresh_view` opening a fresh view on the edited bytes and then
  `flush_pending_body` re-loading from the original (pre-edit) handle bytes,
  wiping the edit from the view session. Fix: eagerly complete the deferred body
  parse inside `refresh_view` using the edited bytes so the deferred load never
  fires with the wrong bytes. Covered by a new Playwright e2e check that
  asserts the typed sentinel is visible in a rendered span.

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
- ~~In multi-page Word documents, typed text is saved but does not repaint~~ — fixed (#593).

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
