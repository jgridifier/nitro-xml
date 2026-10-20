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
});
