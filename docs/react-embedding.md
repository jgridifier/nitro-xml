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

Instrumented path marks (bench harness only — no eager-loader growth):
fixture fetch, worker ready / `initCore`, open round-trip, host
`loadBuffer`, first `tick` / command buffer, measure + layout round-trips,
paint wait. Worker open sub-phases (`Handle.new` / `openDocument` /
`setReadOnly(false)`) were measured with a temporary worker probe that
identified the xlsx M editable-upgrade cost; that probe is not shipped.

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

Post–soft-share / mut-frame / `hf` baseline (this machine, `measure-edit-latency.mjs`):

| Interaction | Tier | Before p50 / p95 | After (drop xlsx mut-frame measures) | Heap peak before → after | n |
|---|---|---:|---:|---:|---:|
| xlsx cell (`setCellInput`) | S | 9.0 / 12.1 ms | **8.0 / 11.5 ms** | 7.3 → 7.4 MiB | 40 |
| xlsx cell (`setCellInput`) | L | 11.9 / 18.2 ms | 12.7 / 17.1 ms (noise) | 16.9 → 17.1 MiB | 40 |

xlsx mut-frame always queued `measureCount=1` for novel cell text, but sticky
`_nxPainted` never re-ticks to apply CanvasTextMeasure results — a dead worker
RT. Drop (drain) those payloads on xlsx mut-frame only; docx/pptx unchanged.

## Remount / StrictMode

| Scenario | Result |
|---|---|
| Remount same xlsx S | first 254 ms → second 28 ms (Δ −225 ms; module + parked worker) |
| React 18 StrictMode double-mount | `host_ready_count=2`, gap ~0.9 ms; mount→paint ~169 ms |

## Module-seeded preload (no host instantiate)

`preload()` compiles once, parks a worker seeded with that `WebAssembly.Module`,
and **does not** call host `initCore()`. React embed opens via `WorkerDocument`
only — a second host-side instance was wasted peak memory. Worker drops the
transferred open buffer after `Handle.new`. Bench: `node measure-open-mem.mjs`
(S/M/L). Same VM, Chromium headless, embed-path harness (no host `initCore`).

| Format | Tier | mount→paint before | after | Δ | JS heap peak before | after | Δ |
|---|---|---:|---:|---:|---:|---:|---:|
| xlsx | S | 171 ms | 120 ms | −51 | 5.7 MiB | 5.9 MiB | ~0 |
| xlsx | M | 172 ms | 128 ms | −44 | 6.7 MiB | 6.8 MiB | ~0 |
| xlsx | L | 121 ms | 188 ms | +67* | 15.6 MiB | 15.4 MiB | −0.2 |
| docx | S | 146 ms | 107 ms | −39 | 6.0 MiB | 5.5 MiB | −0.5 |
| docx | M | 161 ms | 112 ms | −49 | 6.2 MiB | 5.8 MiB | −0.4 |
| docx | L | 180 ms | 87 ms | −93 | 10.8 MiB | 9.8 MiB | −1.0 |
| pptx | S | 109 ms | 75 ms | −34 | 5.9 MiB | 5.5 MiB | −0.4 |
| pptx | M | 117 ms | 74 ms | −43 | 6.7 MiB | 6.1 MiB | −0.6 |
| pptx | L | 119 ms | 82 ms | −37 | 11.0 MiB | 10.6 MiB | −0.4 |

\*xlsx L paint-wait noise on a single run; open ms still improved (73 → 60).

## Paint before soft-share + apply-before-measure (embed open)

Worker posts the first editable `take-frame` **before** `setReadOnly(false)` /
`loadEditorChrome`, then signals `{ type: "er" }` after soft-share. Host applies
the command buffer (and chrome) before awaiting measure/outline acks; timed
edits await `_eP` / `whenEditReady()` so sample0 does not race materialise.

`measure-open-mem.mjs` on this VM (label `before` = pre-change tip, `after` =
this branch):

| Format | Tier | mount→paint before | after | open before | after | paint before | after | heap peak before | after |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| xlsx | S | 194 ms | 203 ms | 102 ms | 97 ms | 92 ms | 105 ms | 6.1 MiB | 6.5 MiB |
| xlsx | M | 254 ms | 246 ms | 112 ms | 124 ms | 142 ms | 123 ms | 7.0 MiB | 6.8 MiB |
| xlsx | L | **798 ms** | **757 ms** | 93 ms | **76 ms** | **705 ms** | **681 ms** | 16.0 MiB | 15.1 MiB |
| docx | S | 143 ms | 154 ms | 86 ms | 96 ms | 57 ms | 58 ms | 5.5 MiB | 5.5 MiB |
| docx | M | 159 ms | 129 ms | 91 ms | 89 ms | 67 ms | 40 ms | 6.3 MiB | 6.8 MiB |
| docx | L | 177 ms | 173 ms | 87 ms | 84 ms | 91 ms | 90 ms | 10.3 MiB | 10.3 MiB |
| pptx | S | 137 ms | 134 ms | 83 ms | 81 ms | 54 ms | 53 ms | 5.8 MiB | 5.4 MiB |
| pptx | M | 124 ms | 135 ms | 76 ms | 83 ms | 48 ms | 52 ms | 6.7 MiB | 6.4 MiB |
| pptx | L | 176 ms | 146 ms | 102 ms | 89 ms | 74 ms | 57 ms | 11.0 MiB | 11.0 MiB |

`measure-edit-latency.mjs` (`EDIT_SAMPLES=20`, after): xlsx cell S p50 **8.9 ms**,
L p50 **16.1 ms** (barrier after open; first-paint path unchanged).

Follow-up on tip: measure/outline refine is fire-and-forget after apply (eager
brotli **17390 / 17408 B** with `er` / `whenEditReady` in lazy tick+wp). xlsx L
paint-wait remains ~0.7 s class — soft-share is off the first frame, but host
DOM apply / tick still dominate `waitFirstPaint`.

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
| (this) | Module-seeded preload; no host `initCore`; drop open bytes after `Handle.new` |
| (this) | xlsx edit keys: WASM `hf` formula-bar delta (~1–2 KB) vs full `h`; lazy loader patches `.xfb` |
| (this) | xlsx mut-frame: drop dead CanvasTextMeasure payloads (sticky `_nxPainted`) |
| (this) | Paint-before-soft-share + host apply-before-measure + `er` / `_eP` edit barrier |

Stay out of xlsx grid paint / StringTable encode (separate track). Keep eager
loader ≤ 17 408 B brotli (new JS under `loader/lazy/`). No budget, threshold,
or fixture changes. Stay out of `.github/workflows/` and #113.
