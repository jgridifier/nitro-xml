// SPDX-License-Identifier: MIT
/**
 * Batched text-measurement read-back (Head of Product / #33 / #37).
 * One `measureText` pass per frame; widths return via a single `setMeasureResults`.
 */

/**
 * @typedef {{
 *   measureCount: number,
 *   resolve: (id: number) => string | undefined | null,
 *   setMeasureResults: (widths: Float64Array) => void,
 * }} MeasureHandle
 *
 * Requests are read via optional SoA pointers when the host can view wasm memory,
 * otherwise via a `takeMeasureReqs()` helper the session provides.
 */

/**
 * Run all pending measure requests and write results back in one call.
 *
 * @param {MeasureHandle & {
 *   takeMeasureReqs?: () => { fontId: number, sizePx: number, textId: number }[],
 * }} handle
 * @param {CanvasRenderingContext2D | null} ctx
 * @returns {number} number of widths written (0 if none)
 */
export function flushMeasureBatch(handle, ctx) {
  const n = handle.measureCount | 0;
  if (n === 0) return 0;
  const reqs =
    typeof handle.takeMeasureReqs === "function"
      ? handle.takeMeasureReqs()
      : [];
  if (reqs.length !== n) {
    // Length must match pending count or setMeasureResults rejects.
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

/**
 * Build a request list from wasm SoA pointers (when memory views are available).
 * Used by the session when the Handle exposes measure*Ptr + memory.
 *
 * @param {{
 *   measureCount: number,
 *   measureFontPtr: number,
 *   measureSizePtr: number,
 *   measureTextPtr: number,
 * }} handle
 * @param {WebAssembly.Memory} memory
 */
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
