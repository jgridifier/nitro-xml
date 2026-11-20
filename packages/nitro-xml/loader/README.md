<!-- SPDX-License-Identifier: MIT -->
# Loader API (#37)

The **only shipped JavaScript** in Nitro XML. It instantiates the wasm, forwards
browser events, applies each frame's command buffer, and performs batched DOM
read-backs (pre-apply text measurement and post-apply layout queries) with at
most one reply batch of each kind per frame. Nothing else ships as JS
(PRODUCT_SPEC.md; Head of Product review on #33).

Wire formats live in `docs/boundary.md`, command-buffer / measure types in
`rs/crates/nitro-xml-render/`, and the wasm handle / events / string table in
`rs/crates/nitro-xml-wasm` (`handle.rs`, `events.rs`, `transport.rs`).

## Bootstrap

```js
import { initCore, defineNitroXml, wasmSupported } from "nitro-xml/loader";

if (!wasmSupported()) {
  // Show the unsupported-browser message; no other path.
}
await initCore();          // compileStreaming / instantiateStreaming when possible
defineNitroXml();          // registers <nitro-xml>
defineNitroXlsx();         // registers <nitro-xlsx readonly> xlsx viewer (#19)

// React hosts: compile+cache wasm and park a Module-seeded worker (lazy chunk).
// preload() does not instantiate on the host (WorkerDocument owns the instance).
const { preload } = await import("nitro-xml/loader/lazy/preload.js");
await preload();
```

`defineNitroXlsx()` from the same entry registers `<nitro-xlsx readonly>` for the
Stage-3 xlsx grid viewer (worker + command buffer). Use `readonly` (default) for
view-only embeds; editing stays on `<nitro-xml>` / future editor paths.

- Prefers `WebAssembly.compileStreaming` / the wasm-bindgen streaming init path
  with a `Response` (`instantiateStreaming` when glue receives a `Response`).
- Compiled `WebAssembly.Module` is cached in memory for the page lifetime; wasm
  bytes are written to the Cache API when available (next navigation skips fetch).
- `preload()` parks a worker seeded with that Module (skips a second compile) and
  does not call host `initCore()`; session.js / explicit callers still can.
- Worker is also pre-warmed on first `WorkerDocument` open when preload was skipped.
- `loadFormatModule(ext)` / `loadEditorModule()` are hooks for a future split:
  small core + per-format modules on demand; editor module only on edit. Today
  the core package already links all format crates.

If wasm cannot run (missing `WebAssembly`, CSP without `'wasm-unsafe-eval'`,
instantiate failure) the component shows:

> Unsupported browser: Nitro XML needs WebAssembly with 'wasm-unsafe-eval' in Content-Security-Policy.

## Font source (#121)

Hosts configure where remote webfont stylesheets come from. The loader only
forwards the option; URL building, fetch-off policy and substitution diagnostics
live in Rust/WASM (`Handle.planWebfontStylesheet`).

```js
// Process default (subsequent opens inherit this):
await initCore({ fontSource: "https://fonts.googleapis.com/css2" }); // upstream default if omitted
await initCore({ fontSource: "https://fonts.internal.example/css2" }); // mirror
await initCore({ fontSource: false }); // or "off" — no remote webfont fetch

// Per document (overrides the process default):
const session = await openDocument(bytes, {
  name: "deck.pptx",
  fontSource: false,
});
// Or a resolver: (families: string[]) => href | null
await openDocument(bytes, {
  fontSource: (families) =>
    families.length ? `https://cdn.example/css2?family=${encodeURIComponent(families.join("|"))}` : null,
});
```

| Value | Behaviour |
|---|---|
| omitted / default | Same as upstream: CSS2 base `https://fonts.googleapis.com/css2` |
| string | CSS2-compatible mirror base URL |
| `false` or `"off"` | No remote fetch; local/embedded fonts only; each skipped face is recorded as a `font_substitution` warning on the handle |
| `(families) => href \| null` | Host builds the stylesheet href; engine still decides when to resolve and records substitutions when the resolver returns null. Main-thread `openDocument` only (functions are not posted into the worker) |

