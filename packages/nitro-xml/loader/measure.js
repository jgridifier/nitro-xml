// SPDX-License-Identifier: MIT
/** Batched measureText. sizePx -1/−2=lineHeight/ascent; -3=GDI track; -4=hang-punct spacing. */
export function fillMeasureWidths(ctx, n, fontAt, sizeAt, textAt) {
  const widths = new Float64Array(n);
  if (!ctx) return widths;
  for (let i = 0; i < n; i++) {
    const f = fontAt(i) || "sans-serif",
      sz = sizeAt(i),
      raw = textAt(i) || "",
      env = raw.startsWith("envw:"),
      text = env ? raw.slice(5) : raw;
    ctx.font = /\d+(\.\d+)?px/.test(f) ? f : `${sz < 0 ? 12 : sz}px ${f}`;
    if (env) {
      // WordArt envelope advances (pptx text.rs `envelope_glyph_advances`): kerned prefix widths.
      ctx.fontKerning = "normal";
      widths[i] = ctx.measureText(text).width;
      ctx.fontKerning = "auto";
      continue;
    }
    if (sz === -3) {
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
      const nat = ctx.measureText(text).width;
      widths[i] = nat > 0 ? Math.min(1 / 12, Math.max(-1 / 12, (ppt / 6 - nat) / ch.length)) : 0;
      continue;
    }
    if (sz === -4) {
      const ch = text.startsWith("pptx-hang-space:") ? text.slice(16) : text;
      widths[i] = ch
        ? Math.round((ctx.measureText(ch).width - ctx.measureText(" ").width) * 1e3) / 1e3
        : 0;
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
