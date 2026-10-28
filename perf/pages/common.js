// SPDX-License-Identifier: MIT
// Shared page helpers for the perf hosts (#6). Times are ms since navigation start
// (performance.now() origin), so cold runs include bundle fetch, parse and WASM compile.

export const params = new URLSearchParams(location.search);
export const engine = params.get('engine') === 'wasm' ? 'wasm' : 'ts';
export const fixtureId = params.get('fixture');
export const marks = {};

export const mark = (name) => {
  marks[name] = performance.now();
  performance.mark(`perf:${name}`);
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
    window.__perf = { ...window.__perf, done: true, marks, detail };
  } catch (e) {
    window.__perf = { ...window.__perf, done: true, error: String(e?.stack || e).slice(0, 800) };
  }
}
