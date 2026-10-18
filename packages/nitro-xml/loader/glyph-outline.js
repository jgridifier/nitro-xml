// SPDX-License-Identifier: MIT
/**
 * Host glyph-outline tracing for WordArt envelope presets.
 * Rasterizes glyphs on an offscreen canvas at 512px, contours at 50% coverage
 * via marching squares with sub-pixel accuracy, simplifies via Douglas-Peucker,
 * and returns contours in em units for per-point warping through envelope maps.
 */

const TRACE_SIZE_PX = 512;
const SIMPLIFY_PX = 0.2;
const PAD = 3;

let traceCanvas = null;

function getTraceContext() {
  if (typeof document === "undefined" || !document.createElement) {
    return null;
  }
  if (!traceCanvas) {
    traceCanvas = document.createElement("canvas");
  }
  return traceCanvas.getContext("2d", { willReadFrequently: true }) || null;
}

/**
 * Douglas-Peucker polyline simplification.
 * Returns simplified contour with tolerance applied.
 */
function simplify(points, tolerance) {
  if (points.length < 4) {
    return points;
  }
  const keep = new Uint8Array(points.length);
  keep[0] = 1;
  keep[points.length - 1] = 1;
  const stack = [[0, points.length - 1]];
  while (stack.length > 0) {
    const [a, b] = stack.pop();
    const pa = points[a];
    const pb = points[b];
    const len = Math.hypot(pb.x - pa.x, pb.y - pa.y);
    let worst = -1;
    let worstDist = tolerance;
    for (let i = a + 1; i < b; i++) {
      const p = points[i];
      const dist =
        len > 0
          ? Math.abs((pb.x - pa.x) * (pa.y - p.y) - (pa.x - p.x) * (pb.y - pa.y)) / len
          : Math.hypot(p.x - pa.x, p.y - pa.y);
      if (dist > worstDist) {
        worst = i;
        worstDist = dist;
      }
    }
    if (worst >= 0) {
      keep[worst] = 1;
      stack.push([a, worst], [worst, b]);
    }
  }
  return points.filter((_, i) => keep[i] === 1);
}

/**
 * Calculate signed area of a closed contour.
 */
function signedArea(ring) {
  let area = 0;
  for (let i = 0; i < ring.length; i++) {
    const a = ring[i];
    const b = ring[(i + 1) % ring.length];
    area += a.x * b.y - b.x * a.y;
  }
  return area / 2;
}

/**
 * Point-in-polygon test for winding determination.
 */
function pointInRing(p, ring) {
  let inside = false;
  for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
    const a = ring[i];
    const b = ring[j];
    if (a.y > p.y !== b.y > p.y && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x) {
      inside = !inside;
    }
  }
  return inside;
}

/**
 * Marching-squares iso-contouring of a coverage raster at level 0.5.
 * Returns closed rings oriented for SVG nonzero fill rule.
 */
