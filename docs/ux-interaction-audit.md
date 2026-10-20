<!-- SPDX-License-Identifier: MIT -->
# UX interaction audit (slice 1: findings only, no fixes)

Goal: make caret, selection, drag, keyboard and touch behave like PowerPoint / Word / Excel, for a React-embedded
real-time view+edit surface where input latency matters. This document is the audit that the fix PRs work from.

- Harness: `tests/interaction/audit.mjs` (+ `lib.mjs`, `scenarios.mjs`, `server.mjs`). Real mouse, keyboard, touch (`hasTouch`) and CDP IME events in headless Playwright.
- Targets: the live demos (`https://jgridifier.github.io/nitro-xml/demo/*-full.html`) and a local build (`demo/` + `packages/nitro-xml/{loader,pkg}` served as `pages.sh` lays them out). Browsers: Chromium and WebKit.
- Commit audited: `0648541` (main) + this branch. Run: `bash scripts/build-wasm.sh && node tests/interaction/audit.mjs` (flags: `--targets local,live --browsers chromium,webkit --formats docx,xlsx,pptx --only <substr> --skip-latency --concurrency N`). Output: `tests/interaction/out/results.json`, failure screenshots in `tests/interaction/out/shots/` (gitignored). The audit always exits 0; it is not a gate.
- Before-screenshots of the worst issues: `/workspace/ux-shots/before-*.png` (named in each finding).
- **Live and local agree** on every scenario except timing-sensitive ones (noted). So everything below is in the shipped demo, not a local artifact.
- Result tally (65 scenarios per browser/target): local chromium 21 pass / 44 fail; local webkit 22 / 38 (+5 n/a: CDP-only); live chromium 22 / 43; live webkit 22 / 38 (+5 n/a). Full matrix at the end.

Severity: **S1** data lands in the wrong place / is lost, page goes blank, or the core edit loop is broken. **S2** a standard Office interaction is missing or wrong. **S3** noticeable deviation. **S4** polish.

## Architecture facts that explain most findings

1. The loader forwards **pointer, wheel and a few keys** to Rust as batched events (`loader/element.js` `_bind`, `p()` and `keyEv`), and **every key** additionally goes `keydown -> worker editorKey(e.key, mods) -> Rust` (`element.js` `_bx` `onkeydown`, `worker-host.js` `editorKey`). There is no `beforeinput`, `compositionstart/update/end`, `paste`, `copy`, `cut`, `dblclick` or `touch*` handling anywhere in the loader, and no hidden text-input element. Focus sits on `div.v[tabindex=0]`.
2. Rust consumes pointer events only for **xlsx** (`rs/crates/nitro-xml-wasm/src/xlsx.rs` `on_pointer`: left-button down/move/up on cells only) and **pptx** (`pptx.rs` `apply_event`: left-button *down* only = pick shape). **docx consumes no pointer events at all** (`docx.rs` `process_events` handles only PageUp/PageDown/ArrowLeft/ArrowRight as page navigation).
3. docx editing is `setParagraphText` of the whole paragraph from a byte-offset caret held in `edit.print.caret`, which nothing but the edit itself ever moves (`docx_edit.rs` ~L100-150, `office_edit.rs` `edit_plain`), followed by `refresh_view` which **saves and re-opens the whole document** after each edit.
4. Every handled `editorKey` response embeds the full chrome HTML (`h`: ribbon + formula bar), and the loader does `this._ch.innerHTML = edit.h` (`element.js` `paintEdit`). Measured payload per keystroke: **xlsx 54.5 KB, docx 6.1 KB, pptx 5.8 KB**.

## Findings (ranked)

### S1

**S1-1. DOCX: clicking never places a caret, and typed text always goes to the start of the first paragraph.**
Repro: open `docx-full.html`, click in the middle of "WebAssembly engine entirely in your browser", type `ZQ`. Saved model paragraph 0 becomes `ZQQuarterly field notes` (title); the clicked paragraph is untouched. No caret is drawn at any time (pixel diff over four blink phases: 0 changed pixels).
Browsers: chromium + webkit. Live and local. Scenarios: `docx.caret-visible`, `docx.type`. Shot: `before-docx-no-caret.png`.
Suspect: `docx.rs:89 process_events` (no pointer branch), `docx_edit.rs` caret defaults to block 0 / offset 0, no caret overlay in the print render (`dve-print-*` DOM has no caret/selection layer).

