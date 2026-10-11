// SPDX-License-Identifier: MIT
/** Batched measureText; LH/ascent sentinels `\u0001`/`\u0002` match createCanvasMeasurer. */
const LH = "\u0001",
  ASC = "\u0002";
export function measureOne(ctx, raw, sz, text) {
  let f = raw || "sans-serif",
    k = "auto",
    p = f.indexOf("\0");
  if (p >= 0) {
    k = f.slice(p + 1) || "auto";
    f = f.slice(0, p);
  }
  ctx.font = /\d+(\.\d+)?px/.test(f) ? f : `${sz}px ${f}`;
  if (ctx.fontKerning !== undefined) ctx.fontKerning = k;
  const m = ctx.measureText(text === LH || text === ASC ? "Mg" : text);
  if (text === LH) {
    const a = m.fontBoundingBoxAscent,
      d = m.fontBoundingBoxDescent;
    return a !== undefined && d !== undefined ? a + d : sz * 1.22;
  }
  if (text === ASC) return m.fontBoundingBoxAscent ?? sz * 0.95;
  return m.width;
}
export function flushMeasureBatch(handle, ctx) {
  const n = handle.measureCount | 0;
  if (n === 0) return 0;
  const t = typeof handle.takeMeasureReqs === "function" ? handle.takeMeasureReqs() : null;
  const widths = new Float64Array(n);
  if (ctx && t) {
    const soa = t.length === 3 && t[0] && t[0].length === n;
    for (let i = 0; i < n; i++) {
      widths[i] = measureOne(
        ctx,
        handle.resolve(soa ? t[0][i] : t[i].fontId),
        soa ? t[1][i] : t[i].sizePx,
        handle.resolve(soa ? t[2][i] : t[i].textId) || "",
      );
    }
  }
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
