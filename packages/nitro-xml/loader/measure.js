// SPDX-License-Identifier: MIT
/** Batched measureText. sizePx -1/−2 = lineHeight/ascent; -3 = pptx GDI letter-spacing (px/char). */
const ADVANCE_STEPS_PER_PX = 6;
const MAX_TRACKING_PX_PER_CHAR = 1 / (2 * ADVANCE_STEPS_PER_PX);

function advancesOf(ctx, canvasFont, chars) {
  ctx.font = canvasFont;
  ctx.fontKerning = "none";
  const advances = [];
  let previous = 0;
  let prefix = "";
  for (const char of chars) {
    prefix += char;
    const width = ctx.measureText(prefix).width;
    advances.push(width - previous);
    previous = width;
  }
  return advances;
}

/** Ported from upstream text-metric-tracking.ts `resolveMetricTrackingPx`. */
export function resolveMetricTrackingPx(ctx, text, canvasFont) {
  if (!text || !ctx) return 0;
  const chars = [...text];
  let powerPoint = 0;
  for (const advance of advancesOf(ctx, canvasFont, chars)) {
    powerPoint += Math.round(advance * ADVANCE_STEPS_PER_PX);
  }
  ctx.font = canvasFont;
  ctx.fontKerning = "auto";
  const natural = ctx.measureText(text).width;
  if (!(natural > 0)) return 0;
  powerPoint /= ADVANCE_STEPS_PER_PX;
  const raw = (powerPoint - natural) / chars.length;
  return Math.min(MAX_TRACKING_PX_PER_CHAR, Math.max(-MAX_TRACKING_PX_PER_CHAR, raw));
}

export function fillMeasureWidths(ctx, n, fontAt, sizeAt, textAt) {
  const widths = new Float64Array(n);
  if (!ctx) return widths;
  for (let i = 0; i < n; i++) {
    const f = fontAt(i) || "sans-serif",
      sz = sizeAt(i),
      text = textAt(i) || "";
    const canvasFont = /\d+(\.\d+)?px/.test(f) ? f : `${sz < 0 ? 12 : sz}px ${f}`;
    if (sz === -3) {
      widths[i] = resolveMetricTrackingPx(ctx, text, canvasFont);
      continue;
    }
    ctx.font = canvasFont;
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
