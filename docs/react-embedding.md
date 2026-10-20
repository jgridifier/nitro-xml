<!-- SPDX-License-Identifier: MIT -->
# React embedding latency

Numbers from `perf/react-embed` (Vite + React 18, `mode="embedded"`).
Re-run with `cd perf/react-embed && npm install && npm run bench` after a
`bash scripts/build-wasm.sh`. No budget or CI gate yet — measurement harness only.

Machine: cloud agent VM, Chromium headless (Playwright), cold page per scenario.

## Startup (cold) — before / after streaming-instantiate fix

| Step | Before (#307 Module-first) | After (Response + overlap) |
|---|---|---|
| WASM fetch | ~2 ms | ~3 ms |
| `compileStreaming` | ~37 ms (awaited first) | ~58 ms (overlaps instantiate) |
| instantiate | ~0.7 ms non-streaming (`Module`) | ~56 ms `instantiateStreaming` |
| streaming flags | compile=true, **instantiate=false** | compile=true, **instantiate=true** |
| `initCore` | ~74 ms | ~68 ms (compile ∥ instantiate) |
| Worker construct | ~0.1 ms | ~0.1 ms |
| Peak memory | one wasm instance + Cache API entry | unchanged residency |

`mode="embedded"` also kicks `lazy/preload.js` as soon as the attribute is set.
Hosts can still call `preload()` earlier from route layout for maximum overlap.

## Phase breakdown (mount → first paint)

Instrumented path marks: fixture fetch, worker ready / `initCore`, open
round-trip (worker `Handle.new`, `openDocument`, editable upgrade), host
`loadBuffer`, first `tick` / command buffer, measure + layout round-trips,
paint wait. Worker open sub-phases ride on `opened.trace` when
`globalThis.__nitroBench` is set (bench only).

| Format | Tier | mount→paint | open RT | openDocument | “chrome” bucket | 1st tick / paint wait | frames |
|---|---|---:|---:|---:|---:|---:|---:|
| xlsx | S | 255 | 145 | 48 | **33** | 103 | 1 |
| xlsx | M | 425 | 353 | 71 | **219** | 61 | 1 |
| docx | S | 162 | 105 | 35 | 3 | 54 | 4 |
| docx | M | 181 | 107 | 38 | 8 | 71 | 4 |
| pptx | S | 134 | 107 | 41 | 3 | 24 | 1 |
| pptx | M | 133 | 115 | 46 | 8 | 14 | 1 |

**Hottest bucket (xlsx):** most of the open “chrome” time is **`setReadOnly(false)`** (~200 ms on M), not `loadEditorChrome` / cell encode. Follow-up #320 defers the editable upgrade until after first paint.

Reproduce:

```bash
bash scripts/build-wasm.sh
cd perf/react-embed && npm install && npm run bench
# Phase table prints under “Phase breakdown”; JSON in results/latest.json
```

## Mount → first paint (post #308/#312/#313 stack)

| Format | Tier | open ms | paint ms | mount→paint ms | bytes |
|---|---|---|---|---|---|
| xlsx | S | 177 | 113 | 289 | 8 012 |
| xlsx | M | 370 | 62 | 432 | 503 874 |
| docx | S | 116 | 58 | 174 | 2 945 |
| docx | M | 113 | 76 | 190 | 403 491 |
| pptx | S | 116 | 29 | 145 | 11 993 |
| pptx | M | 124 | 18 | 142 | 442 863 |

xlsx S/M remain the largest embed cost (open+first frame); grid encode cuts are a separate track (#305/#310/#315).

## Edit → repaint (p50 / p95)

| Interaction | p50 ms | p95 ms | n |
|---|---|---|---|
| xlsx cell (`setCellInput`) | 33.4 | 33.9 | 20 |
| docx type (`editorKey`) | 16.6 | 16.9 | 20 |
| pptx type (`editorKey`) | 16.6 | 16.8 | 20 |
| bold (`shellAction cmd home.bold`) | 25.9 | 54.2 | 10 |

## Remount / StrictMode

| Scenario | Result |
|---|---|
| Remount same xlsx S | first 254 ms → second 28 ms (Δ −225 ms; module + parked worker) |
| React 18 StrictMode double-mount | `host_ready_count=2`, gap ~0.9 ms; mount→paint ~169 ms |

## Landed cuts

| PR | Change |
|---|---|
| #307 | Cache API put + in-memory `Module` reuse + host `preload()` |
| #308 | Park worker on dispose; rAF `applyEdit` coalesce |
| #312 | Skip host `loadEditorModule` before worker open; defer embedded toolbar |
| #313 | Auto-`preload()` when `mode="embedded"` is set |
| #318 | Cold `initCore` passes `Response` again so `instantiateStreaming` runs; overlap compile |
| #319 | Phase marks + breakdown table (this) |
| #320 | Defer `setReadOnly(false)` + chrome until after first paint |

Stay out of xlsx grid paint / StringTable encode (separate track). Keep eager
loader ≤ 17 408 B brotli (new JS under `loader/lazy/`). No budget, threshold,
or fixture changes. Stay out of `.github/workflows/` and #113.
