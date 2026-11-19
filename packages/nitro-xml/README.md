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

## Loader size gate

```bash
bash scripts/ci/bundle_budget.sh   # must stay ≤ 17408 B brotli (eager path)
```

`loader/lazy/**` is excluded from the eager measure (dynamic `import()` only).
