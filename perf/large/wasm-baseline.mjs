// SPDX-License-Identifier: MIT
/**
 * WASM open baseline: time-to-openDocument + wasm linear memory + JS heap.
 * Requires packages/nitro-xml/pkg from scripts/build-wasm.sh.
 *
 *   node perf/large/wasm-baseline.mjs fixtures/generated/xlsx/cells_1m.xlsx
 *   node --expose-gc perf/large/wasm-baseline.mjs <fixture> [runs]
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { performance } from "node:perf_hooks";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");
const PKG = join(ROOT, "packages/nitro-xml/pkg/nitro_xml_wasm.js");
const WASM = join(ROOT, "packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm");
const path = process.argv[2];
const runs = Number(process.argv[3] || 5);
if (!path) {
  console.error("usage: node [--expose-gc] perf/large/wasm-baseline.mjs <fixture> [runs]");
  process.exit(1);
}

const mod = await import(pathToFileURL(PKG).href);
const exports = mod.initSync({ module: readFileSync(WASM) });
const { Handle } = mod;
const memory = exports.memory;
if (!memory) {
  console.error("initSync did not return WebAssembly.Memory");
  process.exit(1);
}

const file = readFileSync(path);
const name = path.split("/").pop();

function median(xs) {
  const a = [...xs].sort((x, y) => x - y);
  return a[Math.floor(a.length / 2)];
}

function wasmBytes() {
  return memory.buffer.byteLength;
}

function jsHeap() {
  if (typeof globalThis.gc === "function") globalThis.gc();
  const u = process.memoryUsage();
  return {
    heapUsed: u.heapUsed,
    external: u.external,
    arrayBuffers: u.arrayBuffers ?? 0,
    rss: u.rss,
  };
}

/** Drive deferred SeekTail / sheet shell until `handle.t` (full load for xlsx). */
function drainTail(h, maxTicks = 64) {
  let ticks = 0;
  while (h.t === false && ticks < maxTicks) {
    h.prepareFrame();
    ticks++;
  }
  return ticks;
}

/** Scroll proxy: wheel-ish prepareFrame ticks after the sheet is complete. */
function scrollTicks(h, n = 8) {
  const t0 = performance.now();
  for (let i = 0; i < n; i++) h.prepareFrame();
  return performance.now() - t0;
}

// Residency probe: N live Handles (construct only, no free) — catches a leaked
// pass-buffer copy that stayed resident alongside Arc package bytes.
const residencyN = Math.max(2, Math.min(6, runs));
const residencyBefore = wasmBytes();
const live = [];
for (let i = 0; i < residencyN; i++) {
  live.push(new Handle(new Uint8Array(file)));
}
const residencyAfter = wasmBytes();
const wasmPerLiveHandle = Math.round((residencyAfter - residencyBefore) / residencyN);
for (const h of live) h.free();

const openMs = [];
const scrollMs = [];
const fullMs = [];
const wasmOpenAbs = [];
const wasmFullAbs = [];
const jsOpenAbs = [];
const jsFullAbs = [];
let peakWasm = wasmBytes();
let peakJsHeap = jsHeap().heapUsed;

// Cold first open in a fresh growth window (no prior free freelist reuse).
{
  const js0 = jsHeap();
  const wasm0 = wasmBytes();
  const h = new Handle(new Uint8Array(file));
  const wasmNew = wasmBytes();
  const t0 = performance.now();
  h.openDocument(name);
  const tOpen = performance.now();
  drainTail(h);
  const tFull = performance.now();
  const scroll = scrollTicks(h);
  openMs.push(tOpen - t0);
  fullMs.push(tFull - tOpen);
  scrollMs.push(scroll);
  wasmOpenAbs.push(wasmNew - wasm0);
  wasmFullAbs.push(wasmBytes() - wasm0);
  jsOpenAbs.push(jsHeap().heapUsed - js0.heapUsed);
  jsFullAbs.push(jsHeap().heapUsed - js0.heapUsed);
  peakWasm = Math.max(peakWasm, wasmBytes());
  peakJsHeap = Math.max(peakJsHeap, jsHeap().heapUsed);
  h.free();
}

for (let i = 1; i < runs; i++) {
  const js0 = jsHeap();
  const wasm0 = wasmBytes();
  const bytes = new Uint8Array(file); // host→boundary copy (mirrors non-transfer path)
  const t0 = performance.now();
  const h = new Handle(bytes);
  h.openDocument(name);
  const tOpen = performance.now();
  const wasmOpen = wasmBytes();
  const jsOpen = jsHeap();

  const tFull0 = performance.now();
  drainTail(h);
  const tFull = performance.now();
  const wasmFull = wasmBytes();
  const jsFull = jsHeap();

  const scroll = scrollTicks(h);

  openMs.push(tOpen - t0);
  fullMs.push(tFull - tFull0);
  scrollMs.push(scroll);
  wasmOpenAbs.push(wasmOpen - wasm0);
  wasmFullAbs.push(wasmFull - wasm0);
  jsOpenAbs.push(jsOpen.heapUsed - js0.heapUsed);
  jsFullAbs.push(jsFull.heapUsed - js0.heapUsed);
  peakWasm = Math.max(peakWasm, wasmFull);
  peakJsHeap = Math.max(peakJsHeap, jsFull.heapUsed);

  h.free();
}

console.log(
  JSON.stringify(
    {
      fixture: path,
      file_bytes: file.byteLength,
      runs,
      openDocument_ms_median: +median(openMs).toFixed(2),
      openDocument_ms_samples: openMs.map((x) => +x.toFixed(2)),
      full_load_ms_median: +median(fullMs).toFixed(2),
      scroll_prepareFrame_ms_median: +median(scrollMs).toFixed(2),
      wasm_per_live_handle_bytes: wasmPerLiveHandle,
      wasm_per_live_handle_over_file: +(wasmPerLiveHandle / file.byteLength).toFixed(2),
      wasm_linear_delta_open_samples: wasmOpenAbs,
      wasm_linear_delta_full_samples: wasmFullAbs,
      wasm_linear_peak_bytes: peakWasm,
      js_heap_delta_open_median: median(jsOpenAbs),
      js_heap_delta_full_median: median(jsFullAbs),
      js_heap_peak_bytes: peakJsHeap,
      note: "Handle::new(Vec<u8>) takes ownership of the wasm-bindgen pass buffer (one host→WASM copy), then Arc-shares with FirstView. Worker path transfers ArrayBuffer then drops the JS view after Handle construction. wasm_per_live_handle_over_file ~2 = leaked pass buffer + Arc; ~1 = single residency.",
    },
    null,
    2,
  ),
);
