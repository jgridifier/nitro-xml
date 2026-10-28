// SPDX-License-Identifier: MIT
// Shared page helpers for the perf hosts (#6). Times are ms since navigation start
// (performance.now() origin), so cold runs include bundle fetch, parse and WASM compile.

export const params = new URLSearchParams(location.search);
export const engine = params.get('engine') === 'wasm' ? 'wasm' : 'ts';
export const fixtureId = params.get('fixture');
export const marks = {};

import {
  installWorkerProbe, requestWasmMemory, longTaskSummary, wasmSummary, timeToInteractive, stepSummary,
} from './instrument.js';
export { timedStep, yieldTask } from './instrument.js';

// The wasm engine runs in the loader's worker: wrap Worker before the loader is imported so the probe
// can read its linear memory (#7).
if (engine === 'wasm') installWorkerProbe();

// Issue #7 mark names, as aliases of the #6 ones: bytes-ready = fixture_ready, first-render = first_paint
// (all_slides is pptx full-render). WASM linear memory is requested at parse_end and full_render.
const ALIAS = { fixture_ready: 'bytes_ready', first_paint: 'first_render', all_slides: 'full_render' };
const memory = {};

export const mark = (name) => {
  marks[name] = performance.now();
  if (ALIAS[name]) marks[ALIAS[name]] = marks[name];
  performance.mark(`perf:${name}`);
  if (engine === 'wasm' && (name === 'parse_end' || name === 'full_render' || name === 'all_slides')) {
    memory[ALIAS[name] ?? name] = requestWasmMemory();
  }
};

export const waitFrames = (n = 2) =>
  new Promise((resolve) => {
    const step = () => (--n <= 0 ? resolve() : requestAnimationFrame(step));
    requestAnimationFrame(step);
  });

/** querySelector through open shadow roots. */
export function deepFind(root, selector) {
  const hit = root.querySelector?.(selector);
  if (hit) return hit;
  for (const el of root.querySelectorAll?.('*') ?? []) {
    if (el.shadowRoot) {
      const inner = deepFind(el.shadowRoot, selector);
      if (inner) return inner;
    }
  }
  if (root.shadowRoot) return deepFind(root.shadowRoot, selector);
  return null;
}

/** querySelectorAll through open shadow roots. */
export function deepFindAll(root, selector, out = []) {
  out.push(...(root.querySelectorAll?.(selector) ?? []));
  for (const el of root.querySelectorAll?.('*') ?? []) if (el.shadowRoot) deepFindAll(el.shadowRoot, selector, out);
  if (root.shadowRoot) deepFindAll(root.shadowRoot, selector, out);
  return out;
}

/** Resolve once `probe()` returns truthy; rejects after `limit` frames so a broken host fails loudly. */
export async function until(probe, what, limit = 600) {
  for (let i = 0; i < limit; i++) {
    const v = await probe();
    if (v) return v;
    await waitFrames(1);
  }
  throw new Error(`perf host: timed out waiting for ${what}`);
}

export async function loadFixture(expectFormat) {
  if (!fixtureId) throw new Error('perf host: ?fixture=<id> is required');
  const { fixtures } = await (await fetch('/perf/fixtures.json')).json();
  const meta = fixtures.find((f) => f.id === fixtureId);
  if (!meta || meta.format !== expectFormat) throw new Error(`perf host: unknown ${expectFormat} fixture "${fixtureId}"`);
  const url = `/${meta.path}`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`perf host: fixture ${url}: HTTP ${res.status}`);
  const bytes = new Uint8Array(await res.arrayBuffer());
  mark('fixture_ready');
  return { meta, url, bytes, name: meta.path.split('/').pop() };
}

/** Publish for the runner: window.__perf.done flips true; `error` is set instead of `marks` on failure. */
export async function run(expectFormat, body) {
  window.__perf = { done: false, engine, fixture: fixtureId, format: expectFormat };
  try {
    const detail = await body();
    if (marks.full_render === undefined) mark('full_render'); // single-target hosts: nothing more to lay out
    const lt = await longTaskSummary();
    const steps = detail.steps ? stepSummary(detail.steps) : null;
    const wasm = engine === 'wasm' ? {
      linear_memory_parse_end: await memory.parse_end,
      linear_memory_full_render: await memory.full_render,
      ...wasmSummary(),
    } : {};
    const { steps: _s, ...rest } = detail;
    window.__perf = {
      ...window.__perf, done: true, marks, detail: rest,
      extra: {
        long_task_count: lt.count, long_task_total_ms: lt.total_ms, tbt_ms: lt.tbt_ms,
        tti_ms: timeToInteractive(marks.first_render, marks.full_render, lt),
        ...(steps ?? {}), ...wasm,
      },
    };
  } catch (e) {
    window.__perf = { ...window.__perf, done: true, error: String(e?.stack || e).slice(0, 800) };
  }
}