`WorkerDocument` accepts the serializable forms (`string` / `false` / `"off"`). With fetch off, `planWebfontStylesheet` returns no href (so the host must not inject a `<link>` or `fetch` webfonts) and pushes diagnostics naming the requested face.

## Formats

Allow-list (shared with #13): `.docx`, `.xlsx`, `.pptx`, macro-enabled
`.docm`/`.xlsm`/`.pptm`, templates `.dotx`/`.xltx`/`.potx`, macro-templates
`.dotm`/`.xltm`/`.potm`. Macros never execute.

`ACCEPT` matches the component `accept` attribute. Unsupported files (legacy
`.doc`/`.xls`/`.ppt`, encrypted packages, CSV, anything else) show a clear
unsupported-format message naming the supported Word/Excel/PowerPoint variants.

## Event forwarding

`createEventCoalescer` / `DocumentSession.pushEvent` / `WorkerDocument.pushEvent`
encode the #14 binary batch (`events.rs`) and post **at most one** batch per
animation frame into `Handle.pushEventBatch`.

## Per-frame command buffer

Rust encodes seed opcodes (`transport.rs`); the loader takes
`Handle.takeFrameCommands()` and runs `applyCommandBuffer` in a **single pass**
(canvas draw + DOM build). No per-element WASM→JS calls on the render path.

Opcode gaps vs upstream painters are listed in `docs/boundary.md`. Painter placement (worker vs main) is in `docs/adr/0002-renderer-port.md` (#36).

## Batched read-back (measurement)

Each frame, before applying draw commands:

1. Read all pending `{font, size, text}` SoA requests from the handle.
2. Run `measureText` for the whole batch on a canvas 2D context.
3. Write widths back with **one** `Handle.setMeasureResults` call.

Never one wasm call per string. Worker path: worker sends the request SoA +
resolved strings; main thread measures; one `measure-results` message returns.

## Post-apply layout read-back

After `applyCommandBuffer`, Rust may have queued **node-id-keyed** layout
queries for the DOM the loader just built. The loader snapshots them with
`Handle.takeLayoutReqs` (or the worker includes them on the `frame` message)
and answers the **entire** batch on the **next** frame with one
`Handle.setLayoutResults` call — never one WASM→JS call per element. Iterative
autofit therefore needs no per-element round-trips.

| Kind | Tag | Upstream consumer | Result lanes (`f64` × 4 per request) |
|---|---:|---|---|
| `SpanRect` | 1 | docx `canvas-measurer.ts` (hidden `<span>` / `getBoundingClientRect` for ligatures and small caps) | `x, y, width, height` |
| `ScrollHeight` | 2 | pptx `text-autofit-shrink-measure.ts`, `shape-autofit-resize.ts` (`scrollHeight` / off-screen clone height) | `scrollHeight, 0, 0, 0` |
| `ComputedStyle` | 3 | same autofit path (`getComputedStyle` → `font-size`, `line-height`; property name is a string-table id) | `parseFloat(value)` or `NaN`, then `0, 0, 0` |
| `NaturalWidth` | 4 | P5 `image-native-size.ts` (`HTMLImageElement.naturalWidth` / `naturalHeight` for `a:tile` sx/sy) | `naturalWidth, naturalHeight, 0, 0` |

Wire format and handle APIs: `docs/boundary.md`. Types: `loader/layout-readback.js`
(`LayoutReadKind`, `flushLayoutReadBatch`). Session tick order: answer prior-frame
layout → measure → apply → take this frame's layout requests.

## Worker messaging

One dedicated worker per document. Messages:

| Main → worker | Worker → main |
|---|---|
| `prewarm` | `ready` |
| `open` (+ transferred `ArrayBuffer`) | `opened` |
| `events` (+ batch bytes) | `events-acked` |
| `take-frame` | `frame` (commands + measure SoA + layout SoA) |
| `measure-results` (Float64Array) | `measure-acked` |
| `layout-results` (Float64Array, `awaiting * 4`) | `layout-acked` |
| `sa` (`op`, `arg`; `ui` = snapshot) | `sa` (`result` JSON) |
| `dispose` | `disposed` |

DOM/canvas apply, measure batches and layout read-backs run on the main thread; parse/model/layout stay in the worker; pptx text layout stays on the main thread (`docs/adr/0002-renderer-port.md`, #36).

## Viewer shell (#29)

Shell **logic** (zoom, tabs, find, copy, safe-href, diagnostics incl. #121
`font_substitution`), **chrome HTML**, and hotkeys live in Rust (`shell/chrome.rs`).
`ShellViewerElement` (shared by `<nitro-xlsx>` / `<nitro-pptx>`) only assigns
`result.h` (chromeHtml), forwards click/input/keydown from `data-a` / `data-hk`,
and opens clipboard/hyperlinks. Pptx slide arrows come from Rust `data-hk`
(`nav-next` / `nav-prev`). Docx reuses the same base when #23 lands.

```js
const result = await viewer.shellAction("find-query", "revenue");
// result.h (chromeHtml) · result.c / copyText · result.p / pendingHyperlink · result.ui
viewer.addEventListener("nitro-hyperlink", (e) => {});
viewer.addEventListener("nitro-copy", (e) => {});
```

Keys (from Rust `data-hk`): `Ctrl/Cmd+F` find, `Ctrl/Cmd+C` copy, `Ctrl/Cmd+=`/`-` zoom, `Esc` closes find.

### Embedded mode

`<nitro-pptx mode="embedded">` (also `<nitro-docx>`, `<nitro-xlsx>`; or `el.mode = "embedded"`)
swaps the chrome for one compact, PowerPoint-Home-style toolbar for host apps. Default is
`mode="full"` (unchanged). The toolbar is a lazy chunk (`loader/lazy/embedded/`, dynamic
`import()` only for this mode) that calls `shellAction` and mirrors state from the hidden Rust
chrome. Theme with the `theme` attribute / property or `--nx-*` tokens (`docs/theme.md`), or `::part(embedded-toolbar|embedded-group|embedded-button)`.
See `docs/embedded-mode.md`.

## Headless session (same thread)

```js
import { openDocument } from "nitro-xml/loader";

const session = await openDocument(bytes, { name: "sheet.xlsx", canvas, root });
session.pushEvent({ kind: 6, time_ms: 0, width: 800, height: 600, dpr: 1 });
session.tick();   // prior layout → measure → apply → take layout → flush events
session.dispose();
```

Font loading (pptx embedded / remote webfont fallback) uses the **Font source
(#121)** option above (merged in #126): upstream default, mirror URL, off, or
resolver; substitutions are recorded as `font_substitution` diagnostics.

`FinalizationRegistry` on the session calls `handle.free()` if the embedder
drops the last reference without `dispose()` (wasm-bindgen also registers the
raw `Handle`).

## Host API

Methods and events on `<nitro-docx>`, `<nitro-xlsx>`, `<nitro-pptx>` for host
applications that embed the editor inside a chat UI or agent harness.

### `getSelection(): Promise<NitroSelection | null>`

Returns a snapshot of the current selection, or `null` when no document is
loaded. Works in both read-only and edit mode.

```js
const sel = await viewer.getSelection();
// sel.format is "docx" | "xlsx" | "pptx"
```

**Return shapes** (see `NitroSelection` in `index.d.ts`):

| Format | Shape |
|---|---|
| docx | `{ format, text, page, start:{paragraph,run,offset}, end:{paragraph,run,offset}, collapsed }` |
| xlsx | `{ format, sheet, sheetIndex, range, anchor, values[][], truncated }` — values always `[]` in this release (see note below); `truncated:true` when the range spans >10 000 cells |
| pptx | `{ format, slide, shapeId, shapeName, text, textRange:{start,end} }` — `shapeId`/`shapeName`/`text`/`textRange` are `null` until pptx shape-selection is surfaced from Rust |

DOCX currently reports only a collapsed caret and the entire paragraph text,
not a selected text range. No `nitro-selectionchange` event is emitted.
These selection limitations remain open in #561.

**Copy-paste example (docx):**

```js
const el = document.querySelector("nitro-docx");
// After loading completes and the user makes a selection:
const sel = await el.getSelection();
if (sel) console.log("paragraph", sel.start?.paragraph, "text", sel.text);
```

### `dirty: boolean` (read-only getter)

`true` when the loaded document has unsaved edits; `false` in read-only mode or
after a successful `save()` / new document load. Only available in edit mode
(`readonly="false"`); always `false` in view-only mode.

### `nitro-change` event

Fired on the element when `dirty` flips. Bubbles and is composed (crosses shadow
DOM). Never fires during `loadBuffer` / `loadFile` / `loadUrl`, or for
scroll / zoom / selection / navigation — only on a first user edit and after
`save()` resets it.

```js
el.addEventListener("nitro-change", (e) => {
  console.log("dirty:", e.detail.dirty); // boolean
});
```

**When it fires / does not fire:**

| Event | Fires? |
|---|---|
| First keystroke / cell edit after load | ✅ `{dirty:true}` |
| Subsequent edits (already dirty) | ❌ no duplicate |
| `save()` success | ✅ `{dirty:false}` |
| `loadBuffer` / open | ❌ never |
| Scroll / zoom / nav / selection | ❌ never |
| `save()` with no edits (no-op) | ❌ never |

**Copy-paste example:**

```js
const el = document.querySelector("nitro-xlsx");
el.setAttribute("readonly", "false");
el.addEventListener("nitro-change", (e) => {
  document.getElementById("save-btn").disabled = !e.detail.dirty;
});
```

### `save()` no-op when unedited (#571)

`save()` on an unedited document returns byte-identical content (the engine
preserves untouched parts) and fires no `nitro-change` or other host-visible
events. Host autosave loops can call `save()` freely.

### Position, reload, and live readonly

The following APIs apply to `<nitro-docx>`, `<nitro-xlsx>` and `<nitro-pptx>`.
Indices are zero-based, zoom is a percentage, and scroll offsets use CSS pixels.

- `getPosition(): NitroPosition` synchronously returns the last painted
  `{page?, sheet?, sheetName?, slide?, scrollTop, scrollLeft, zoom?}`. Only the
  current format's index is present. Before open it returns zero scroll offsets.
- `setPosition(pos: NitroPositionInput): Promise<void>`
  applies supplied fields, clamps numeric indices/scroll/zoom, and leaves omitted
  fields unchanged. `sheet` accepts an index or exact sheet name; an unknown name
  or non-finite number rejects. It neither focuses the editor nor emits `nitro-change`.
- `loadBuffer(bytes: ArrayBuffer | Uint8Array, name?: string, opts?: {position?: NitroPositionInput}): Promise<void>`,
  `loadFile(file: Blob, name?: string, opts?: {position?: NitroPositionInput}): Promise<void>`,
  and `loadUrl(url: string, opts?: {position?: NitroPositionInput}): Promise<void>`
  apply the optional position at the opening frame. Omit `position` for the file's
  default location. These calls do not emit `nitro-change` or request focus.
- `reload(bytes: ArrayBuffer | Uint8Array, opts?: {keepPosition?: boolean, name?: string}): Promise<void>`
  replaces the file in the existing worker and retains the element and shadowRoot.
  `keepPosition` defaults to `true`; restored indices and scroll clamp to the new
  file. `false` opens at the default location. `name` defaults to the previous name.
  Mode stays unchanged; dirty resets silently to false and the edit journal starts clean. No `nitro-change` or focus change.
- `readonly: boolean` is writable. `el.readonly = false` enables editing in place;
  `true` disables editing and makes `save()` reject. Attribute changes work too:
  `readonly="false"` enables editing; absent, empty and other values mean readonly
  (the existing default). Changes preserve the document, position, and host focus.
- `nitro-mode: CustomEvent<{readonly: boolean}>` bubbles and is composed. It fires
  after a live mode transition finishes, not on initial open or a redundant assignment.
  Mode switches do not emit `nitro-change`.

```js
const editor = document.querySelector('nitro-docx');
await editor.loadUrl('/report.docx', { position: { page: 1, zoom: 100 } });
const position = editor.getPosition();
await editor.setPosition({ page: 2, scrollTop: 40 });
await editor.loadFile(fileInput.files[0], 'report.docx', { position });
await editor.loadBuffer(bytes, 'report.docx', { position });
await editor.reload(updatedBytes); // keep position and mode
await editor.reload(updatedBytes, { keepPosition: false, name: 'updated.docx' });
editor.addEventListener('nitro-mode', ({ detail }) => console.log(detail.readonly));
editor.readonly = false; // async transition; nitro-mode signals completion
editor.setAttribute('readonly', 'true');
// Spreadsheet example: await sheetEditor.setPosition({ sheet: 'Budget' });
```

### Font supply without Google Fonts (`fonts`, `fontSource`) — #566

By default Nitro may request webfont stylesheets from `fonts.googleapis.com`.
Supply fonts explicitly to prevent this:

```js
await initCore({
  fontSource: "off",        // no Google Fonts fallback
  fonts: [
    { family: "Inter", url: "/fonts/Inter.woff2", weight: "400", style: "normal" },
    { family: "NitroMono", url: "local(monospace)" },
    "/fonts/custom.css",    // stylesheet URL — all @font-face rules injected
  ],
});
```

**Per-element override** — set `el.fonts` before `loadBuffer`/`loadFile`/`loadUrl`:

```js
el.fonts = [{ family: "Inter", url: "/fonts/Inter.woff2" }];
await el.loadBuffer(buf, "doc.docx");
```

**Reuse fonts the host page already loaded:**

```js
await initCore({ fontSource: "page" });
// Nitro forwards families from document.fonts to the WASM worker.
```

**`fontSource` values:**

| Value | Behaviour |
|---|---|
| omitted | CSS2 base URL (`https://fonts.googleapis.com/css2`) |
| `"off"` / `false` | No remote fetch; unknown families get substitution diagnostics |
| `"page"` | Reuse families already in `document.fonts`; forward to worker |
| `string` (URL) | CSS2 mirror base URL (alternative CDN) |

The `fonts` array is loaded via `FontFace` on the main thread **and** forwarded
to the WASM worker. No request to `fonts.googleapis.com` or `fonts.gstatic.com`
is made when `fonts` is set or `fontSource` is `"off"` / `"page"`.

### Focus — load never steals focus (`autofocus`, `{focus:true}`) — #567

`loadBuffer` / `loadFile` / `loadUrl` **never move focus** by default. The editor
only receives focus on explicit user interaction (pointer click on the document).

**Opt-in focus after load:**

```js
// Attribute form (declarative):
<nitro-docx autofocus></nitro-docx>

// loadBuffer option form:
await el.loadBuffer(buf, "doc.docx", { focus: true });
```

User clicks on the document always focus the editor regardless of these options.

## Size budget

Hand-written loader JS plus generated `pkg/nitro_xml_wasm.js`, brotli-compressed,
is gated by `perf/budgets.json` → `loader_js_brotli_kb` via
`scripts/ci/bundle_budget.sh`. Ceiling ratchets down only. Per-format wasm
payload ceilings are TODO(#46).

### CSP and host-controlled hyperlinks

`initCore({ fontSource: false }): Promise<NitroCore>` disables automatic remote font
fetches. It resolves to `{glue, exports, memory}` (the loaded module, WASM exports,
and WASM memory or `null`); hosts normally only await readiness. Call it before creating viewers. With no explicit `fonts` list, opening
a document requests only same-origin loader modules, module workers, WASM and the
document bytes. Explicit font URLs still request the resources the host supplied.
No font or hyperlink event fires merely because a document opens.

Serve the loader, worker and WASM from the same origin. No inline script, blob
worker, `eval` or `new Function` is required. The renderer currently requires
inline styles; document images use data/blob URLs. The tested CSP is:

```http
Content-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:
```

Use an external module (for example `<script type="module" src="host.js"></script>`):

```js
// host.js
import { initCore, defineNitroDocx } from 'nitro-xml/loader';
await initCore({ fontSource: false });
defineNitroDocx();
const viewer = document.querySelector('nitro-docx');
viewer.addEventListener('nitro-hyperlink', event => {
  const { href, text } = event.detail;
  // Display a host-owned confirmation or route internally; Nitro never opens it.
  document.querySelector('#link-preview').textContent = `${text}: ${href}`;
});
await viewer.loadUrl('/documents/report.docx');
```

**Breaking change:** `nitro-hyperlink: CustomEvent<{ href: string; text: string }>`
now bubbles and crosses shadow boundaries (`composed: true`). It fires when a
rendered document hyperlink is clicked or a shell hyperlink action returns a
link. Nitro does not navigate or open a window. The event does not fire on load,
ordinary text clicks or selection. `href` is the target (or `#bookmark` for an
internal link); `text` is the clicked rendered text. Programmatic shell actions
without rendered text return `text: ""`. Hosts must choose whether/how to navigate.
The former `{target, open, ...}` detail is replaced by `{href, text}`.

### Stable `data-testid` contract

These attributes add no visible UI. Additions are compatible; renames/removals
are breaking changes. Query inside `viewer.shadowRoot` for the element surface
and shell, or the Office frame container for its chrome. IDs are scoped to that
container, not globally unique. Only mounted controls and painted cells exist;
offscreen virtualized cells, inactive ribbon panels and unopened menus may be absent.
Sheet/page/slide indices are zero-based; cell references are one-based A1.

| Control | Stable ID |
| --- | --- |
| Editing surface | `nitro-surface` |
| Find bar/input | `find-bar`, `find-input`, `find-status` |
| Ribbon tab | `ribbon-tab-home`, `ribbon-tab-view`, and `ribbon-tab-<lowercase tab>` |
| Ribbon command | `ribbon-command-<engine command or shell operation>`; menu-only controls use their lowercase hyphenated label |
| File menu/items | `file-menu`, `file-items`, `file-open`, `file-sample`, `file-save`, `file-close` |
| Status and items (Office frame) | `status`, `status-bar`, `status-position`, `status-find`, `status-<readout>` |
| Zoom (Office frame) | `zoom-control`, `zoom-input`, `zoom-zoom-in`, `zoom-zoom-out`, `zoom-reset` |
| Shell actions/zoom readout | `shell-<operation>`, `zoom-value` |
| Sheet tabs | `sheet-tabs` (Office frame), `sheet-tab-<index>` |
| Painted spreadsheet cell | `cell-A1`, `cell-B2`, etc. |
| Slide thumbnails/page tabs | `slide-thumb-<index>`, `page-tab-<index>` |
| Embedded toolbar/buttons | `embedded-toolbar`, `embedded-<lowercase hyphenated accessible label>` |

```js
const surface = viewer.shadowRoot.querySelector('[data-testid="nitro-surface"]');
const firstCell = viewer.shadowRoot.querySelector('[data-testid="cell-A1"]');
// Playwright pierces open shadow roots automatically:
// await page.getByTestId('cell-A1').click();
// await page.getByTestId('ribbon-tab-view').click();
```

The checked-in `scripts/e2e/host-api/testids.json` lists the required selectors
and ribbon commands by format/tab. `testids.py` guards all three formats in full
and embedded modes, including chrome replacement. The sample host is
`demo/host-api/csp.html`; its external scripts also drive the generated-fixture
round trips. `agent_fixtures.py` and `csp.py` exit non-zero on any failed check.
