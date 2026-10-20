// SPDX-License-Identifier: MIT
// Host preload (lazy chunk — not in the first-load brotli budget).
import {
  compileWasmModule,
  defaultWasmUrl,
  takePrewarmedWorker,
  unsupportedBrowserError,
  wasmSupported,
} from "../bootstrap.js";

/**
 * Compile+cache wasm and park a Module-seeded document worker before open.
 * Call from a React route layout / app bootstrap:
 *   await import("nitro-xml/loader/lazy/preload.js").then((m) => m.preload());
 *
 * Does not call initCore() on the host: React embed opens via WorkerDocument
 * only, and a host-side instantiate doubled peak JS/wasm memory. The compiled
 * Module is posted with the worker's first prewarm so it skips a second
 * compileStreaming.
 */
export async function preload(opts = {}) {
  if (!wasmSupported()) throw unsupportedBrowserError();
  const mod = await compileWasmModule(
    opts.module || opts.wasmUrl || defaultWasmUrl(),
  ).catch(() => null);
  if (typeof Worker === "undefined") return mod;
  // Replace any unseeded park so the Module arrives with the first prewarm.
  const stale = takePrewarmedWorker();
  if (stale) {
    try {
      stale.terminate();
    } catch {}
  }
  try {
    const url = opts.workerUrl || new URL("../worker.js", import.meta.url);
    const w = new Worker(url, { type: "module", name: "nitro-xml-prewarm" });
    if (mod) w.postMessage({ type: "prewarm", module: mod });
    else w.postMessage({ type: "prewarm" });
    takePrewarmedWorker(w);
  } catch {}
  return mod;
}
