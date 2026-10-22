// SPDX-License-Identifier: MIT
/**
 * Public ESM entry for `nitro-xml`.
 *
 * Re-exports the loader (WASM is still lazy via `initCore` / `WorkerDocument`).
 * Prefer `import … from "nitro-xml/loader"` in apps that want the explicit path.
 * Direct wasm-bindgen glue stays at `nitro-xml/wasm` for harnesses.
 */
export * from "./loader/index.js";
