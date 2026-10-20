// SPDX-License-Identifier: MIT
/**
 * Page-side instrumentation for React embed latency.
 * Patches fetch / WebAssembly / Worker before the nitro loader is imported.
 */
import { installWorkerMessageTrace } from "./trace.js";

function now() {
  return performance.now();
}

const state = {
  t0: now(),
  wasm: {
    fetch_ms: null,
    compile_ms: null,
    instantiate_ms: null,
    streaming_compile: false,
    streaming_instantiate: false,
  },
  worker: {
    construct_ms: null,
    count: 0,
  },
  marks: [],
  errors: [],
  done: false,
  result: null,
};

function mark(name, extra = {}) {
  const entry = { name, t: now() - state.t0, ...extra };
  state.marks.push(entry);
  return entry;
}

function isWasmUrl(url) {
  const s = String(url || "");
  return s.includes(".wasm") || s.includes("nitro_xml_wasm");
}

function patchFetch() {
  if (typeof fetch !== "function") return;
  const orig = fetch.bind(globalThis);
  globalThis.fetch = async (input, init) => {
    const url = typeof input === "string" || input instanceof URL ? String(input) : input?.url;
    if (!isWasmUrl(url)) return orig(input, init);
    const t0 = now();
    const res = await orig(input, init);
    state.wasm.fetch_ms = now() - t0;
    mark("wasm_fetch", { ms: state.wasm.fetch_ms, url });
    return res;
  };
}

function patchWebAssembly() {
  const WA = globalThis.WebAssembly;
  if (!WA) return;

  if (typeof WA.compileStreaming === "function") {
    const orig = WA.compileStreaming.bind(WA);
    WA.compileStreaming = async (source) => {
      const t0 = now();
      const mod = await orig(source);
      state.wasm.compile_ms = now() - t0;
      state.wasm.streaming_compile = true;
      mark("wasm_compileStreaming", { ms: state.wasm.compile_ms });
      return mod;
    };
  }

  if (typeof WA.instantiateStreaming === "function") {
    const orig = WA.instantiateStreaming.bind(WA);
    WA.instantiateStreaming = async (source, imports) => {
      const t0 = now();
      const result = await orig(source, imports);
      state.wasm.instantiate_ms = now() - t0;
      state.wasm.streaming_instantiate = true;
      mark("wasm_instantiateStreaming", { ms: state.wasm.instantiate_ms });
      return result;
    };
  }

  if (typeof WA.compile === "function") {
    const orig = WA.compile.bind(WA);
    WA.compile = async (buf) => {
      const t0 = now();
      const mod = await orig(buf);
      if (state.wasm.compile_ms == null) {
        state.wasm.compile_ms = now() - t0;
        state.wasm.streaming_compile = false;
        mark("wasm_compile", { ms: state.wasm.compile_ms });
      }
      return mod;
    };
  }

  if (typeof WA.instantiate === "function") {
    const orig = WA.instantiate.bind(WA);
    WA.instantiate = async (source, imports) => {
      const t0 = now();
      const result = await orig(source, imports);
      if (state.wasm.instantiate_ms == null) {
        state.wasm.instantiate_ms = now() - t0;
        state.wasm.streaming_instantiate = false;
        mark("wasm_instantiate", { ms: state.wasm.instantiate_ms });
      }
      return result;
    };
  }
}

function patchWorker() {
  if (typeof Worker === "undefined") return;
  const Orig = Worker;
  function InstrumentedWorker(url, opts) {
    const t0 = now();
    const w = new Orig(url, opts);
    const ms = now() - t0;
    state.worker.count += 1;
    if (state.worker.construct_ms == null) state.worker.construct_ms = ms;
    mark("worker_construct", { ms, url: String(url), count: state.worker.count });
    return w;
  }
  InstrumentedWorker.prototype = Orig.prototype;
  Object.defineProperty(InstrumentedWorker, "name", { value: "Worker" });
  globalThis.Worker = InstrumentedWorker;
}

export function installInstrumentation() {
  if (globalThis.__nitroBenchInstalled) return state;
  globalThis.__nitroBenchInstalled = true;
  patchFetch();
  patchWebAssembly();
  patchWorker();
  installWorkerMessageTrace();
  globalThis.__nitroBench = {
    state,
    mark,
    finish(result) {
      state.result = result;
      state.done = true;
      mark("done");
    },
    fail(err) {
      state.errors.push(String(err && err.stack ? err.stack : err));
      state.done = true;
      state.result = { ok: false, error: String(err) };
    },
  };
  return state;
}

export { state as benchState };
