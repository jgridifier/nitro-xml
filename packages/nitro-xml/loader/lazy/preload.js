// SPDX-License-Identifier: MIT
// Host preload (lazy chunk — not in the first-load brotli budget).
import {
  compileWasmModule,
  defaultWasmUrl,
  prewarmWorker,
  unsupportedBrowserError,
  wasmSupported,
} from "../bootstrap.js";

/**
 * Compile+cache wasm and prewarm the document worker before open.
 * Call from a React route layout / app bootstrap:
 *   await import("nitro-xml/loader/lazy/preload.js").then((m) => m.preload());
 *
 * Does not call initCore() on the host: React embed opens via WorkerDocument
 * only, and a host-side instantiate doubled peak JS/wasm memory. When a
 * Module is ready it is posted to the parked worker (best-effort seed so the
 * worker can skip a second compileStreaming). Never terminate a parked worker
 * — WorkerDocument may already own it.
 */
export async function preload(opts = {}) {
  if (!wasmSupported()) throw unsupportedBrowserError();
  const mod = await compileWasmModule(
    opts.module || opts.wasmUrl || defaultWasmUrl(),
  ).catch(() => null);
  const w = prewarmWorker(opts.workerUrl);
  if (mod && w) {
    try {
      w.postMessage({ type: "prewarm", module: mod });
    } catch {}
  }
  return mod;
}
