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
defineNitroXml();          // registers <nitro-xml>, pre-warms a worker
defineNitroXlsx();         // registers <nitro-xlsx readonly> xlsx viewer (#19)
```

`defineNitroXlsx()` from the same entry registers `<nitro-xlsx readonly>` for the
Stage-3 xlsx grid viewer (worker + command buffer). Use `readonly` (default) for
view-only embeds; editing stays on `<nitro-xml>` / future editor paths.

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

## Size budget

Hand-written loader JS plus generated `pkg/nitro_xml_wasm.js`, brotli-compressed,
is gated by `perf/budgets.json` → `loader_js_brotli_kb` via
`scripts/ci/bundle_budget.sh`. Ceiling ratchets down only. Per-format wasm
payload ceilings are TODO(#46).
