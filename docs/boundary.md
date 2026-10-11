<!-- SPDX-License-Identifier: MIT -->
# JS/WASM boundary (#14)

Stage 2 transport decision for Nitro XML. Everything that ships is Rust/WASM; the only JS is the minimal loader (#37). This document records the conventions, the measured bulk-transfer numbers, and which payload uses which transport.

Upstream painters are ported one-to-one; the **only** allowed change on the render path is batching each frame's draw/DOM calls into one handoff (PRODUCT_SPEC.md §6 R0).

## Status

| Item | Status |
|---|---|
| Handle / error / warning shapes | In `nitro-xml-wasm` (`handle.rs`) |
| SoA views, string table, command buffer, measure batch | In `transport.rs` |
| Event batch wire format | In `events.rs` |
| Loader (`FinalizationRegistry`, command-buffer applier, event coalesce) | TODO(#37) |
| Worker vs main-thread split per painter | TODO(#36) |
| Perf-history ingestion of these numbers | TODO(#8) |

## Handle conventions

- **Ownership.** `open → Handle` copies document bytes once into wasm linear memory. The handle owns the model, string table, pending events, the current frame buffer and measure batch.
- **`free()`.** wasm-bindgen generates `Handle.free()`. Embedders must call it. Linear memory does not shrink; dropping the handle releases the Rust allocations.
- **`FinalizationRegistry`.** The loader (#37) registers every handle so a GC'd JS reference still calls `free()`. That is a safety net, not the primary API.
- **Errors.** `OoxmlError { code, message, part? }` — stable `code` strings are the machine API; `part` is set only for part-local failures.
- **Warnings.** Same shape as errors; collected on the handle and read with `warnings()` / `warningCount`.

## Worker pattern

- **One dedicated worker per document.** The file `ArrayBuffer` is transferred into the worker (zero copy), then copied once into wasm.
- **No SharedArrayBuffer by default** (no COOP/COEP for embedders). A threaded build is opt-in only (PRODUCT_SPEC.md Q2).
- **Crash recovery.** Viewer: reload once in a fresh worker. Editor: replay the op journal into a fresh worker (PRODUCT_SPEC.md §5(e)).
- Painter placement (DOM apply on main thread vs OffscreenCanvas in the worker) is fixed at R0 (#36).

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

| Op | Tag | Payload |
|---|---|---|
| FillRect | 1 | `i32 x,y,w,h` `u32 color` |
| StrokeRect | 2 | `i32 x,y,w,h` `u32 color` `f32 line` |
| DrawImage | 3 | `u32 img` `i32 x,y,w,h` |
| DrawText | 4 | `i32 x,y` `u32 font` `f32 size` `u32 text` `u32 color` |
| CreateElement | 5 | `u32 id, tag, parent` |
| SetAttr | 6 | `u32 id, key, val` |
| AppendChild | 7 | `u32 parent, child` |

### Batched text measurement

Fonts and measurement follow upstream for now. Rust queues `{font_id, size_px, text_id}` SoA requests; the loader runs `measureText` once per batch and writes widths back with `setMeasureResults` (TODO(#37)). Never one wasm call per string.

## Benchmarks

Harness: `perf/micro/boundary.bench.mjs` (test/bench only, never shipped). Headless Chrome, COOP/COEP for full `performance.now()` resolution, 2 warm-ups + 7 timed runs, median ms. Synthetic SoA records (not OOXML fixtures). Results also in `perf/micro/results/`.

**Run recorded 2026-10-11** on this agent host, Chrome 148, commit base `2779b69` (pre-PR).

| N records | SoA view+scan | serde-wasm-bindgen | JSON string |
|---|---:|---:|---:|
| 1,000 | 0.015 ms | 0.380 ms | 0.310 ms |
| 100,000 | 0.565 ms | 25.270 ms | 28.035 ms |
| 1,000,000 | 1.400 ms | 339.020 ms | 349.735 ms |

| Workload | Median |
|---|---:|
| Encode + take command buffer (1M mixed ops) | 16.250 ms |
| Per-call `touchCell` × 50k (trivial f64 getter) | 0.300 ms |
| Linear estimate of that getter at 1M | ≈ 6 ms |

**Reading.** SoA views are about **240×** faster than serde-wasm-bindgen and JSON at 1M records for a view+scan. serde and JSON also allocate a full JS object graph. The trivial per-call getter is a lower bound only; real per-element canvas/DOM calls from wasm are forbidden on the render path and would dominate frame time. The command buffer is the required handoff shape regardless.

Reproduce:

```bash
bash scripts/build-wasm.sh
cd perf/micro && npm ci && npm run bench
```

## Decision table

| Payload | Transport | Why |
|---|---|---|
| Cell / run / shape numeric grids | SoA typed-array views over wasm memory | Fastest measured; no per-cell calls |
| Interned strings | `u32` ids in SoA; resolve on demand | Keeps bulk traffic numeric |
| Images / media / `vbaProject.bin` | `Uint8Array` view or transfer | Opaque bytes; macros never run |
| Frame canvas draw + DOM build | Per-frame command buffer | One handoff; PRODUCT_SPEC R0 port rule |
| Text measurement | Batched SoA reqs + widths reply | Upstream measurement path without per-string calls |
| Input / pointer / wheel / keyboard / composition / resize | One binary event batch per frame | Loader coalesce; ≤1 message/frame |
| Open document | `ArrayBuffer` transfer into worker, one copy into wasm | Zero-copy into worker |
| `OoxmlError` / `Warning` | Typed `wasm_bindgen` objects | Stable `code`; optional `part` |
| Rare diagnostics / debug dumps | JSON string OK | Infrequent; clarity over speed |
| Never | Per-cell or per-element render calls | Erases WASM gains (PRODUCT_SPEC §5) |

## Follow-ups

- **#37** — loader: `FinalizationRegistry`, command-buffer applier, measureText batch, event coalesce, worker host.
- **#36** — which painters apply on the main thread vs OffscreenCanvas in the worker.
- **#8** — ingest `perf/micro/results/boundary-*.json` into the perf history gate.
