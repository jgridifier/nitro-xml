// SPDX-License-Identifier: MIT
/** Batched measureText read-back. `takeMeasureReqs` returns SoA `[fonts,sizes,texts]` or object rows. */
function font(f, sz) {
  return /\d+(\.\d+)?px/.test(f) ? f : `${sz}px ${f}`;
}
export function flushMeasureBatch(handle, ctx) {
  const n = handle.measureCount | 0;
  if (n === 0) return 0;
  const t = typeof handle.takeMeasureReqs === "function" ? handle.takeMeasureReqs() : null;
  const widths = new Float64Array(n);
  if (ctx && t) {
    const soa = t.length === 3 && t[0] && t[0].length === n;
    for (let i = 0; i < n; i++) {
      const f = handle.resolve(soa ? t[0][i] : t[i].fontId) || "sans-serif";
      const text = handle.resolve(soa ? t[2][i] : t[i].textId) || "";
      ctx.font = font(f, soa ? t[1][i] : t[i].sizePx);
      widths[i] = ctx.measureText(text).width;
    }
  }
  handle.setMeasureResults(widths);
  return n;
}

/** @deprecated prefer Handle.takeMeasureReqs SoA; kept for tests that poke wasm memory. */
export function readMeasureReqsFromMemory(handle, memory) {
  const n = handle.measureCount | 0;
  if (n === 0) return [];
  const u32 = new Uint32Array(memory.buffer, handle.measureFontPtr, n);
  const f32 = new Float32Array(memory.buffer, handle.measureSizePtr, n);
  const t32 = new Uint32Array(memory.buffer, handle.measureTextPtr, n);
  const out = new Array(n);
  for (let i = 0; i < n; i++) {
    out[i] = { fontId: u32[i], sizePx: f32[i], textId: t32[i] };
  }
  return out;
}
