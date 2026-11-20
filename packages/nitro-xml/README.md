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

Returns a snapshot of the current selection (#561). Call after loading
completes and after the user's selection gesture. No `nitro-selectionchange`
event is emitted, and reading the selection never marks the document dirty.

- **DOCX**: a double-click selects the word under the pointer and returns its
  `text` with `start`/`end` `{paragraph, run, offset}` and `collapsed: false`.
  Offsets count code points from the start of the paragraph (`run` is `0`).
  Otherwise it returns the caret paragraph's text with equal `start`/`end` and
  `collapsed: true`. The word selection is not highlighted, and typing inserts
  at the caret instead of replacing it. Drag and Shift+arrow ranges are not
  reported.
- **XLSX**: `{format, sheet, sheetIndex, range, anchor, values, truncated}`.
  `values` holds the stored cell values row by row: numbers, strings, booleans,
  error text such as `"#DIV/0!"`, or `null` for an empty cell. At most 10,000
  cells are returned; past that, whole rows are dropped and `truncated` is `true`.
- **PPTX**: a selected shape returns `shapeId`, `shapeName` and its whole `text`
  with `textRange: null`. A double-click on a word returns that word as `text`
  and its code-point `textRange` in the shape text, with paragraphs joined by
  `"\n"`. With nothing selected, the shape fields are `null`.

```js
const selection = await viewer.getSelection();
console.log(selection); // e.g. { format: "xlsx", range: "B2:C3", values: [[4, 6], [6, 9]], ... }
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
