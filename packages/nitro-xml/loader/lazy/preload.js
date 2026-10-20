// SPDX-License-Identifier: MIT
// Host preload (lazy chunk — not in the first-load brotli budget).
import {
  compileWasmModule,
  defaultWasmUrl,
  initCore,
  prewarmWorker,
  unsupportedBrowserError,
  wasmSupported,
} from "../bootstrap.js";

/**
 * Start wasm compile+cache, worker spin-up, and initCore before the user opens
 * a file. Call from a React route layout / app bootstrap:
 *   await import("nitro-xml/loader/lazy/preload.js").then((m) => m.preload());
 */
export async function preload(opts = {}) {
  if (!wasmSupported()) throw unsupportedBrowserError();
  await compileWasmModule(opts.module || opts.wasmUrl || defaultWasmUrl()).catch(() => {});
  prewarmWorker(opts.workerUrl);
  return initCore(opts);
}
