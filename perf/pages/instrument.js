// SPDX-License-Identifier: MIT
// Metric instrumentation for the perf hosts (#7): long tasks, wasm timings and linear memory, resource
// timing, and the scroll / page-jump / slide-change step latency. Same code for both engines.
// Times are ms since navigation start (performance.now()). Nothing here ships.

import { installWasmTiming, wasmTimes as pageWasmTimes } from './worker-probe.js';

const TASK_MS = 50;
const longTasks = [];
try {
  new PerformanceObserver((list) => longTasks.push(...list.getEntries().map((e) => ({ start: e.startTime, duration: e.duration })))).observe({
    type: 'longtask',
    buffered: true,
  });
} catch {}

export const yieldTask = () =>
  new Promise((resolve) => {
    const ch = new MessageChannel();
    ch.port1.onmessage = () => {
      ch.port1.close();
      resolve();
    };
    ch.port2.postMessage(0);
  });

/** Long tasks seen so far: count, total duration, and total blocking time (time over 50 ms). */
export async function longTaskSummary() {
  await yieldTask(); // let queued observer callbacks deliver
  const entries = [...longTasks];
  return {
    count: entries.length,
    total_ms: entries.reduce((s, t) => s + t.duration, 0),
    tbt_ms: entries.reduce((s, t) => s + Math.max(0, t.duration - TASK_MS), 0),
    last_end_ms: entries.length ? Math.max(...entries.map((t) => t.start + t.duration)) : null,
    entries,
  };
}

// ---- wasm: linear memory and compile timing (engine=wasm) ---------------------------------------
const workers = [];
let probeId = 0;
const bootstrapUrl = () => new URL('/packages/nitro-xml/loader/bootstrap.js', location.href).href;

/** Wrap Worker so each loader worker starts with worker-probe.js. Call before importing the loader. */
export function installWorkerProbe() {
  installWasmTiming();
  const Native = window.Worker;
  const probe = new URL('./worker-probe.js', import.meta.url).href;
  window.Worker = class PerfWorker extends Native {
    constructor(url, opts) {
      const target = new URL(url, location.href).href;
      const src = `import ${JSON.stringify(probe)};\nimport ${JSON.stringify(target)};\n`;
      super(URL.createObjectURL(new Blob([src], { type: 'text/javascript' })), opts);
      this.__perf = { pending: new Map(), wasmTimes: [], wasmResources: [] };
      this.addEventListener('message', (e) => {
        if (e.data?.type !== 'perf:memory') return;
        e.stopImmediatePropagation();
        this.__perf.wasmTimes = e.data.wasmTimes;
        this.__perf.wasmResources = e.data.wasmResources;
        this.__perf.pending.get(e.data.id)?.(e.data.bytes);
      });
      workers.push(this);
    }
  };
}

/** Ask every loader worker for wasm linear memory now; resolve later (null when unavailable). */
export function requestWasmMemory() {
  const asks = workers.map(
    (w) =>
      new Promise((resolve) => {
        const id = ++probeId;
        w.__perf.pending.set(id, resolve);
        w.postMessage({ type: 'perf:memory', id, bootstrap: bootstrapUrl() });
        setTimeout(() => resolve(null), 10000);
      }),
  );
  return Promise.all(asks).then((all) => {
    const v = all.filter((x) => typeof x === 'number');
    return v.length ? Math.max(...v) : null;
  });
}

function wasmTimingSummary() {
  const origin = performance.timeOrigin;
  const all = [...pageWasmTimes, ...workers.flatMap((w) => w.__perf.wasmTimes)];
  if (!all.length) return null;
  const compile = all.filter((t) => /compile|instantiate/.test(t.api));
  return {
    // wall time of the compile/instantiate calls (streamed from the network), summed, and when the last finished
    wasm_instantiate_ms: compile.reduce((s, t) => s + (t.end - t.start), 0),
    wasm_ready_ms: Math.max(...compile.map((t) => t.end)) - origin,
    apis: [...new Set(all.map((t) => t.api))],
  };
}

function wasmResourceSummary() {
  const r = [...performance.getEntriesByType('resource'), ...workers.flatMap((w) => w.__perf.wasmResources)].filter((e) => e.name.endsWith('.wasm'));
  if (!r.length) return null;
  const e = r[r.length - 1];
  return { wasm_fetch_ms: e.responseEnd - e.startTime, wasm_transfer_bytes: e.transferSize, wasm_encoded_bytes: e.encodedBodySize, wasm_from_cache: e.transferSize === 0 && e.decodedBodySize > 0 ? 1 : 0 };
}

export const wasmSummary = () => ({ ...wasmTimingSummary(), ...wasmResourceSummary() });

// ---- step latency (scroll, page jump, slide change) ---------------------------------------------
/**
 * Time one view query. `sig()` returns a string that changes when the view has updated. Latency runs
 * from before `action()` until the view changed and two animation frames later (painted), so it
 * includes the engine's work and the paint; the poll is a message-channel task, not a frame.
 * Returns null when the view did not change within `timeout` ms (the engine does not support the step).
 */
export async function timedStep(action, sig, { timeout = 3000 } = {}) {
  const before = sig();
  const t0 = performance.now();
  await action();
  while (sig() === before) {
    if (performance.now() - t0 > timeout) return null;
    await yieldTask();
  }
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  return performance.now() - t0;
}

const sortedNums = (xs) => [...xs].sort((a, b) => a - b);
const q = (xs, p) => {
  const a = sortedNums(xs);
  const i = (a.length - 1) * p;
  const lo = Math.floor(i);
  return a[lo] + (a[Math.ceil(i)] - a[lo]) * (i - lo);
};

/** Reduce step latencies to the per-run median and p95. `unsupported` counts steps with no view change. */
export function stepSummary(steps) {
  const ok = steps.filter((x) => typeof x === 'number');
  return {
    step_count: ok.length,
    step_unsupported: steps.length - ok.length,
    step_median_ms: ok.length ? q(ok, 0.5) : null,
    step_p95_ms: ok.length ? q(ok, 0.95) : null,
    step_latencies_ms: ok,
  };
}

/** Time-to-interactive: first render, or the end of the last long task up to full render if later. */
export function timeToInteractive(firstRender, fullRender, lt) {
  const ends = lt.entries.map((t) => t.start + t.duration).filter((e) => e > firstRender && e <= fullRender + 100);
  return Math.max(firstRender, ...ends);
}
