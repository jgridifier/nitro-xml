// SPDX-License-Identifier: MIT
/** Batched measureText. sizePx -1/−2 = lineHeight/ascent; -3 = pptx GDI track; -4 = hang-punct word-spacing. */
export function fillMeasureWidths(ctx, n, fontAt, sizeAt, textAt) {
  const widths = new Float64Array(n);
  if (!ctx) return widths;
  for (let i = 0; i < n; i++) {
    const f = fontAt(i) || "sans-serif",
      sz = sizeAt(i),
      text = textAt(i) || "";
    ctx.font = /\d+(\.\d+)?px/.test(f) ? f : `${sz < 0 ? 12 : sz}px ${f}`;
    if (sz === -3) {
      // GDI: round advances to 1/6px (upstream text-metric-tracking).
      if (!text) {
        widths[i] = 0;
        continue;
      }
      const ch = [...text];
      ctx.fontKerning = "none";
      let ppt = 0,
        prev = 0,
        pref = "";
      for (const c of ch) {
        pref += c;
        const w = ctx.measureText(pref).width;
        ppt += Math.round((w - prev) * 6);
        prev = w;
      }
      ctx.fontKerning = "auto";
      const nat = ctx.measureText(text).width,
        lim = 1 / 12;
      widths[i] = nat > 0 ? Math.min(lim, Math.max(-lim, (ppt / 6 - nat) / ch.length)) : 0;
      continue;
    }
    if (sz === -4) {
      const ch = text.startsWith("pptx-hang-space:")
        ? text.slice("pptx-hang-space:".length)
        : text;
      if (!ch) {
        widths[i] = 0;
        continue;
      }
      const glyph = ctx.measureText(ch).width;
      const space = ctx.measureText(" ").width;
      widths[i] = Math.round((glyph - space) * 1000) / 1000;
      continue;
    }
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
