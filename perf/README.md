<!-- SPDX-License-Identifier: MIT -->
# Perf harness (#6, #7): server, Playwright/CDP runner and metrics

Drives real Chromium against the fixed corpus with an engine switch: `ts` (the upstream TypeScript
engine, test-only baseline, built from the pinned `ChristopherVR/ooxml @ e66aaec`) and `wasm` (the Nitro
loader plus the Rust/WASM package). Everything runs on localhost; no file leaves the machine.
Nothing under `perf/` ships.

Single entrypoint: **`node perf/run.mjs`**. Other perf tools (`perf/large`, `perf/scoreboard`,
`perf/react-embed`, `perf/micro`) are separate, older measurements; `perf/runner` is the long-session
correctness harness (#90), not a timing runner.

## Pinned versions

| What | Version |
|---|---|
| Chromium | **156.0.8078.4** (Playwright's `chromium` channel, revision 1248, new headless mode) |
| Playwright | **playwright-core 1.64.0** (exact pin in `perf/package.json`) |
| Node | >= 22 per the root `package.json` (developed on 20.19; `run.mjs` has no 22-only syntax) |
| Upstream | `ChristopherVR/ooxml` @ `e66aaec24adc0b68472eae1fc42aa4598e09c352` (the `upstream/` submodule; `build-ts.mjs` refuses any other commit) |

The Chrome version actually used is printed on the first line of every run and stored in the result
JSON (`chrome`). Quote that number, not this table, when posting results. `--chrome /path/to/chrome`
runs a different build (for example `/usr/bin/google-chrome`); the report then carries that version.

## Run it locally

```bash
cd perf && npm ci                      # playwright-core 1.64.0
npx playwright-core install chromium   # once, fetches revision 1248 (network needed only here)
node build-ts.mjs                      # once: upstream submodule + bun install + TS bundles into perf/dist/ts/
bash ../scripts/build-wasm.sh          # only for --engine wasm (writes packages/nitro-xml/pkg)

node run.mjs                           # cold, ts, all fixtures that exist, 10 iterations
node run.mjs --engine ts,wasm --fixtures S --mode warm
node run.mjs --engine wasm --fixtures xlsx-M,pptx --cpu-throttle 4 --iterations 15
```

`build-ts.mjs` needs `git`, [`bun`](https://bun.sh) and network the first time only. It builds the
upstream JS without the `tsc` declaration passes (upstream's type-check fails on a clean checkout and
the harness needs only JS), and judges success by the emitted files.

### Flags

| Flag | Default | Meaning |
|---|---|---|
| `--engine ts\|wasm\|ts,wasm` | `ts` | which engine each host mounts |
| `--mode cold\|warm` | `cold` | see below |
| `--iterations N` | 10 cold, 20 warm | measured loads per fixture and engine (the warm-up is extra) |
| `--cpu-throttle 1\|4` | `1` | CDP `Emulation.setCPUThrottlingRate` |
| `--fixtures a,b,..` | all | ids from `fixtures.json` (`xlsx-S`), formats (`docx`) or tiers (`M`) |
| `--memory` | off | also report `performance.measureUserAgentSpecificMemory` after `full_render` (needs the COOP/COEP headers; takes up to 20 s per load) |
| `--out file.json` | `perf/results/<date>-<mode>-<engines>.json` | result file (`results/` is git-ignored) |
| `--chrome path` | Playwright Chromium | browser binary |
| `--timeout ms` | 120000 | per-load limit |

Missing fixtures (the L and XL tiers are generated on demand by `fixtures/gen`) are skipped with a
warning, never substituted. The process exits non-zero if any load fails.

### Modes

- **cold**: a new browser context per iteration, so no cache, storage or process state carries
  over. The HTTP cache is disabled twice (CDP `Network.setCacheDisabled` and `Cache-Control:
  no-store` from the server). V8 script code caches and compiled-WASM caches are stored through the
  HTTP cache, so they are off too. The runner also fails the run if any cold response is reported
  as served from cache.
- **warm**: one persistent context (temporary profile, disk-backed HTTP cache; the upstream
  bundles are 2 to 13 MiB and do not fit Chromium's in-memory cache). One discarded warm-up load, then
  N loads, each in a fresh page of that same context, with `Cache-Control: max-age=3600`.
  Chromium keeps its own code-cache heuristics, so a warm load is "a returning visitor", not a
  guarantee of a code-cache hit.

### Offline and isolation

The server binds `127.0.0.1` only, serves just `/perf/`, `/fixtures/` and `/packages/nitro-xml/`
(the upstream tree is never served), and sends `Cross-Origin-Opener-Policy: same-origin`,
`Cross-Origin-Embedder-Policy: require-corp` and `Cross-Origin-Resource-Policy: same-origin`.
Chromium runs with `--host-resolver-rules="MAP * ~NOTFOUND, EXCLUDE 127.0.0.1"`, so any other host
fails to resolve; failures are counted as `blockedRequests` in the result file and warned about.

To browse a host by hand: `node perf/server.mjs` then open
`http://127.0.0.1:8123/perf/pages/xlsx.html?engine=ts&fixture=xlsx-S` and read `window.__perf`.

## What is measured (render targets)

All times are milliseconds since navigation start of the host page, 1280x800 viewport, DPR 1, so
they include the bundle (or loader plus WASM) fetch, parse and compile. A load is "done" two animation
frames after the target is in the DOM.

| Format | Target | Marks reported |
|---|---|---|
| **xlsx** | First viewport: first grid cells in the DOM of the open workbook (active sheet, top-left window). | `first_paint` |
| **docx** | **Print Layout**: the view is switched to Print Layout and the first page sheet is in the DOM. The result also reports `pages` (sheets rendered). | `first_paint` |
| **pptx** | **Slide 1** is painted (`first_paint`), then every slide is shown in order, one frame apart (`all_slides`). | `first_paint`, `all_slides` |

Derived columns in the JSON (`stats.<name>`: n, min, median, p95, max, mean):

- `engine_ms`: navigation start until the engine is ready (bundle imported, or loader imported).
- `fetch_ms`: fixture download from localhost.
- `render_ms`: from the moment the element is mounted and the load begins until `first_paint`.
- `first_paint_ms`: the whole thing. `all_slides_ms`, `all_slides_render_ms`: pptx only.
- `JSHeapUsedSize`, `JSHeapTotalSize`: CDP `Performance.getMetrics` after the target. This is the page
  JS heap only; WASM linear memory and the WASM engine's worker are not in it. Use `--memory`
  (`uamem_bytes`) for a page-wide figure.

### Public APIs only

| Format | `engine=ts` (upstream, via its published builds) | `engine=wasm` |
|---|---|---|
| xlsx | `<xlsx-editor>` from `ooxml-ui/xlsx` (`defineXlsxEditor()`, then `load(bytes, name)`), the xlsx grid | `<nitro-xlsx>` (`defineNitroXlsx()`, `loadFile`) |
| docx | `<docx-editor>` from `ooxml-ui/docx` (`registerDocxEditor()`, `load(bytes)`), Print Layout selected through the status bar's `office-status-activate` event (`{id: 'print'}`), the event its Print Layout button emits; there is no attribute or method for the view | `<nitro-docx>` (`defineNitroDocx()`, `loadFile`) |
| pptx | `createPptxViewer(host, { source, onLoad })` from `pptx-vanilla-viewer`, then `goToSlide(i)` | `<nitro-pptx>` (`defineNitroPptx()`, `loadFile`), slides advanced with the keyboard event path |

Note on `createRenderController`: it exists at
`viewers/pptx/packages/vanilla/src/viewer/render-controller.ts` but is **not** exported from the
package entry. `createPptxViewer` constructs it internally (`PptxViewer.ts`), so the pptx host measures
it through the public entry instead of importing the internal module.

`TODO(#7)`: the `wasm` pptx host takes the slide count from `slides` in `fixtures.json` because the
loader exposes none; read it from the loader once it does. Memory-per-document gates, history in
`perf-data` and regression thresholds are `#8`; XL-tier runs and the other stages' gates are not part of
this harness.

## Metrics (#7)

Every load reports the same metrics for both engines. `run.mjs` writes the summary JSON (`--out`) and,
next to it, a `.jsonl` with **one object per (fixture, engine, mode, iteration)**; `aggregate.mjs` turns
that into median, p95 and a bootstrap CI (see below).

| Metric (JSON key) | What it is |
|---|---|
| `bytes_ready_ms`, `parse_end_ms`, `first_render_ms`, `full_render_ms` | The marks, ms since navigation start: `bytes-ready` (fixture bytes in memory) -> `parse-end` (the engine's load/open promise resolved) -> `first-render` (first page, slide or viewport in the DOM, then a double `requestAnimationFrame`) -> `full-render`. `parse_ms` = `parse_end - bytes_ready`. `first_render` is the #6 `first_paint`; `all_slides` is pptx `full_render`. |
| full-render target | xlsx: first viewport, then a scripted scroll in 600 px steps until the last used row (`lastRow` in `fixtures.json`) is on screen. docx: page jumps through the document to the last page (at most 60, evenly spread). pptx: every slide in order. |
| `long_task_count`, `long_task_total_ms`, `tbt_ms` | `PerformanceObserver({type:'longtask', buffered:true})` over the whole load until `full_render`: count, summed duration, and total blocking time (sum of time over 50 ms per task). |
| `tti_ms` | Time to interactive, simplified: `first_render`, or the end of the last long task between `first_render` and `full_render` if later. TODO(#8): a quiet-window definition once thresholds are set. |
| `peak_heap_used_bytes` | CDP `Runtime.getHeapUsage` every 10 ms from before navigation until done; the max `usedSize`. `heap_samples` counts the samples. |
| `retained_heap_used_bytes` | `HeapProfiler.collectGarbage`, then `Runtime.getHeapUsage` after the run. |
| `uamem_bytes` | `performance.measureUserAgentSpecificMemory()` after `full_render` (cross-origin isolated page; only with `--memory`, up to 20 s per load). `null` where unavailable. |
| `linear_memory_parse_end`, `linear_memory_full_render` | wasm only: `memory.buffer.byteLength` of the `memory` export of nitro-xml-wasm, read in the loader's worker at `parse_end` and at `full_render`. Linear memory never shrinks, so the last value is the peak. See "How wasm memory is read". |
| `wasm_fetch_ms`, `wasm_transfer_bytes`, `wasm_from_cache`, `wasm_instantiate_ms`, `wasm_ready_ms` | wasm only: the `.wasm` resource timing (`transferSize` 0 means cached), the summed wall time of `WebAssembly.compileStreaming`/`instantiateStreaming`/`instantiate` (the streamed fetch is inside it), and when the last one finished. Cold runs have no HTTP or code cache; warm runs reuse the cached module. These are all inside `first_render_ms`, which counts from navigation start. |
| `step_count`, `step_median_ms`, `step_p95_ms`, `step_unsupported`; JSONL `step_latencies_ms` | Scroll / page-jump / slide-change latency per step: from before the action until the view changed (a `MessageChannel` poll, not a frame) plus a double `requestAnimationFrame`, so it includes the engine's work and the paint. xlsx: one 600 px scroll (ts: `.xg-scroller.scrollTop`; wasm: a wheel event). docx: jump to page N (ts: `scrollIntoView`; wasm: the PageDown key). pptx: next slide (ts: `goToSlide`; wasm: ArrowRight). The view change is detected by row headers (xlsx), scroll position (docx ts), or text (the others). |
| `sizes` (summary JSON), `node perf/sizes.mjs` | Raw, gzip (9) and brotli (11) of the ts bundles in `dist/ts/`, and for wasm the `.wasm`, the wasm-bindgen glue and the loader JS. |

Known gaps, recorded rather than hidden:

- **wasm docx has no page navigation yet** (it renders page 1 only). The page-jump step times out on the
  first jump, so `step_*` is empty for it and `full_render` equals `first_render`; `TODO(#33)` in `docx.html`.
- **#6 correction:** the `wasm` pptx host sent `key:` strings, which the event codec reads as key id 0, so it
  never advanced a slide and `all_slides` measured no slide changes. It now sends `KeyId.ArrowRight`
  and fails if a slide does not render. The wasm `all_slides` numbers are therefore higher than in #6 runs.
- docx/xlsx `full_render` for the L and XL tiers needs `lastRow` or `pages` in `fixtures.json`, or the
  scroll runs until the view stops changing.

### How wasm memory is read

The engine lives in the loader's worker, so the page cannot see its `memory`. For `engine=wasm` the host
replaces `window.Worker` (before the loader is imported) with a subclass that starts each worker through a
blob wrapper: `import "pages/worker-probe.js"; import "<loader worker.js>";`. The probe registers a
`message` listener before the loader's `onmessage`, answers `{type:'perf:memory'}` with
`bootstrap.getWasmMemory().buffer.byteLength`, and stops that message from reaching the loader. The loader
and the wasm package are not modified, so the shipped loader budget is unaffected, and the probe only
exists on the perf pages.

### Aggregation and the reference dataset

```bash
node perf/aggregate.mjs perf/results/<run>.jsonl [--out summary.json]   # median, p95, bootstrap CI95 per metric
node perf/aggregate.mjs --check                                         # recompute the reference, compare
node --test perf/test/                                                  # same check as a test
```

`stats.mjs` has the median, p95 (linear interpolation) and a seeded percentile bootstrap (2000 resamples,
95 %), so the same JSONL always gives the same CI. `reference/aggregate-reference.jsonl` is a recorded
dataset with known answers in `reference/aggregate-reference.json`: median and p95 of 1..100 (50.5 and 95.05),
a constant series, and a five-point series, derived by hand; the CI values are the recorded output of the
seeded bootstrap.

## Files

| Path | Role |
|---|---|
| `run.mjs` | runner and the documented entrypoint (CDP collectors: heap sampler, retained heap, `uamem`) |
| `aggregate.mjs`, `stats.mjs`, `reference/`, `test/` | median / p95 / bootstrap CI over the JSONL, and its known-answer check |
| `sizes.mjs` | raw / gzip / brotli sizes per engine |
| `pages/instrument.js`, `pages/worker-probe.js` | long tasks, step latency, wasm memory and timing probes |
| `server.mjs` | COOP/COEP static server (`startServer()` is imported by the runner) |
| `build-ts.mjs` | builds the TS-engine bundles into `perf/dist/ts/` from the upstream pin |
| `hosts/ts-*.mjs` | bundle entries that re-export the upstream public entry points |
| `pages/{xlsx,docx,pptx}.html`, `pages/common.js` | hosts; `?engine=ts\|wasm&fixture=<id>` |
| `fixtures.json` | corpus ids, tiers, paths |
| `results/`, `dist/` | outputs (git-ignored) |

## Reading the numbers

- Compare engines only within one run, same mode, same throttle; the interleaving is per fixture, not
  per iteration, so do not run other heavy work on the machine meanwhile.
- The `ts` bundles are minified single files of the upstream builds, 2 to 13 MiB each. Most of the
  `ts` `engine_ms` is parse and evaluation of that file; this is real cost of the baseline and is why
  it is reported separately from `render_ms`.
- Gates and thresholds live in `perf/budgets.json` and `scripts/ci/perf*.sh`; this harness does not
  change or arm them.
