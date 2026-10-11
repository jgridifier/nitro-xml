// SPDX-License-Identifier: MIT
/** Batched text-measurement read-back: one setMeasureResults per frame. */

/** Run pending measure requests; returns widths written (0 if none). */
export function flushMeasureBatch(handle, ctx) {
  const n = handle.measureCount | 0;
  if (n === 0) return 0;
  const reqs =
    typeof handle.takeMeasureReqs === "function" ? handle.takeMeasureReqs() : [];
  if (reqs.length !== n) {
    const widths = new Float64Array(n);
    handle.setMeasureResults(widths);
    return n;
  }
  const widths = new Float64Array(n);
  if (ctx) {
    for (let i = 0; i < n; i++) {
      const r = reqs[i];
      const font = handle.resolve(r.fontId) || "sans-serif";
      const text = handle.resolve(r.textId) || "";
      ctx.font = `${r.sizePx}px ${font}`;
      widths[i] = ctx.measureText(text).width;
    }
  }
  handle.setMeasureResults(widths);
  return n;
}

/** Build measure request list from wasm SoA pointers. */
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
