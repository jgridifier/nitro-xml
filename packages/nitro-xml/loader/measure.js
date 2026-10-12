// SPDX-License-Identifier: MIT
/** Batched measureText. sizePx -1/−2 = lineHeight/ascent (fontBoundingBox; createCanvasMeasurer). */
export function fillMeasureWidths(ctx, n, fontAt, sizeAt, textAt) {
  const widths = new Float64Array(n);
  if (!ctx) return widths;
  for (let i = 0; i < n; i++) {
    const f = fontAt(i) || "sans-serif",
      sz = sizeAt(i),
      text = textAt(i) || "";
    ctx.font = /\d+(\.\d+)?px/.test(f) ? f : `${sz < 0 ? 12 : sz}px ${f}`;
    const m = ctx.measureText(sz < 0 ? "Mg" : text),
      a = m.fontBoundingBoxAscent,
      d = m.fontBoundingBoxDescent;
    widths[i] =
      sz === -2
        ? (a ?? 12 * 0.95)
        : sz === -1
          ? a !== undefined && d !== undefined
            ? a + d
            : 12 * 1.22
          : m.width;
  }
  return widths;
}
export function flushMeasureBatch(handle, ctx) {
  const n = handle.measureCount | 0;
  if (n === 0) return 0;
  const t = typeof handle.takeMeasureReqs === "function" ? handle.takeMeasureReqs() : null;
  const soa = !!(t && t.length === 3 && t[0] && t[0].length === n);
  const widths =
    ctx && t
      ? fillMeasureWidths(
          ctx,
          n,
          (i) => handle.resolve(soa ? t[0][i] : t[i].fontId),
          (i) => (soa ? t[1][i] : t[i].sizePx),
          (i) => handle.resolve(soa ? t[2][i] : t[i].textId) || "",
        )
      : new Float64Array(n);
  handle.setMeasureResults(widths);
  return n;
}
function ptr(handle, name) {
  const v = handle[name];
  return typeof v === "function" ? v.call(handle) : v;
}
/** @deprecated prefer Handle.takeMeasureReqs SoA; kept for tests that poke wasm memory. */
export function readMeasureReqsFromMemory(handle, memory) {
  const n = handle.measureCount | 0;
  if (n === 0) return [];
  const u32 = new Uint32Array(memory.buffer, ptr(handle, "measureFontPtr"), n);
  const f32 = new Float32Array(memory.buffer, ptr(handle, "measureSizePtr"), n);
  const t32 = new Uint32Array(memory.buffer, ptr(handle, "measureTextPtr"), n);
  const out = new Array(n);
  for (let i = 0; i < n; i++) out[i] = { fontId: u32[i], sizePx: f32[i], textId: t32[i] };
  return out;
}
