<!-- SPDX-License-Identifier: MIT -->
# @nitro-xml/react

SSR-safe React binding for [`nitro-xml`](../nitro-xml/README.md).

After publication: `npm install @nitro-xml/react@0.1.0 nitro-xml@0.1.0 react react-dom`.

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


The host persists returned bytes. `onChange` reports saves, not every keystroke. Mount a new editor for a new document and disable save while it loads.

## Props

| Prop | Type | Notes |
|------|------|--------|
| `file` | `ArrayBuffer \| Uint8Array \| Blob \| File \| string` | Bytes or URL |
| `fileName` | `string` | Required for raw bytes (format from extension) |
| `editable` | `boolean` | Default `false` (`readonly` on the custom element) |
| `mode` | `"embedded" \| "full"` | Maps to the web component `mode` attribute |
| `onReady` | `(el) => void` | After open + first tick |
| `onError` | `(error) => void` | Open, paint, and save failures; `code`, `limit`, `actual`, and `part` when available |
| `onChange` | `(bytes) => void` | `nitro-save` / save path |

WASM and custom elements load only inside `useEffect` — safe to import from
RSC / SSR bundles (no `window` at module top level).

## Peer deps

`react`, `react-dom` (≥18). Runtime dependency: `nitro-xml ^0.1.0`.

## Example

See [`examples/vite-react`](../../examples/vite-react/) (Vite + Playwright open /
edit / save for xlsx, docx, pptx).

Failed imperative saves reject as well as notifying `onError`. Element `loadBuffer`, `loadFile`, and `loadUrl` reject on open failure and emit an `error` CustomEvent (`detail` is the error). Size limits use the existing `input_too_large`, `part_too_large`, `total_too_large`, `too_many_parts`, and `compression_ratio` codes. Worker requests exceeding the existing 120-second harness ceiling terminate the worker and reject with `NITRO_TIMEOUT` (milliseconds). VBA parts are preserved as opaque package data and are never executed.
