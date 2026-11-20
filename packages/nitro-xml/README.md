<!-- SPDX-License-Identifier: MIT -->
# nitro-xml (npm)

Open, view, edit and save `.xlsx`, `.docx` and `.pptx` in the browser via
Rust/WebAssembly. This package ships the **WASM binary**, the **tiny loader**
(eager first-load budget ≤ 17 408 B brotli) and **TypeScript types**.

## Install

```bash
npm install nitro-xml
# React binding (separate package; does not grow the eager loader):
npm install @nitro-xml/react react react-dom
```

Names are recommended in [`docs/PUBLISHING_NAMES.md`](../../docs/PUBLISHING_NAMES.md);
nothing is published until Jared registers the scope and runs a release.

## ESM + lazy WASM

```js
import {
  wasmSupported,
  initCore,
  defineNitroXlsx,
  defineNitroDocx,
  defineNitroPptx,
} from "nitro-xml/loader";

if (!wasmSupported()) throw new Error("WebAssembly required");
await initCore(); // compileStreaming / instantiateStreaming; Module cached
defineNitroXlsx();
defineNitroDocx();
defineNitroPptx();
```

- Default import `"nitro-xml"` re-exports the loader (tree-shakeable; side-effect
  free except `loader/element.js` / `loader/index.js` when you call `define*`).
- WASM bytes load only when `initCore` / `WorkerDocument.open` run — not at
  module evaluate time.
- Advanced: `"nitro-xml/wasm"` is the raw wasm-bindgen glue; `"nitro-xml/nitro_xml_wasm_bg.wasm"` is the binary URL for bundlers.

## React

```tsx
import { useRef, useState } from "react";
import { NitroViewer, type NitroViewerHandle } from "@nitro-xml/react";

export function Editor({ file, fileName, persist }: {
  file: File | Uint8Array;
  fileName: string; // e.g. report.docx, budget.xlsx or slides.pptx
  persist: (bytes: Uint8Array) => Promise<void>;
}) {
  const ref = useRef<NitroViewerHandle>(null);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState("");
  async function save() {
    try {
      if (ref.current) await persist(await ref.current.save());
    } catch (e) { setError(String(e)); }
  }
  return <>
    <button disabled={!ready} onClick={save}>Save</button>
    <p role="alert">{error}</p>
    <NitroViewer key={fileName} ref={ref} file={file} fileName={fileName}
      editable mode="embedded" style={{ height: 600 }}
      onReady={() => setReady(true)} onError={e => setError(e.message)} />
  </>;
}
```

For Vite production builds, keep dynamically imported worker chunks as ES modules:

```js
// vite.config.js
import { defineConfig } from "vite";
export default defineConfig({ worker: { format: "es" } });
```


The host owns persistence; `save()` returns bytes and `onChange` reports saves.
See [0.1.0 release notes](https://github.com/jgridifier/nitro-xml/blob/main/docs/releases/0.1.0.md) for limits and known losses.

## Host API

For hosts embedding Nitro inside a chat UI or agent harness — full docs in
[`loader/README.md#host-api`](loader/README.md#host-api).

### `getSelection(): Promise<NitroSelection | null>`

Returns `Promise<NitroSelection | null>`. The current implementation is incomplete
(#561): DOCX returns the caret paragraph's text and equal `start`/`end`
`{paragraph, run, offset}` positions with `collapsed: true`; XLSX returns
`{format, sheet, sheetIndex, range, anchor, values: [], truncated}` (values are
not populated); PPTX returns `{format, slide, shapeId: null, shapeName: null,
text: null, textRange: null}`. Do not use these placeholders to anchor comments.
No `nitro-selectionchange` event is emitted. Call after loading completes and
after the user's selection gesture:

```js
const selection = await viewer.getSelection();
console.log(selection); // Snapshot only; see the limitations above.
```

### `dirty: boolean` getter + `nitro-change` event

For `readonly="false"`, real typing (and committed XLSX cell edits) sets the
read-only getter to `true`. The bubbling, composed event has
`detail: { dirty: boolean }`. `save(): Promise<Uint8Array>` returns the package
bytes and resets dirty after success.

```js
el.addEventListener("nitro-change", (e) => {
  console.log("dirty:", e.detail.dirty, el.dirty); // true on first edit, false after save()
});
await el.save(); // resets dirty → false, fires nitro-change {dirty:false}
```

Fires only when `dirty` flips. Never fires during load, scroll, zoom, navigation,
or `save()` on an unedited document.

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
  default location (xlsx `topLeftCell`, saved scroll/zoom). The viewport is stabilised
  before the saved position is applied, so chrome layout during open does not shift it.
  These calls do not emit `nitro-change` or request focus.
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
To avoid that, supply fonts explicitly:

```js
// Global default — applies to all future WorkerDocument / element loads.
await initCore({
  fontSource: "off",          // stop any Google Fonts fallback
  fonts: [
    // Load from an explicit URL (served from your CDN / bundle):
    { family: "Inter", url: "/fonts/Inter.woff2", weight: "400", style: "normal" },
    // Or reuse a system font under a custom family name:
    { family: "NitroMono", url: "local(monospace)" },
    // Stylesheet URL — all @font-face rules inside are injected:
    "/fonts/custom.css",
  ],
});
```

**Per-element override** (set before `loadBuffer`/`loadFile`/`loadUrl`):

```js
el.fonts = [{ family: "Inter", url: "/fonts/Inter.woff2" }];
await el.loadBuffer(buf, "doc.docx");
```

**Reuse fonts the host page already loaded** (`fontSource: "page"`):

```js
await initCore({ fontSource: "page" });
// Nitro forwards families from document.fonts to the WASM worker — no Google request.
```

**`fontSource` values:**

| Value | Behaviour |
|---|---|
| omitted | CSS2 base URL (`https://fonts.googleapis.com/css2`) |
| `"off"` / `false` | No remote fetch; unknown families get substitution diagnostics |
| `"page"` | Reuse families already in `document.fonts`; forward to worker |
| `string` (URL) | CSS2 mirror base URL (alternative CDN) |

The `fonts` array is loaded via `FontFace` on the main thread **and** forwarded to
the WASM worker (where text is measured). No request to `fonts.googleapis.com` or
`fonts.gstatic.com` is made when `fonts` is set or `fontSource` is `"off"` / `"page"`.

### Focus — load never steals focus (`autofocus`, `{focus:true}`) — #567

By default, `loadBuffer` / `loadFile` / `loadUrl` **never move focus** away from
whatever the host page has focused (e.g. a chat textarea). The editor only receives
focus on explicit user interaction (pointer click on the document area).

**Opt-in focus after load:**

```js
// Option A: attribute (declarative)
<nitro-docx autofocus></nitro-docx>

// Option B: loadBuffer option
await el.loadBuffer(buf, "doc.docx", { focus: true });
```

User clicks on the document always focus the editor regardless of these options.

## Loader size gate

```bash
bash scripts/ci/bundle_budget.sh   # must stay ≤ 17408 B brotli (eager path)
```

`loader/lazy/**` is excluded from the eager measure (dynamic `import()` only).

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