**S1-2. DOCX: the first keystroke blanks the page.** Within ~300 ms of the first edited key `.dom` goes from 2 children (26 lines) to 0 and the page area is empty grey; the typed text *is* in the saved model (`saved model has XYZ=true`) but nothing is on screen until reload. Subsequent keys keep it blank.
Browsers: chromium + webkit, live + local. Scenarios: `docx.type`, `docx.delete-*`, `docx.undo-redo-type`. Shot: `before-docx-type-blanks-page.png`.
Suspect: `refresh_view` re-opens the document from saved bytes each edit (layout lost until a measure/layout round-trip), and `loader/apply.js:20` clears the root (`root.replaceChildren()`) before applying every frame, so an empty/partial frame wipes the DOM. Also the source of docx typing latency (key to DOM p95 62-173 ms, table below).

**S1-3. DOCX: Backspace, arrow keys and paragraph/run joins do not work.** `editorKey('Backspace')` returns `handled:false` at the default caret (offset 0) and arrows are never handled, so there is no caret movement; Backspace/Delete across a paragraph or run boundary is not implemented (`edit_plain` edits one paragraph string). Model after 3x Backspace mid-paragraph: unchanged. Delete-forward works but leaves the page blank (S1-2). From code reading (not run-verified): `setParagraphText` re-applies the first run's attributes to the whole paragraph, so typing into a mixed-format paragraph would flatten run formatting.
Scenarios: `docx.backspace-in-run`, `-across-paragraph`, `-across-runs`, `docx.delete-across-paragraph`, `docx.delete-forward`.

**S1-4. DOCX: Ctrl+Z does not undo a typed edit** (typed=true, undone=false, redone=true when checked against the saved model). Scenario `docx.undo-redo-type`. (xlsx and pptx undo/redo round-trip correctly.)

**S1-5. No real text-input target in any format, so IME, mobile keyboards, dictation, dead keys/AltGr and paste cannot work.** `document.activeElement` after a click is `div.v`, not an `input`/`textarea`/`contenteditable`. CDP `Input.imeSetComposition` + `insertText` into docx and xlsx inserts nothing (chromium; webkit has no CDP, so n/a, but the missing handlers are browser-independent). On touch devices no virtual keyboard can be summoned.
Scenarios: `docx.text-input-target` (x3 formats), `docx.ime-compose`, `xlsx.ime-compose`. Shot: `before-docx-ime-dropped.png`.
Suspect: `element.js` `_bx` is keydown-only; no `beforeinput`/composition listeners; `KeyId`/`EventKind.Composition` exist in `events.js` but nothing produces them.

### S2

**S2-1. DOCX has no selection at all.** Drag, shift+click, shift+arrows, double-click word and triple-click paragraph all paint nothing (pixel diff 0.000); copy writes an empty string to the clipboard, cut does nothing, paste is received (`paste` event fires) but ignored. Scenarios `docx.select-*`, `docx.copy`, `docx.cut`, `docx.paste`. Shots: `before-docx-drag-no-highlight.png`. Same root cause as S1-1 (no pointer handling, no selection model or highlight layer in the docx render).

**S2-2. PPTX direct manipulation is missing.** Click picks a shape and draws a 1 px dark-blue outline (nearly invisible on the dark title slide), but there are **no resize/rotate handles**, **drag does not move** (shape stays at the same x,y during and after the drag, document stays clean), corner drag does not resize, the cursor never changes, no shift/ctrl multi-select, and arrow keys change slide instead of nudging the selected shape. Double-click then typing appends text at the *end* of the shape's text regardless of where you clicked (`Nitro XML` + `XYZ`), with no caret drawn.
Scenarios `pptx.select-shape`, `pptx.drag-move`, `pptx.resize-handles`, `pptx.text-edit` (passes only in the weak sense that text lands). Shots: `before-pptx-select-no-handles.png`, `before-pptx-drag-no-move.png`.
Suspect: `pptx.rs:192` handles `Pointer` type 0 only; no move/up/drag state; `pptx_edit.rs` has the shape-edit commands but nothing drives them from pointer input.

**S2-3. PPTX slide thumbnails cannot be drag-reordered.** Drag thumbnail 1 below thumbnail 3: order unchanged, no `draggable`. Click-to-navigate works. Scenario `pptx.thumb-drag-reorder`. Shot: `before-pptx-thumb-reorder-noop.png`. Suspect: `loader/lazy/ribbon/index.js` thumbnail pane (~L410-430, `thumbs.js`); needs a reorder edit op in the pptx editor.

