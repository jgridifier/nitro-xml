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

## Loader size gate

```bash
bash scripts/ci/bundle_budget.sh   # must stay ≤ 17408 B brotli (eager path)
```

`loader/lazy/**` is excluded from the eager measure (dynamic `import()` only).
