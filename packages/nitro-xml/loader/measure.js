// SPDX-License-Identifier: MIT
/** Batched measureText read-back. `takeMeasureReqs` returns SoA `[fonts,sizes,texts]` or object rows. */
/** Line-height / ascent sentinels (docx CachedMeasurer; matches createCanvasMeasurer). */
const LH = "\u0001";
const ASC = "\u0002";
function font(f, sz) {
  return /\d+(\.\d+)?px/.test(f) ? f : `${sz}px ${f}`;
}
function splitFont(f) {
  const p = f.indexOf("\0");
  return p < 0 ? [f, "auto"] : [f.slice(0, p), f.slice(p + 1) || "auto"];
}
function sample(ctx, text, sz) {
  const m = ctx.measureText(text === LH || text === ASC ? "Mg" : text);
  if (text === LH) {
    const a = m.fontBoundingBoxAscent;
    const d = m.fontBoundingBoxDescent;
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
      const raw = handle.resolve(soa ? t[0][i] : t[i].fontId) || "sans-serif";
      const [f, k] = splitFont(raw);
      const sz = soa ? t[1][i] : t[i].sizePx;
      const text = handle.resolve(soa ? t[2][i] : t[i].textId) || "";
      ctx.font = font(f, sz);
      if (ctx.fontKerning !== undefined) ctx.fontKerning = k;
      widths[i] = sample(ctx, text, sz);
    }
  }
  handle.setMeasureResults(widths);
  return n;
}

/** wasm-bindgen exposes SoA ptrs as methods — call them for the byte offset. */
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
  for (let i = 0; i < n; i++) {
    out[i] = { fontId: u32[i], sizePx: f32[i], textId: t32[i] };
  }
  return out;
}