**S2-4. XLSX: double-click replaces the cell instead of editing it.** Double-click `A2` ("row-1"), type `Z`, Enter: the cell becomes `Z` (Excel: `row-1Z`, caret at end). There is no click-count in the pointer event, so the second click is just another select. `F2` + Escape works. Scenario `xlsx.dblclick-edit`. Shot: `before-xlsx-dblclick-replaces-text.png`. Suspect: `events.rs` Pointer encoding has no click count; `xlsx.rs on_pointer`.

**S2-5. XLSX: Tab / Shift+Tab do not move the active cell, and Tab is not `preventDefault`ed.** Chromium: selection stays put. **WebKit: focus leaves the grid on Tab and every later key (Enter, arrows) is dead** (`B3 B3 B3 ...` for the rest of the sequence). Ctrl+Home does nothing. Tab-across-row then Enter does not return to the starting column (Excel: G21; we get H20). Enter and Shift+Enter work. Scenarios `xlsx.enter-tab-nav`, `xlsx.tab-enter-return-column`. Shot: `before-xlsx-webkit-tab-focus-escape.png`. Suspect: `element.js` `keyEv` only prevents arrows/select-all; `xlsx_edit.rs nudge_for` has Tab but is only reached while a cell editor is open (`editor_key_idle`).

**S2-6. XLSX: headers and fill handle.** Clicking or dragging column/row headers does not select columns/rows (`on_pointer` ignores `HitArea` != `Cell`); dragging a header edge does not resize and the cursor stays `auto`; the selection has no fill handle (`.xg-handle` never exists) and so no fill-drag; no copy marching-ants. Scenarios `xlsx.header-select`, `xlsx.column-resize`, `xlsx.selection-border-and-fill-handle`, `xlsx.fill-handle-drag`.

**S2-7. Clipboard.** xlsx: Ctrl+C writes the cell text to the system clipboard (works on both engines), but **Ctrl+V does nothing**, so copy-paste and cut-paste both fail at the paste step (`xlsx.copy-paste`, `xlsx.cut-paste`). docx: see S2-1. The copy path is async (`worker -> shellAction -> navigator.clipboard.writeText` in `element.js shellAction`), i.e. outside the user gesture; it succeeded in headless WebKit here, but a write that happens after an async worker round-trip depends on the browser still honouring the user gesture (not verified on real Safari); writing synchronously from the `copy` event with `clipboardData.setData` avoids that.

**S2-8. Touch.** `.v` has `touch-action: auto` and `user-select: auto` on all three formats (`touch.touch-action`). A one-finger drag on xlsx fires `pointerdown`, 2x `pointermove`, then `pointercancel` and neither scrolls nor selects (`touch.drag-xlsx`); long-press does not select a word in docx (`touch.long-press-word-docx`); tap does not place a caret in docx (`touch.tap-docx-caret`); double-tap does not edit an xlsx cell (`touch.dblclick-edit-xlsx`). Tap-select in xlsx and tap-select in pptx work; one-finger scroll in docx works only because `.dom` is a native scroller. Shot: `before-touch-long-press-docx.png`. Suspect: `element.js _css()` (no `touch-action`), `p()` ignores `pointercancel` and has no `pointerType` handling.

**S2-9. XLSX layout shifts under the pointer.** The first interaction mounts the formula bar and the grid moves down **29 px** (after the click or the first keystroke depending on timing). A drag or click already in flight lands one row off. Scenario `xlsx.layout-shift-on-first-key`. Shot: `before-xlsx-layout-shift.png`. Cause: chrome HTML arrives in the editorKey/ui response and is swapped into `.chrome` after first paint (`element.js` `paintEdit`, `_ao`); the formula bar height is not reserved.

### S3

**S3-1. A key sent right after a click acts on the old selection (chromium).** Click F9, press ArrowDown immediately: selection stays F9 (expected F10). Pointer events wait for the next rAF tick (`worker-host.js` `_events.flushNow()` is called from `tick`), while `editorKey` goes straight to the worker, so the two streams are reordered. A human needs >100 ms between click and key, but IME/automation/type-ahead do not. Also seen: back-to-back `Shift+Arrow` presses with 0 ms gaps drop steps. Scenario `xlsx.click-then-immediate-key-order`. (Keystroke rates of 10-30 ms/char and 30 ms arrow repeat are fine: `*.rapid-typing`, `xlsx.rapid-arrow-repeat` pass everywhere.)

