// SPDX-License-Identifier: MIT
// Perf-only probe (#7), loaded in the page and, via a blob wrapper installed by instrument.js, as the
// first module of the loader's worker (wasm engine only). Nothing here ships.
//   1. Times WebAssembly.compileStreaming / instantiateStreaming / instantiate / compile (the wasm
//      fetch is streamed into compilation, so these durations include the network read).
//   2. In the worker: answers {type:'perf:memory'} with `memory.buffer.byteLength` of the loader's
//      wasm instance (bootstrap.getWasmMemory(); the `memory` export of nitro-xml-wasm) and the timings.
//      Also returns the worker's own resource timing for the .wasm (the fetch happens in the worker).
//      The listener is registered before the loader's `self.onmessage`, so it runs first and stops the
//      message from reaching the loader.

const times = [];
export const wasmTimes = times;

export function installWasmTiming() {
  if (self.__nitroWasmTiming) return;
  self.__nitroWasmTiming = true;
  for (const name of ['compileStreaming', 'instantiateStreaming', 'compile', 'instantiate']) {
    const native = WebAssembly[name];
    if (typeof native !== 'function') continue;
    WebAssembly[name] = function (...a) {
      const start = performance.timeOrigin + performance.now();
      const done = () => times.push({ api: name, start, end: performance.timeOrigin + performance.now() });
      const p = native.apply(this, a);
      p.then(done, done);
      return p;
    };
  }
}

installWasmTiming();

if (typeof WorkerGlobalScope !== 'undefined' && self instanceof WorkerGlobalScope) {
  self.addEventListener('message', async (e) => {
    if (e.data?.type !== 'perf:memory') return;
    e.stopImmediatePropagation();
    let bytes = null;
    try {
      const boot = await import(e.data.bootstrap);
      bytes = boot.getWasmMemory()?.buffer.byteLength ?? null;
    } catch {}
    const wasm = performance.getEntriesByType('resource').filter((r) => r.name.endsWith('.wasm')).map((r) => r.toJSON());
    self.postMessage({ type: 'perf:memory', id: e.data.id, bytes, wasmTimes: times, wasmResources: wasm });
  });
}
