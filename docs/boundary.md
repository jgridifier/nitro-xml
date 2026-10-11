<!-- SPDX-License-Identifier: MIT -->
# JS/WASM boundary (#14)

Stage 2 transport decision for Nitro XML. Everything that ships is Rust/WASM; the only JS is the minimal loader (#37). This document records the conventions, the measured bulk-transfer numbers, and which payload uses which transport.

Upstream painters are ported one-to-one; the **only** allowed change on the render path is batching each frame's draw/DOM calls into one handoff (PRODUCT_SPEC.md §6 R0).

Speed testing beyond the transfer contrast below is deferred (Jared, 2026-10-11): development comes first. The `bench` cargo feature (synthetic grids, serde/JSON export arms) is **not** in the shipped wasm.

## Status

| Item | Status |
|---|---|
| Handle / error / warning shapes | In `nitro-xml-wasm` (`handle.rs`) |
| String table | In `nitro-xml-wasm` (`transport.rs`, shipped) |
| Command buffer + measure batch + layout read-back | In `nitro-xml-render` (`command_buffer/`, `measure/`); wasm handle holds instances |
| Event batch wire format | In `events.rs` |
| Synthetic SoA / serde / JSON bench arms | Behind non-default `bench` feature (`bench.rs`); not shipped |
| Loader (`FinalizationRegistry`, command-buffer applier, event coalesce, measure + layout read-back) | `packages/nitro-xml/loader/` (#37); opcode gaps remain for painter ports |
| Worker vs main-thread split per painter | Done — threading section of `docs/adr/0002-renderer-port.md` (#36) |
| Perf-history ingestion | TODO(#8) |

## Handle conventions

- **Ownership.** `open → Handle` copies document bytes once into wasm linear memory. The handle owns the model, string table, pending events, the current frame buffer, measure batch and layout read-back batch.
- **`free()`.** wasm-bindgen generates `Handle.free()`. Embedders must call it. Linear memory does not shrink; dropping the handle releases the Rust allocations.
- **`FinalizationRegistry`.** The loader (#37) registers every handle so a GC'd JS reference still calls `free()`. That is a safety net, not the primary API.
- **Errors.** `OoxmlError { code, message, part? }` — stable `code` strings are the machine API; `part` is set only for part-local failures.
- **Warnings.** Same shape as errors; collected on the handle and read with `warnings()` / `warningCount`.

## Worker pattern

- **One dedicated worker per document.** The file `ArrayBuffer` is transferred into the worker (zero copy), then copied once into wasm.
- **No SharedArrayBuffer by default** (no COOP/COEP for embedders). A threaded build is opt-in only (PRODUCT_SPEC.md Q2).
- **Crash recovery.** Viewer: reload once in a fresh worker. Editor: replay the op journal into a fresh worker (PRODUCT_SPEC.md §5(e)).
- Painter placement is fixed at R0 (#36): see the threading section of `docs/adr/0002-renderer-port.md`. Parse/model/layout in the worker; DOM paint and measurement on the main thread; pptx text layout on the main thread; no OffscreenCanvas on the M1 view path; no SharedArrayBuffer. Matches `packages/nitro-xml/loader/` (`WorkerDocument` / `DocumentSession`).

## Event-forwarding protocol

The loader coalesces browser events and posts **at most one** batch per animation frame into `Handle.pushEventBatch(bytes)`.

Binary layout (little-endian), version `1`:

| Kind | Tag | Payload |
|---|---|---|
| Input | 1 | `u32 text_id` |
| Pointer | 2 | `f32 x,y` `u8 type` `u8 button` `u16 buttons` |
| Wheel | 3 | `f32 x,y,dx,dy` |
| Keyboard | 4 | `u32 key_id, code_id` `u8 type` `u8 mods` |
| Composition | 5 | `u8 type` `u32 text_id` |
| Resize | 6 | `f32 width, height, dpr` |

Malformed batches return `OoxmlError` (`event_batch_*`); they never panic. The engine drains with `drainEvents()`.

## Interned string table

Per document. `intern(str) → u32`, `resolve(id) → string`. Id `0` is always `""`. Cell text, font names, DOM tag/attr names in the command buffer, and event text ids all use this table so bulk SoA traffic stays numeric.

## Per-frame command buffer

One `Uint8Array` of opcodes per frame, taken with `takeFrameCommands()` and applied by the loader in a single pass (#37). No per-element WASM→JS calls on the render path.

### Seed opcodes (Stage 2)

| Op | Tag | Payload |
|---|---|---|
| FillRect | 1 | `i32 x,y,w,h` `u32 color` |
| StrokeRect | 2 | `i32 x,y,w,h` `u32 color` `f32 line` |
| DrawImage | 3 | `u32 img` `i32 x,y,w,h` |
| DrawText | 4 | `i32 x,y` `u32 font` `f32 size` `u32 text` `u32 color` |
| CreateElement | 5 | `u32 id, tag, parent` |
| SetAttr | 6 | `u32 id, key, val` |
| AppendChild | 7 | `u32 parent, child` |
| SetText | 8 | `u32 id, text_id` |
| SetStyle | 9 | `u32 id, prop_id, val_id` |
| SetInnerMarkup | 10 | `u32 parent, markup` — parse SVG/HTML fragment into `parent` in one pass (#28 charts) |

Opcodes **8–9** (xlsx grid #18) cover DOM `textContent` and `style` property writes for the cell layer. Opcode **10** (`SetInnerMarkup`) is for trusted, Rust-generated markup only (chart S1 SVG); remaining gaps below still apply to SVG NS creates, etc. Keep this table aligned with `rs/crates/nitro-xml-render/src/command_buffer/ops.rs` and `packages/nitro-xml/loader/apply.js`.

### Opcode coverage gaps (vs upstream painters / PR #111)

The seed set plus **SetText** / **SetStyle** / **SetInnerMarkup** is still **not** enough for a full one-to-one painter port. Missing call kinds that upstream actually makes (see `docs/painters.md` / `tools/painter-calls`), to be added as painter ports extend the loader applier (placement in `docs/adr/0002-renderer-port.md` #36):

- `createElementNS` / SVG element creation (charts, sparklines, SmartArt, connectors, ink, WordArt)
- Text nodes and `textContent` / `createTextNode` updates (partially covered by SetText for grid cells)
- Inline style property writes (`element.style.*`, CSS custom properties) (partially covered by SetStyle)
- `classList` add / remove / toggle
- Tree surgery: `insertBefore`, `remove`, `replaceChildren`, `replaceWith`
- Markup paths beyond trusted SetInnerMarkup: `DOMParser` / `svgNode` string parse into the DOM
- Canvas 2D state: `save` / `restore`, `setTransform` / `resetTransform`, `fillStyle` / `strokeStyle`, path ops (`beginPath`, `moveTo`, `lineTo`, `arc`, `fill`, `stroke`)
- Canvas text: `fillText`, `measureText` (measurement stays batched; draw ops still need opcodes)
- Attribute NS variants (`setAttributeNS`) and dataset / ARIA property writes
- MathML element creation (docx equations)
- WebGL / three.js handoff for opt-in 3D chart and SmartArt painters (likely a separate buffer family, not DOM opcodes)

### Batched text measurement

Fonts and measurement follow upstream for now. Rust queues `{font_id, size_px, text_id}` SoA requests; the loader runs `measureText` once per batch and writes widths back with `setMeasureResults` (`packages/nitro-xml/loader/measure.js`, #37). Never one wasm call per string.

### Post-apply layout read-back

Node-layout queries run **after** a frame is applied. Types live in `nitro-xml-render::measure` (`LayoutReadBatch`). Rust queues requests keyed by command-buffer **node id**; the loader takes them with the frame (`takeLayoutReqs` / worker `frame.layoutReqs`) and answers the whole batch on the **next** frame with one `setLayoutResults` (`packages/nitro-xml/loader/layout-readback.js`). No per-element WASM→JS calls; iterative autofit uses this batch channel.

| Kind | Tag | Request fields | Result (`f64` × 4 per request) | Upstream |
|---|---:|---|---|---|
| SpanRect | 1 | `node_id` | `x, y, w, h` from `getBoundingClientRect` | docx `canvas-measurer.ts` |
| ScrollHeight | 2 | `node_id` | `scrollHeight, 0, 0, 0` | pptx `text-autofit-shrink-measure.ts`, `shape-autofit-resize.ts` |
| ComputedStyle | 3 | `node_id`, `prop_id` (string table; e.g. `font-size`, `line-height`) | `parseFloat(getComputedStyle(el).getPropertyValue(prop))` or `NaN`, then zeros | same autofit path |
| NaturalWidth | 4 | `node_id` (`<img>`) | `naturalWidth, naturalHeight, 0, 0` | P5 `image-native-size.ts` |

Session / worker tick order: answer prior-frame layout → measure batch → `applyCommandBuffer` → take this frame's layout requests.

## Bulk-transfer contrast (recorded; further speed work deferred)

Harness: `perf/micro/boundary.bench.mjs` (test/bench only, never shipped). Build with `bash scripts/build-wasm.sh --features bench` (or `cd perf/micro && npm run bench`). Headless Chrome, COOP/COEP, 2 warm-ups + 7 timed runs, median ms. Synthetic SoA records.

The SoA arm is **zero-copy view creation plus a scan** over wasm memory. The serde-wasm-bindgen and JSON arms **materialise a full JS object graph**. That asymmetry is intentional for the transport decision.

**Run recorded 2026-10-11** (agent host). Reviewer reproduction on pinned Chrome 154: SoA 0.89 ms vs serde 357 ms vs JSON 378 ms at 1M records (~400×).

| N records | SoA view+scan | serde-wasm-bindgen | JSON string |
|---|---:|---:|---:|
| 1,000 | 0.015 ms | 0.380 ms | 0.310 ms |
| 100,000 | 0.565 ms | 25.270 ms | 28.035 ms |
| 1,000,000 | 1.400 ms | 339.020 ms | 349.735 ms |

**Reading.** SoA typed-array views are the bulk-transfer rule. serde and JSON are reserved for rare diagnostics. Further command-buffer vs per-op timing is deferred until development needs it.

Reproduce (optional; not a CI gate):

```bash
cd perf/micro && npm ci && npm run bench   # builds wasm with --features bench
```

Shipped package (no bench feature):

```bash
bash scripts/build-wasm.sh                # must stay near the pre-#14 size
```

## Decision table

| Payload | Transport | Why |
|---|---|---|
| Cell / run / shape numeric grids | SoA typed-array views over wasm memory | Fastest measured; no per-cell calls |
| Interned strings | `u32` ids in SoA; resolve on demand | Keeps bulk traffic numeric |
| Images / media / `vbaProject.bin` | `Uint8Array` view or transfer | Opaque bytes; macros never run |
| Frame canvas draw + DOM build | Per-frame command buffer | One handoff; PRODUCT_SPEC R0 port rule |
| Text measurement | Batched SoA reqs + widths reply | Upstream measurement path without per-string calls |
| Post-apply layout (span rect, scrollHeight, computed style, naturalWidth) | Batched node-id SoA + 4-lane reply next frame | Autofit / span probe / tile math without per-element calls |
| Input / pointer / wheel / keyboard / composition / resize | One binary event batch per frame | Loader coalesce; ≤1 message/frame |
| Open document | `ArrayBuffer` transfer into worker, one copy into wasm | Zero-copy into worker |
| `OoxmlError` / `Warning` | Typed `wasm_bindgen` objects | Stable `code`; optional `part` |
| Rare diagnostics / debug dumps | JSON string OK | Infrequent; clarity over speed |
| Never | Per-cell or per-element render calls | Erases WASM gains (PRODUCT_SPEC §5) |

## Follow-ups

- **#37** — loader skeleton landed (`packages/nitro-xml/loader/`); extend opcodes from the gap list as painters port.
- **#36** — done (threading section of `docs/adr/0002-renderer-port.md`). Opcode gap list above remains for painter/loader work.
- **#8** — ingest `perf/micro/results/boundary-*.json` into the perf history gate when speed testing resumes.