**S3-2. Per-keystroke chrome re-render.** Every handled key ships the whole ribbon/formula-bar HTML (xlsx 54.5 KB) and replaces `.chrome.innerHTML`, which re-parses the ribbon, resets hover/tooltip/focus state inside it and causes the S2-9 shift. Latency effect is in the table; the fix is to send chrome only when it changed (selection label, undo state, dirty flag) and patch.

**S3-3. Intermittent stale DOM while typing fast in pptx** (1 of 4 runs: model has all 20 characters, `.dom` does not after 2 s; local chromium only). Scenario `pptx.rapid-typing`. Probably the frame/measure race in `worker-host-tick.js tickBody` under load; needs a stress repro.

**S3-4. Selection feedback is weak on dark fills.** The pptx selected-shape outline is 1 px and nearly invisible on the dark title slide (see `before-pptx-select-no-handles.png`).

## What already works (keep when refactoring)

xlsx: click, drag range, shift+click, shift+arrows (with ≥80 ms gaps), select-all (Ctrl+A gives `A1:T50`), wheel scroll, typing into a cell with Enter commit, F2/Escape, Delete, undo/redo; 10 ms/char typing and 30 ms arrow repeat lose nothing. pptx: thumbnail click navigation, PageUp/PageDown, undo/redo of a text edit, fit-to-viewport; text edit lands in the model. Live and local builds match.

## Input-to-screen latency

Measured by an in-page probe: `keydown`/`pointerdown` `event.timeStamp` to the first DOM mutation under the viewer (`dom`), and to the second `requestAnimationFrame` after it (`frame`, the paint estimate). Headless Linux, 1280x800, DPR 1, no CPU throttling. xlsx = type into cell / ArrowUp-Down / click; pptx = typing in a shape / click; docx = first keystroke on 5 fresh pages (the page stops painting after it, see S1-2) and ArrowRight.

| Target | Browser | Format | Input | n | key→DOM p50 / p95 (ms) | key→next frame p50 / p95 (ms) |
|---|---|---|---|---|---|---|
| local | chromium | xlsx | type | 15 | 21.6 / 27.1 | 40.7 / 46.1 |
| local | chromium | xlsx | nav | 15 | 24.4 / 102.9 | 46.3 / 124.9 |
| local | chromium | xlsx | click | 14 | 13.5 / 21.3 | 32.3 / 45.2 |
| local | chromium | pptx | type | 15 | 12.1 / 21.3 | 38.2 / 50.9 |
| local | chromium | pptx | click | 14 | 3.5 / 16.5 | 28.9 / 46.8 |
| local | chromium | docx | type | 10 | 12.4 / 62.2 | 26.5 / 89.1 |
| local | chromium | docx | nav | 10 | 12.2 / 13.3 | 27.3 / 39.6 |
| local | webkit | xlsx | type | 15 | 38 / 45 | 72 / 122 |
| local | webkit | xlsx | nav | 15 | 33 / 43 | 70 / 88 |
| local | webkit | xlsx | click | 14 | 32 / 58 | 69 / 108 |
| local | webkit | pptx | type | 15 | 22 / 35 | 49 / 70 |
| local | webkit | pptx | click | 14 | 17 / 21 | 43 / 143 |
| local | webkit | docx | type | 10 | 19 / 63 | 46 / 91 |
| local | webkit | docx | nav | 10 | 17 / 20 | 44 / 49 |
| live | chromium | xlsx | type | 15 | 22.9 / 60.3 | 41.7 / 82.9 |
| live | chromium | xlsx | nav | 15 | 18.9 / 28.3 | 40.7 / 51.9 |
| live | chromium | xlsx | click | 14 | 14.7 / 21.2 | 35 / 44.6 |
| live | chromium | pptx | type | 15 | 13.2 / 27 | 37.3 / 53.4 |
| live | chromium | pptx | click | 14 | 3.3 / 17.4 | 28.7 / 48.2 |
| live | chromium | docx | type | 10 | 11.6 / 66.3 | 26.8 / 89.9 |
| live | chromium | docx | nav | 10 | 11 / 14.1 | 27.1 / 39.7 |
| live | webkit | xlsx | type | 15 | 43 / 57 | 83 / 118 |
| live | webkit | xlsx | nav | 15 | 35 / 46 | 73 / 107 |
| live | webkit | xlsx | click | 0 | null / null | null / null |
| live | webkit | pptx | type | 15 | 23 / 32 | 50 / 67 |
| live | webkit | pptx | click | 14 | 16 / 22 | 41 / 48 |
| live | webkit | docx | type | 10 | 28 / 83 | 63 / 105 |
| live | webkit | docx | nav | 10 | 17 / 28 | 45 / 61 |

