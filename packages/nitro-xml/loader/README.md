<!-- SPDX-License-Identifier: MIT -->
# Loader API (#37)

The **only shipped JavaScript** in Nitro XML. It instantiates the wasm, forwards
browser events, applies each frame's command buffer, and performs one batched
DOM read-back (text measurement) per frame. Nothing else ships as JS
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
defineNitroXml();          // registers <nitro-xml>, pre-warms a worker
```

- Prefers `WebAssembly.compileStreaming` / the wasm-bindgen streaming init path
  with a `Response`.
- Compiled `WebAssembly.Module` is cached in memory for the page lifetime (Cache
  API used when fetch URLs allow).
- Worker is pre-warmed on `defineNitroXml()`.
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

## Worker messaging

One dedicated worker per document. Messages:

| Main → worker | Worker → main |
|---|---|
| `prewarm` | `ready` |
| `open` (+ transferred `ArrayBuffer`) | `opened` |
| `events` (+ batch bytes) | `events-acked` |
| `take-frame` | `frame` (commands + measure SoA) |
| `measure-results` (Float64Array) | `measure-acked` |
| `dispose` | `disposed` |

DOM/canvas apply and measure batches run on the main thread; parse/model/layout stay in the worker; pptx text layout stays on the main thread (`docs/adr/0002-renderer-port.md`, #36).

## Headless session (same thread)

```js
import { openDocument } from "nitro-xml/loader";

const session = await openDocument(bytes, { name: "sheet.xlsx", canvas, root });
session.pushEvent({ kind: 6, time_ms: 0, width: 800, height: 600, dpr: 1 });
session.tick();   // measure batch → apply commands → flush events
session.dispose();
```

`FinalizationRegistry` on the session calls `handle.free()` if the embedder
drops the last reference without `dispose()` (wasm-bindgen also registers the
raw `Handle`).

## Size budget

Hand-written loader JS plus generated `pkg/nitro_xml_wasm.js`, brotli-compressed,
is gated by `perf/budgets.json` → `loader_js_brotli_kb` via
`scripts/ci/bundle_budget.sh`. Ceiling ratchets down only. Per-format wasm
payload ceilings are TODO(#46).
