// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { compileWasmModule, wasmSupported } from "../bootstrap.js";

describe("bootstrap wasm module cache", () => {
  it("reuses an in-memory Module across compileWasmModule calls", async () => {
    // Catches: remount re-compiling wasm when a Module was already cached (React embed).
    if (!wasmSupported()) return;
    const bytes = new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
    const mod = await WebAssembly.compile(bytes);
    const once = await compileWasmModule(mod);
    assert.equal(once, mod);
    const again = await compileWasmModule(new Uint8Array(bytes));
    assert.equal(again, mod);
  });

  it("cold init prefers Response over awaiting Module before glue.default", async () => {
    // Catches: #307 setting input=cachedModule after await compileWasmModule so
    // wasm-bindgen skips instantiateStreaming on the first open (React embed).
    const src = await import("node:fs").then((fs) =>
      fs.readFileSync(new URL("../bootstrap.js", import.meta.url), "utf8"),
    );
    assert.match(src, /void compileWasmModule\(res\.clone\(\)\)/);
    assert.match(src, /input = res;/);
    assert.doesNotMatch(src, /input = cachedModule \|\| res/);
  });

  it("preload compiles and seeds the worker without host initCore", async () => {
    // Catches: preload() calling initCore() on the host and doubling peak wasm
    // memory for WorkerDocument-only React embeds.
    const fs = await import("node:fs");
    const preloadSrc = fs.readFileSync(
      new URL("../lazy/preload.js", import.meta.url),
      "utf8",
    );
    const preloadCode = preloadSrc.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");
    assert.match(preloadCode, /compileWasmModule/);
    assert.match(preloadCode, /prewarmWorker/);
    assert.match(preloadCode, /type:\s*"prewarm",\s*module:/);
    assert.doesNotMatch(preloadCode, /\.terminate\s*\(/);
    assert.doesNotMatch(preloadCode, /\binitCore\s*\(/);
    assert.doesNotMatch(preloadCode, /\binitCore\b/);
    const workerSrc = fs.readFileSync(
      new URL("../worker.js", import.meta.url),
      "utf8",
    );
    assert.match(workerSrc, /compileWasmModule\(msg\.module\)/);
    assert.match(workerSrc, /bytes = null/);
    assert.match(workerSrc, /prewarmOpenPaths/);
    assert.match(workerSrc, /lazy\/prewarm-open\.js/);
  });
});
