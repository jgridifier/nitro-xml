// SPDX-License-Identifier: MIT
/**
 * WASM open baseline: time-to-openDocument + wasm linear memory growth.
 * Requires packages/nitro-xml/pkg from scripts/build-wasm.sh.
 *
 *   node perf/large/wasm-baseline.mjs fixtures/generated/xlsx/cells_1m.xlsx
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
  console.error("usage: node perf/large/wasm-baseline.mjs <fixture> [runs]");
  process.exit(1);
}

const { initSync, Handle } = await import(pathToFileURL(PKG).href);
initSync({ module: readFileSync(WASM) });

const file = readFileSync(path);
const name = path.split("/").pop();
const mem0 = () => {
  // wasm-bindgen keeps memory on the module; re-read via a throwaway Handle field if needed
  return undefined;
};

// After init, grab memory from a temporary export path
const glue = await import(pathToFileURL(PKG).href);
// nitro_xml_wasm.js does not re-export memory; use WebAssembly.Memory via Handle internals is hard.
// Measure by constructing Handle and reading process RSS + open timing; for linear memory use
// the fact that wasm-bindgen stores `wasm.memory` privately — instead report open ms only and
// note file is copied into Handle.bytes (JS ArrayBuffer + Rust Vec).

function median(xs) {
  const a = [...xs].sort((x, y) => x - y);
  return a[Math.floor(a.length / 2)];
}

const openMs = [];
const scrollMs = [];
for (let i = 0; i < runs + 1; i++) {
  const bytes = new Uint8Array(file); // intentional copy to mirror host→wasm boundary
  const t0 = performance.now();
  const h = new Handle(bytes);
  h.openDocument(name);
  const t1 = performance.now();
  // scroll proxy: setActiveSheet same sheet or prepareFrame
  if (typeof h.prepareFrame === "function") h.prepareFrame();
  const t2 = performance.now();
  if (i > 0) {
    openMs.push(t1 - t0);
    scrollMs.push(t2 - t1);
  }
  h.free();
}

console.log(
  JSON.stringify(
    {
      fixture: path,
      file_bytes: file.byteLength,
      runs,
      openDocument_ms_median: median(openMs),
      openDocument_ms_samples: openMs.map((x) => +x.toFixed(2)),
      prepareFrame_ms_median: median(scrollMs),
      note: "Each Handle::new copies bytes into Rust Vec; open_first_view copies again into FirstView.bytes (≥2× package in WASM linear memory plus the JS ArrayBuffer).",
    },
    null,
    2,
  ),
);