function contourRaster(values, w, h, level) {
  const at = (x, y) => (x < 0 || y < 0 || x >= w || y >= h ? 0 : values[y * w + x]);
  const stride = w + 2;
  const hKey = (x, y) => ((y + 1) * stride + (x + 1)) * 2;
  const vKey = (x, y) => ((y + 1) * stride + (x + 1)) * 2 + 1;
  const points = new Map();
  
  const point = (key, x, y, horizontal) => {
    if (!points.has(key)) {
      const a = at(x, y);
      const b = horizontal ? at(x + 1, y) : at(x, y + 1);
      const t = b !== a ? (level - a) / (b - a) : 0.5;
      points.set(key, horizontal ? { x: x + t, y } : { x, y: y + t });
    }
    return key;
  };

  const edge = (side, x, y) => {
    switch (side) {
      case "top":
        return point(hKey(x, y), x, y, true);
      case "bottom":
        return point(hKey(x, y + 1), x, y + 1, true);
      case "left":
        return point(vKey(x, y), x, y, false);
      default:
        return point(vKey(x + 1, y), x + 1, y, false);
    }
  };

  const links = new Map();
  const link = (a, b) => {
    if (!links.has(a)) links.set(a, []);
    if (!links.has(b)) links.set(b, []);
    links.get(a).push(b);
    links.get(b).push(a);
  };

  for (let y = -1; y < h; y++) {
    for (let x = -1; x < w; x++) {
      const tl = at(x, y) >= level ? 1 : 0;
      const tr = at(x + 1, y) >= level ? 1 : 0;
      const br = at(x + 1, y + 1) >= level ? 1 : 0;
      const bl = at(x, y + 1) >= level ? 1 : 0;
      const code = tl * 8 + tr * 4 + br * 2 + bl;
      if (code === 0 || code === 15) continue;
      
      const centre = (at(x, y) + at(x + 1, y) + at(x + 1, y + 1) + at(x, y + 1)) / 4 >= level;
      
      switch (code) {
        case 1:
        case 14:
          link(edge("left", x, y), edge("bottom", x, y));
          break;
        case 2:
        case 13:
          link(edge("bottom", x, y), edge("right", x, y));
          break;
        case 3:
        case 12:
          link(edge("left", x, y), edge("right", x, y));
          break;
        case 4:
        case 11:
          link(edge("top", x, y), edge("right", x, y));
          break;
        case 6:
        case 9:
          link(edge("top", x, y), edge("bottom", x, y));
          break;
        case 7:
        case 8:
          link(edge("left", x, y), edge("top", x, y));
          break;
        case 5:
          if (centre) {
            link(edge("left", x, y), edge("top", x, y));
            link(edge("bottom", x, y), edge("right", x, y));
          } else {
            link(edge("left", x, y), edge("bottom", x, y));
            link(edge("top", x, y), edge("right", x, y));
          }
          break;
        case 10:
          if (centre) {
            link(edge("left", x, y), edge("bottom", x, y));
            link(edge("top", x, y), edge("right", x, y));
          } else {
            link(edge("left", x, y), edge("top", x, y));
            link(edge("bottom", x, y), edge("right", x, y));
          }
          break;
      }
    }
  }

  const rings = [];
  const visited = new Set();
  
  for (const startKey of links.keys()) {
    if (visited.has(startKey)) continue;
    
    const ring = [];
    let prev;
    let current = startKey;
    
    while (current !== undefined && !visited.has(current)) {
      visited.add(current);
      ring.push(points.get(current));
      let next;
      for (const k of links.get(current) || []) {
        if (k !== prev && !visited.has(k)) {
          next = k;
          break;
        }
      }
      prev = current;
      current = next;
    }
    
    if (ring.length >= 3) {
      rings.push(ring);
    }
  }

  // Orient rings for SVG nonzero fill rule
  return rings.map((ring, i) => {
    const depth = rings.reduce(
      (n, other, j) => (j !== i && pointInRing(ring[0], other) ? n + 1 : n),
      0
    );
    const wantPositive = depth % 2 === 0;
    return signedArea(ring) > 0 === wantPositive ? ring : ring.slice().reverse();
  });
}

/**
 * Trace a single glyph at the given font.
 * Returns { contours: [[{x,y},...], ...] } or null if tracing fails.
 */