Reading it: Chromium key-to-frame p95 is ~40-60 ms for xlsx type/click and pptx. `docx type` is the outlier (frame p95 ~89-102 ms, dom p95 62-173 ms across runs; consistent with a whole-document save + reopen per key, S1-2). `xlsx nav` showed a dom p95 of 103-139 ms on two local chromium runs but 30 ms live (n=15): treat as noise until reproduced. WebKit is ~1.5-2x slower on xlsx (frame p95 98-129 ms for type and click). The 54 KB chrome swap per key (S3-2) is a plausible contributor to xlsx but is not isolated by this audit. These are lab numbers; a React host with its own work per keystroke and low-end CPUs will be worse, so S3-2 and S1-2 matter for the latency goal.

## Proposed fix themes (each a small PR, in suggested order)

1. **docx caret: pointer hit-test to caret + caret overlay** (Rust `docx.rs` pointer branch using the print layout, caret rect in the command buffer, caret blink). Unblocks S1-1.
2. **docx edit render path: no blank page, no per-key reopen** (keep last good frame when a frame is empty: `apply.js` guard; incremental relayout instead of `refresh_view`; preserve run attrs). S1-2, S1-3.
3. **Text-input proxy in `loader/lazy/`**: a visually hidden `textarea`/`contenteditable` focus target created on first focus (lazy import, no eager-budget cost) that turns `beforeinput`, composition, `paste`, `copy`, `cut` into the existing editor ops. S1-5, S2-7, mobile keyboard, dead keys.
4. **docx selection + clipboard**: selection model, highlight rects, shift+click/arrows, word/paragraph click counts (add `clickCount` to the Pointer event: `events.rs` + `events.js`), Ctrl+C/X/V through the proxy. S2-1, S1-4 (undo of typed text).
5. **xlsx grid interactions**: Tab/Shift+Tab (+`preventDefault`), Ctrl+Home/End, dblclick edit via click count, header select/resize, fill handle, paste. S2-4/5/6/7.
6. **pptx direct manipulation**: handles, drag-move, resize, nudge with arrows, multi-select, high-contrast selection, thumbnail reorder. S2-2, S2-3.
7. **touch pass**: `touch-action`, `pointercancel` handling, long-press select with handles, double-tap edit, xlsx touch scroll/range. S2-8.
8. **key path hygiene**: ordered pointer/key stream (flush pending pointer events before `editorKey`), send chrome HTML only when changed, reserve the formula-bar height. S2-9, S3-1, S3-2, plus the latency numbers above.

Each PR should add rows to `tests/interaction/scenarios.mjs` (the scenarios here become the acceptance checks) and, per AGENTS.md rule 0, a corpus file or differential check where one can isolate the bug.

## Harness notes and limits

- Cell/line geometry is read from the rendered DOM (`.xg-hdr-*`, `.dve-print-line`, `.pptxv-text`); selection comes from `shellAction('ui')`; the docx/pptx *model* is checked by saving through the element and reading the zip (`lib.mjs`). Caret/highlight presence is a pixel diff of a clip around the target, sampled across a blink period.
- A 150 ms "human" delay follows every mouse click/tap (the raw call is used only by the ordering scenario); clipboard scenarios run serially because the system clipboard is shared per browser.
- WebKit cannot do CDP IME / touch dispatch under Playwright: those rows are n/a, not passes. `tests/mobile/MANUAL.md` still applies for real devices.
- Pointer-event capture via real `Input.dispatchTouchEvent` (chromium); WebKit touch is limited to `touchscreen.tap`.
- Not covered yet: right-click context menus, drag-and-drop of files, pinch zoom, high-DPR, RTL caret movement, table-cell selection in docx, charts/SmartArt hit-testing in pptx, find-bar focus interplay.

## Full matrix

✅ pass, ❌ fail, — not testable in that engine, ⚠️ harness error.

