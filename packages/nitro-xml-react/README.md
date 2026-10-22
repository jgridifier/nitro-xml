<!-- SPDX-License-Identifier: MIT -->
# @nitro-xml/react

SSR-safe React binding for [`nitro-xml`](../nitro-xml/README.md).

```tsx
import { useRef } from "react";
import { NitroViewer, type NitroViewerHandle } from "@nitro-xml/react";

export function App({ bytes }: { bytes: ArrayBuffer }) {
  const ref = useRef<NitroViewerHandle>(null);
  return (
    <NitroViewer
      ref={ref}
      file={bytes}
      fileName="sheet.xlsx"
      editable
      mode="embedded"
      onReady={() => console.log("ready")}
      onChange={(saved) => console.log("saved", saved.byteLength)}
      style={{ height: 480 }}
    />
  );
}
```

## Props

| Prop | Type | Notes |
|------|------|--------|
| `file` | `ArrayBuffer \| Uint8Array \| Blob \| File \| string` | Bytes or URL |
| `fileName` | `string` | Required for raw bytes (format from extension) |
| `editable` | `boolean` | Default `false` (`readonly` on the custom element) |
| `mode` | `"embedded" \| "full"` | Maps to the web component `mode` attribute |
| `onReady` | `(el) => void` | After open + first tick |
| `onChange` | `(bytes) => void` | `nitro-save` / save path |

WASM and custom elements load only inside `useEffect` — safe to import from
RSC / SSR bundles (no `window` at module top level).

## Peer deps

`react`, `react-dom` (≥18), `nitro-xml` (≥0.1.0).

## Example

See [`examples/vite-react`](../../examples/vite-react/) (Vite + Playwright open /
edit / save for xlsx, docx, pptx).