function traceGlyph(char, canvasFont, fontSizePx) {
  const ctx = getTraceContext();
  if (!ctx || !traceCanvas) return null;

  ctx.font = canvasFont;
  const m = ctx.measureText(char);
  const left = Math.floor(-(m.actualBoundingBoxLeft ?? TRACE_SIZE_PX)) - PAD;
  const right = Math.ceil(m.actualBoundingBoxRight ?? TRACE_SIZE_PX * 2) + PAD;
  const top = Math.floor(-(m.actualBoundingBoxAscent ?? TRACE_SIZE_PX * 1.2)) - PAD;
  const bottom = Math.ceil(m.actualBoundingBoxDescent ?? TRACE_SIZE_PX * 0.5) + PAD;
  
  const w = right - left;
  const h = bottom - top;
  
  if (!(w > 0 && h > 0 && w < TRACE_SIZE_PX * 8 && h < TRACE_SIZE_PX * 8)) {
    return null;
  }

  traceCanvas.width = w;
  traceCanvas.height = h;
  ctx.clearRect(0, 0, w, h);
  ctx.font = canvasFont;
  ctx.fillStyle = "#000";
  ctx.textBaseline = "alphabetic";
  ctx.fillText(char, -left, -top);

  const imageData = ctx.getImageData(0, 0, w, h);
  const data = imageData.data;
  const alpha = new Float32Array(w * h);
  for (let i = 0; i < w * h; i++) {
    alpha[i] = data[i * 4 + 3] / 255;
  }

  const contours = contourRaster(alpha, w, h, 0.5).map((contour) =>
    simplify(contour, SIMPLIFY_PX).map((p) => ({
      x: (p.x + 0.5 + left) / TRACE_SIZE_PX,
      y: (p.y + 0.5 + top) / TRACE_SIZE_PX,
    }))
  );

  return { contours: contours.filter((c) => c.length >= 3) };
}

// Cache of traced glyphs keyed by "fontString|char"
const traceCache = new Map();

/**
 * Fill outline contours for batched glyph probes.
 * Input: array of [charIndex, canvasFontId] pairs (resolved to strings in Rust)
 * Output: sparse array of outline contours or null per probe
 *
 * This is called from session.js after Rust queues outline requests.
 */
export function fillOutlineContours(ctx, n, charAt, fontAt, fontSizePxAt) {
  const contours = new Array(n);
  if (!ctx) {
    for (let i = 0; i < n; i++) {
      contours[i] = null;
    }
    return contours;
  }

  for (let i = 0; i < n; i++) {
    const char = charAt(i) || "";
    const canvasFont = fontAt(i) || "sans-serif";
    const fontSizePx = fontSizePxAt(i) || 12;

    const key = `${canvasFont}|${char}`;
    let traced = traceCache.get(key);

    if (traced === undefined) {
      traced = traceGlyph(char, canvasFont, fontSizePx);
      // Cache only if font is ready or tracing failed
      if (traced === null || (typeof document !== "undefined" && document.fonts && document.fonts.check)) {
        traceCache.set(key, traced);
      }
    }

    if (traced && traced.contours.length > 0) {
      contours[i] = traced.contours;
    } else {
      contours[i] = null;
    }
  }

  return contours;
}

/**
 * Export for testing: reset the trace cache and canvas.
 */
export function resetGlyphTraceCache() {
  traceCache.clear();
  traceCanvas = null;
}

/**
 * Flush outline batch from WASM to the host. Called from session.js on each tick.
 * Takes pending outline requests and calls fillOutlineContours, then returns results to Rust.
 */
export function flushOutlineBatch(handle, ctx) {
  const n = handle.outlineCount | 0;
  if (n === 0) return 0;
  
  const t = typeof handle.takeOutlineReqs === "function" ? handle.takeOutlineReqs() : null;
  const soa = !!(t && t.length === 3 && t[0] && t[0].length === n);
  
  if (!soa || !t) {
    return 0;
  }

  // t[0] = fonts, t[1] = chars, t[2] = sizes
  const contours = fillOutlineContours(
    ctx,
    n,
    (i) => handle.resolve(soa ? t[1][i] : t[i].charId),
    (i) => handle.resolve(soa ? t[0][i] : t[i].fontId),
    (i) => soa ? t[2][i] : t[i].sizePx,
  );

  // Convert contours to a format we can pass to WASM
  // For now, we'll just return the count; the actual outline data 
  // needs to be handled via a different mechanism (TBD).
  handle.setOutlineResults(contours);
  return n;
}