| Scenario | Default sev | local chromium | local webkit | live chromium | live webkit |
|---|---|---|---|---|---|
| `docx.caret-visible` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.text-input-target` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.text-input-target [pptx]` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.text-input-target [xlsx]` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.type` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.rapid-typing` | S1 | ✅ | ✅ | ✅ | ✅ |
| `docx.backspace-in-run` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.backspace-across-paragraph` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.delete-across-paragraph` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.delete-forward` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.backspace-across-runs` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.undo-redo-type` | S1 | ❌ | ❌ | ❌ | ❌ |
| `docx.select-drag` | S2 | ❌ | ❌ | ❌ | ❌ |
| `docx.select-shift-click` | S2 | ❌ | ❌ | ❌ | ❌ |
| `docx.select-shift-arrows` | S2 | ❌ | ❌ | ❌ | ❌ |
| `docx.select-dblclick-word` | S2 | ❌ | ❌ | ❌ | ❌ |
| `docx.select-triple-paragraph` | S2 | ❌ | ❌ | ❌ | ❌ |
| `docx.ime-compose` | S1 | ❌ | — | ❌ | — |
| `xlsx.layout-shift-on-first-key` | S2 | ❌ | ❌ | ❌ | ❌ |
| `xlsx.click-cell` | S2 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.drag-range` | S2 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.shift-click-range` | S2 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.shift-arrows` | S2 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.selection-border-and-fill-handle` | S3 | ❌ | ❌ | ❌ | ❌ |
| `xlsx.fill-handle-drag` | S3 | ❌ | ❌ | ❌ | ❌ |
| `xlsx.click-then-immediate-key-order` | S3 | ❌ | ✅ | ❌ | ✅ |
| `xlsx.type-into-cell` | S1 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.rapid-arrow-repeat` | S1 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.rapid-typing` | S1 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.enter-tab-nav` | S2 | ❌ | ❌ | ❌ | ❌ |
| `xlsx.tab-enter-return-column` | S3 | ❌ | ❌ | ❌ | ❌ |
| `xlsx.dblclick-edit` | S2 | ❌ | ❌ | ❌ | ❌ |
| `xlsx.f2-and-escape` | S3 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.delete-clears` | S3 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.undo-redo` | S2 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.header-select` | S3 | ❌ | ❌ | ❌ | ❌ |
| `xlsx.column-resize` | S3 | ❌ | ❌ | ❌ | ❌ |
| `xlsx.select-all` | S4 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.wheel-scroll` | S3 | ✅ | ✅ | ✅ | ✅ |
| `xlsx.ime-compose` | S1 | ❌ | — | ❌ | — |
| `pptx.select-shape` | S1 | ❌ | ❌ | ❌ | ❌ |
| `pptx.drag-move` | S1 | ❌ | ❌ | ❌ | ❌ |
| `pptx.resize-handles` | S1 | ❌ | ❌ | ❌ | ❌ |
| `pptx.text-edit` | S1 | ✅ | ✅ | ✅ | ✅ |
| `pptx.rapid-typing` | S1 | ❌ | ✅ | ✅ | ✅ |
| `pptx.undo-redo` | S2 | ✅ | ✅ | ✅ | ✅ |
| `pptx.thumb-click-nav` | S3 | ✅ | ✅ | ✅ | ✅ |
| `pptx.thumb-drag-reorder` | S2 | ❌ | ❌ | ❌ | ❌ |
| `pptx.key-nav` | S3 | ✅ | ✅ | ✅ | ✅ |
| `pptx.slide-fits-viewport` | S3 | ✅ | ✅ | ✅ | ✅ |
| `touch.touch-action [docx]` | S1 | ❌ | ❌ | ❌ | ❌ |
| `touch.touch-action [xlsx]` | S1 | ❌ | ❌ | ❌ | ❌ |
| `touch.touch-action [pptx]` | S1 | ❌ | ❌ | ❌ | ❌ |
| `touch.tap-xlsx [xlsx]` | S2 | ✅ | ✅ | ✅ | ✅ |
| `touch.drag-xlsx [xlsx]` | S2 | ❌ | — | ❌ | — |
| `touch.tap-docx-caret [docx]` | S2 | ❌ | ❌ | ❌ | ❌ |
| `touch.long-press-word-docx [docx]` | S2 | ❌ | — | ❌ | — |
| `touch.drag-docx [docx]` | S2 | ✅ | — | ✅ | — |
| `touch.tap-pptx-select [pptx]` | S2 | ✅ | ✅ | ✅ | ✅ |
| `touch.dblclick-edit-xlsx [xlsx]` | S3 | ❌ | ❌ | ❌ | ❌ |
| `docx.copy` | S2 | ❌ | ❌ | ❌ | ❌ |
| `docx.cut` | S2 | ❌ | ❌ | ❌ | ❌ |
| `docx.paste` | S2 | ❌ | ❌ | ❌ | ❌ |
| `xlsx.copy-paste` | S2 | ❌ | ❌ | ❌ | ❌ |
| `xlsx.cut-paste` | S2 | ❌ | ❌ | ❌ | ❌ |
