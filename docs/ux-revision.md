<!-- SPDX-License-Identifier: MIT -->
# UX revision: Office-style demos

Status: ribbon PR shipped (#295); editing pass below (`feat/demo-editing`). Scope: the live demos in `demo/` and the
host-side frame in `packages/nitro-xml/loader/lazy/ribbon/`. No parser, renderer, xlsx-crate, wasm or eager-loader change.

## Why
The first demos wrapped a plain `<nitro-*>` element in a navy header and three rows of default
browser buttons. Nothing said "Office", the empty and error states did not exist, and the PowerPoint
sample (zero-size shapes) rendered one letter per line. The original ooxml demos had the right
instincts (drop zone, tile icons, grouped ribbon, slide pane, status bar, dark theme); this keeps
those and rebuilds them on the real `shellAction` commands.

## Principles
1. **Real commands only.** Every ribbon, menu and status-bar control calls the element's existing
   `shellAction(op, arg)` (`copy`, `find-*`, `zoom-*`, `nav-*`, `sheet`, `diagnostics-toggle`) or a
   demo-host action (open file, open sample, close, mode, theme). State (enabled, pressed, zoom
   readout, current page) is mirrored from the Rust chrome, the same way embedded mode does it.
2. **A tab exists only if it has commands.** File, Home and View ship. Insert, Design/Layout and
   Review are *not* shown: the engine exposes no insert, layout or review command, and an empty or
   disabled tab would be a fake. They return when the commands do (one manifest entry each).
3. **Per-app accent, one system.** PowerPoint `#C43E1C`, Word `#185ABD`, Excel `#107C41` on the title
   bar, File tab, active-tab underline, selection borders and the sheet-tab underline; neutral
   grays everywhere else; Segoe UI first in the font stack, system fonts after it.
4. **The page is the product, chrome stays quiet.** Gray canvas behind Word pages (as Word does),
   white behind slides and grids (the renderer paints slides from the top-left, so gray would only
   expose the offset). Chrome heights: title bar 40, tabs 34, ribbon about 92, status bar 26.
5. **Every state is designed.** Empty (drop zone), loading (spinner, same card), error (inline,
   message kept, recovery buttons), drag-over (accent border and "Drop to open"), read-only badge.
6. **Two densities, one vocabulary.** *Full* is the frame described here. *Embedded* is the
   single-row simplified ribbon: icon-only buttons with tooltips, thin dividers, a More menu; the
   same icons and accent. Group captions only appear inside the More menu.

## What is where
| Surface | Source | Commands behind it |
| --- | --- | --- |
| Title bar: tile, open, file name and badge, search, mode toggle, theme | `lazy/ribbon/index.js` | open and mode are demo-host; search drives `find-open`, `find-query`, `find-next/prev`, `find-close` |
| File menu | same | Open…, Open sample, Close (all demo-host) |
| Home: Copy, Slides/Pages/Sheets (Previous, Next), Find | same | `copy`, `nav-prev/next`, `find-open/close` |
| View: Zoom out, in, 100%, Diagnostics | same | `zoom-out/in`, `zoom-set`, `diagnostics-toggle` |
| Slide pane (pptx), sheet tabs (xlsx), status bar, zoom slider | same | `sheet`, `zoom-set` |
| Embedded toolbar | `lazy/embedded/` (CSS and icons only) | unchanged manifest |
| Landing page | `demo/index.html` | links; theme toggle; build stamp from `build.json` |

## Dropped or not built (no backing command)
Cut, Paste, Format Painter, Paragraph and Styles groups, New Slide, Layout, Design, Review,
Comments, Share, AutoSave, word count, page-view/outline/reading view buttons, dialog launchers,
Zoom-to-fit. (Font, Undo/Redo and Save are now built for xlsx; see "Editing pass". Real slide
thumbnails are now built; see below.) The default-mode xlsx ribbon keeps rendering from the Rust chrome.

## Editing pass (demos are no longer read-only for xlsx)
**What is editable.** Only xlsx. The WASM `Handle` wires the editor for workbooks only: `saveDocument`,
`applyEdit` and `shellAction("cmd", id)` all go through the xlsx editor session. For pptx and docx
those calls fail with `no_document: not opened` and, with `readonly="false"`, the element would even
show the Excel ribbon. The `nitro-xml-pptx::editor` / `nitro-xml-docx::editor` crates exist but nothing
connects them to the handle (`docs/embedded-mode.md` already says so), so those four demos stay
read-only, show a "Read-only" pill whose tooltip says why, and carry a one-line "View only" note in
the ribbon instead of dead buttons. Principle 1 (real commands only) is the reason, not a cut corner.

**Where the buttons come from.** `lazy/ribbon/commands.js` is the manifest. Every xlsx `cmd` id in it was
run against the engine and changes the saved bytes; `loader/test/ribbon-edit.test.mjs` fails if an id
stops being one the xlsx editor maps (`nitro-xml-xlsx/src/editor/commands.rs`) or if a read-only format
lists an edit command. Enabled state still mirrors the Rust ribbon control when there is one.

| Surface (xlsx, Full) | Controls (all `shellAction("cmd", id)`) |
| --- | --- |
| Title bar quick access | Save (`file.save`), Undo (`edit.undo`), Redo (`edit.redo`); dot on Save while unsaved |
| Home, Clipboard | Copy (`copy`). Cut and Paste are unmapped in the engine, so absent |
| Home, Font | Bold, Italic, Underline, Strikethrough, Larger, Smaller |
| Home, Alignment | Top, Middle, Bottom, Wrap; Left, Center, Right |
| Home, Number | Number format menu (General, Number, Currency, Accounting, Percentage, Comma, Scientific, Date, Time), plus Accounting, Percent, Comma buttons |
| Home, Sheets | Insert sheet, Delete sheet (disabled on a single-sheet workbook) |
| Home, Editing | Clear contents, Find |
| File menu | Save a copy (download), Open, Open sample, Close |
| Typing | Select a cell and type (or F2), edit in the formula bar, Enter/Esc: the engine's own key path (unchanged) |

Embedded keeps the single-row bar (Save, Undo, Redo, Copy, Font, Alignment, Percent/Comma/Accounting, sheets, Clear, Find,
zoom) plus the formula bar; its extra "Save" button inside the formula bar is hidden because the bar has Save.

**Number format as a menu.** One native-feeling dropdown (`role=menu`, arrows, Home/End, Esc returns focus, Enter
applies) instead of nine buttons. It shows its own name, not the current format: the Rust chrome has no live
cell-style state, and a label that lies is worse than a generic one (same reason toggles carry no pressed state).

**Unsaved changes.** The engine has no modified flag, so `lazy/ribbon/dirty.js` serialises the workbook after
edits (debounced) and compares it with a baseline: the bytes at open, then at each save. Unedited documents
serialise to their original bytes, so the compare is exact. States in the title bar: "No changes", dirty dot +
"Unsaved changes", "Saved". The probe asks the worker for a save directly, so it never touches the crash-recovery
journal's last-good-save blob. `beforeunload` warns while dirty; opening another file or closing asks first.
Switching Full/Embedded carries edits across (snapshot, reopen, stays dirty); undo history does not survive it.
Save downloads the edited file under its original name (Ctrl+S and the buttons use the element's existing save path).

**Crash recovery is unchanged.** The journal (`lazy/journal.js`) records `applyEdit` specs only, as before; ribbon
commands go through `shellAction("cmd")`, exactly like the Rust ribbon buttons they replace, so none of that
behaviour moved. (Consequence worth knowing: UI edits are not in the journal; after a worker crash the last save
is what can be offered. Journaling `cmd` is an engine-side decision because `applyEdit` replays are selection
dependent.)

**Real slide thumbnails (pptx).** `lazy/ribbon/thumbs.js` opens the same bytes in a second, hidden `<nitro-pptx>`,
steps it through the slides with the existing `sheet` action and clones each slide's DOM (the renderer lays a slide
out as a 1280x720 box scaled by CSS) into a small shadow root, re-scaled. One slide at a time, only for thumbnails
scrolled into view (IntersectionObserver), so a large deck pays nothing until the pane is read. No renderer change.
The slide pane can be hidden from View, Show, Slide pane. Thumbnails would refresh after edits, but pptx has no
edit path yet, so nothing triggers a refresh today.

## Skipped for lack of a real command or writer
- **pptx and docx editing entirely** (new/duplicate/delete slide, text box, shape, table, image, Bold/Italic, font
  size and colour, alignment, paste): the editor crates define some commands (`insert.shape`, `home.duplicate`,
  `home.bold`, ...) but the handle does not expose them, there is no writer path in WASM (`saveDocument` is xlsx
  only), and `insert.text-box`, `insert.table`, `insert.slide`, `home.delete-slide` return `unsupported` in the
  crate itself. Next step is an engine task, not a demo one.
- **xlsx:** insert/delete rows and columns (`cells.insert-rows` etc. are Unmapped; the edit layer has row/column ops but
  the session does not register them, and the xlsx crate and `wasm/src/xlsx.rs` are out of scope here), font colour,
  fill colour, borders, merge, font family and size pickers, indent, decimals, Format Painter, Cut, Paste, Fill, Sort and
  Filter, AutoSum, Clear formats (all Unmapped: each leaves the saved file byte-identical when dispatched).

## Samples
The demos open `demo/samples/notes.docx` and `deck.pptx`, generated by `make_samples.py` (original
content, MIT), so they start on something worth looking at. The old `sample.docx` / `sample.pptx` stay
(they are pinned in `fixtures/manifest.json`); `sample.xlsx` is unchanged.

## Accessibility
Title bar is a banner with the document name as the page `h1`; ribbon tabs are a `tablist` with
arrow-key movement; the ribbon is a roving-tabindex toolbar (Left, Right, Home, End); File menu is
a `menu` (Up, Down, Escape); toggles carry `aria-pressed`; the zoom slider is a native range; status
readouts are live regions; axe (including color-contrast, light theme) on the full-mode pages reports nothing from
the new frame (remaining findings come from the grid and page surfaces, unchanged).

## Screenshots (editing pass)
Before (read-only, #295): `docs/img/ribbon/after-*-{full,embedded}.jpg`. After (xlsx with real edits applied, unsaved
indicator on; pptx with real thumbnails; docx view-only): `docs/img/edit/{pptx,docx,xlsx}-{full,embedded}.png`.
