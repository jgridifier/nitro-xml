// SPDX-License-Identifier: MIT
// Fast main-thread xlsx selection overlay, header selection, edge auto-scroll,
// size tip, and multi-range controller (#612). Lazy chunk: off eager brotli budget.
import { EventKind as EK } from "../events.js";
import { applyChromePatch } from "./chrome-patch.js";

const MAX_ROW = 1_048_575;
const MAX_COL = 16_383;
const CLEAR_CAP = 10_000;

const FORMAT_CMDS = new Set([
  "home.bold", "home.italic", "home.strikethrough", "home.underline", "home.underline-double",
  "home.grow-font", "home.shrink-font", "home.align-left", "home.align-center", "home.align-right",
  "home.align-top", "home.align-middle", "home.align-bottom", "home.wrap-text",
  "home.percent", "home.format-percent", "home.comma", "home.accounting",
  "home.format-general", "home.format-number", "home.format-currency",
  "home.format-scientific", "home.format-date", "home.format-time",
]);
const STRUCT_CMDS = new Set([
  "cells.insert-rows", "cells.delete-rows", "cells.insert-columns", "cells.delete-columns",
]);

const STYLE = `
.xg-root .xg-overlay>.xg-outline,.xg-root .xg-overlay>.xg-active,.xg-root .xg-overlay>.xg-handle{display:none!important}
.nx-sel-layer{position:absolute;inset:0;pointer-events:none;overflow:hidden;z-index:5}
.nx-sel-q{position:absolute;overflow:hidden;pointer-events:none}
.nx-sel-shade{position:absolute;background:var(--xve-selection,rgba(33,115,70,.14));pointer-events:none}
.nx-sel-outline{position:absolute;box-sizing:border-box;border:2px solid var(--xve-selection-border,#217346);pointer-events:none}
.nx-sel-active{position:absolute;box-sizing:border-box;border:1px solid var(--xve-selection-border,#217346);pointer-events:none}
.nx-sel-handle{position:absolute;box-sizing:border-box;width:7px;height:7px;background:var(--xve-selection-border,#217346);border:1px solid #fff;pointer-events:none}
.nx-sel-tip{position:absolute;z-index:12;pointer-events:none;background:rgba(32,32,32,.88);color:#fff;font:600 11px/1.2 system-ui,sans-serif;padding:3px 7px;border-radius:3px;white-space:nowrap;box-shadow:0 2px 6px rgba(0,0,0,.22)}
.nx-sel-toast{position:absolute;left:50%;bottom:16px;transform:translateX(-50%);z-index:15;pointer-events:none;background:#252423;color:#fff;font:500 12px/1.35 system-ui,sans-serif;padding:7px 14px;border-radius:4px;box-shadow:0 4px 12px rgba(0,0,0,.25);max-width:90%;text-align:center}
`;

export function colLabel(col) {
  let label = "";
  let n = col + 1;
  while (n > 0) {
    const rem = (n - 1) % 26;
    label = String.fromCharCode(65 + rem) + label;
    n = Math.floor((n - 1) / 26);
  }
  return label;
}

function colIndex(label) {
  if (!label || label.length > 3 || !/^[A-Za-z]+$/.test(label)) return -1;
  let n = 0;
  for (const ch of label.toUpperCase()) n = n * 26 + (ch.charCodeAt(0) - 64);
  return n - 1;
}

export function formatAddr(row, col) {
  return colLabel(col) + (row + 1);
}

export function rangeRef(rg) {
  const r0 = Math.min(rg.r0, rg.r1), r1 = Math.max(rg.r0, rg.r1);
  const c0 = Math.min(rg.c0, rg.c1), c1 = Math.max(rg.c0, rg.c1);
  const wholeRows = c0 === 0 && c1 === MAX_COL;
  const wholeCols = r0 === 0 && r1 === MAX_ROW;
  if (wholeRows && wholeCols) return `1:${MAX_ROW + 1}`;
  if (wholeRows) return `${r0 + 1}:${r1 + 1}`;
  if (wholeCols) return `${colLabel(c0)}:${colLabel(c1)}`;
  if (r0 === r1 && c0 === c1) return formatAddr(r0, c0);
  return `${formatAddr(r0, c0)}:${formatAddr(r1, c1)}`;
}

function parseAddr(ref) {
  const m = /^\$?([A-Za-z]{1,3})\$?(\d+)$/.exec((ref || "").trim());
  if (!m) return null;
  const col = colIndex(m[1]);
  const row = parseInt(m[2], 10) - 1;
  if (col < 0 || col > MAX_COL || row < 0 || row > MAX_ROW) return null;
  return { row, col };
}

export function parseSingleRange(ref) {
  const s = (ref || "").trim();
  if (!s) return null;
  const parts = s.split(":");
  if (parts.length === 1) {
    const p = parseAddr(parts[0]);
    return p ? { r0: p.row, c0: p.col, r1: p.row, c1: p.col, kind: "cell" } : null;
  }
  if (parts.length !== 2) return null;
  const a = parts[0].replace(/\$/g, ""), b = parts[1].replace(/\$/g, "");
  if (/^[A-Za-z]+$/.test(a) && /^[A-Za-z]+$/.test(b)) {
    const c1 = colIndex(a), c2 = colIndex(b);
    if (c1 < 0 || c2 < 0 || c1 > MAX_COL || c2 > MAX_COL) return null;
    return { r0: 0, c0: Math.min(c1, c2), r1: MAX_ROW, c1: Math.max(c1, c2), kind: "col" };
  }
  if (/^\d+$/.test(a) && /^\d+$/.test(b)) {
    const r1 = parseInt(a, 10) - 1, r2 = parseInt(b, 10) - 1;
    if (r1 < 0 || r2 < 0 || r1 > MAX_ROW || r2 > MAX_ROW) return null;
    const whole = (r1 === 0 && r2 === MAX_ROW) || (r2 === 0 && r1 === MAX_ROW);
    return { r0: Math.min(r1, r2), c0: 0, r1: Math.max(r1, r2), c1: MAX_COL, kind: whole ? "all" : "row" };
  }
  const p = parseAddr(a), q = parseAddr(b);
  if (!p || !q) return null;
  return { r0: Math.min(p.row, q.row), c0: Math.min(p.col, q.col), r1: Math.max(p.row, q.row), c1: Math.max(p.col, q.col), kind: "cell" };
}

export function parseSelectionString(str) {
  const out = [];
  for (const part of String(str || "").split(",")) {
    const rg = parseSingleRange(part);
    if (rg) out.push(rg);
  }
  return out;
}

function parseTranslate3d(tr) {
  const m = /translate3d\(\s*(-?[\d.]+)px\s*,\s*(-?[\d.]+)px/.exec(tr || "");
  return m ? { x: parseFloat(m[1]), y: parseFloat(m[2]) } : { x: 0, y: 0 };
}

function autoScrollStep(dist) {
  if (!dist) return 0;
  const a = Math.abs(dist);
  const px = Math.min(240, Math.max(4, Math.round(a * 0.35 + (a > 40 ? (a - 40) * 0.5 : 0))));
  return dist < 0 ? -px : px;
}

export function attach(el) {
  if (el._nxSel) {
    el._nxSel.onDocAttach();
    return;
  }
  const sr = el.shadowRoot;
  const v = sr && sr.querySelector(".v");
  if (!sr || !v) return;

  const st = document.createElement("style");
  st.dataset.nxSel = "1";
  st.textContent = STYLE;
  sr.appendChild(st);

  const layer = document.createElement("div");
  layer.className = "nx-sel-layer";
  layer.setAttribute("aria-hidden", "true");
  v.appendChild(layer);

  const tip = document.createElement("div");
  tip.className = "nx-sel-tip";
  tip.hidden = true;
  v.appendChild(tip);

  const toast = document.createElement("div");
  toast.className = "nx-sel-toast";
  toast.setAttribute("role", "status");
  toast.setAttribute("aria-live", "polite");
  toast.hidden = true;
  v.appendChild(toast);

  // Selection state (authoritative on main thread for fast overlay + multi-range)
  const state = {
    ranges: [{ r0: 0, c0: 0, r1: 0, c1: 0, kind: "cell" }],
    active: { row: 0, col: 0 },
    anchor: { row: 0, col: 0 },
    anchors: [{ row: 0, col: 0 }],
    dragging: null, // null | { area: 'cell'|'col'|'row', pointerId, ctrl, x, y }
    scrollX: 0,
    scrollY: 0,
    headerW: 0,
    headerH: 0,
    frozenW: 0,
    frozenH: 0,
    frozenRows: 0,
    frozenCols: 0,
    viewW: 800,
    viewH: 600,
    zoom: 100,
    sheet: 0,
    colMap: new Map(),
    rowMap: new Map(),
    merges: [], // [{ r0, c0, r1, c1 }]
    paintRaf: 0,
    autoRaf: 0,
    scrollTickBusy: false,
    toastTimer: 0,
    lastDoc: null,
    syncSeq: 0,
    syncPending: Promise.resolve(),
  };

  let suppressViewer = false;
  let wrappedDoc = null;

  function wrapDoc(d) {
    if (!d || wrappedDoc === d) return;
    wrappedDoc = d;
    const origPush = d.pushEvent.bind(d);
    d.pushEvent = (ev) => {
      if (suppressViewer && (ev.kind === EK.Pointer || ev.kind === EK.Keyboard)) return;
      return origPush(ev);
    };
    if (typeof d.editorKey === "function") {
      const origKey = d.editorKey.bind(d);
      d.editorKey = async (ev, mac) => {
        const res = await origKey(ev, mac);
        if (res && res.handled && res.selection) {
          adoptSingleOrMultiRef(res.selection);
          schedulePaint();
        }
        return res;
      };
    }
  }

  // Also wrap el._tick so suppressed events don't schedule a take-frame grid repaint
  const origElTick = el._tick.bind(el);
  el._tick = () => {
    if (suppressViewer) return;
    return origElTick();
  };

  function defaultColW() {
    return Math.max(1, Math.round((64 * state.zoom) / 100));
  }
  function defaultRowH() {
    return Math.max(1, Math.round((20 * state.zoom) / 100));
  }

  function colRect(c) {
    const hit = state.colMap.get(c);
    if (hit) return hit;
    const dw = defaultColW();
    let bestK = -1, bestR = null;
    for (const [k, r] of state.colMap) {
      if (k <= c && k > bestK) { bestK = k; bestR = r; }
    }
    if (bestR) return { left: bestR.left + bestR.width + (c - bestK - 1) * dw, width: dw };
    return { left: c * dw, width: dw };
  }

  function rowRect(r) {
    const hit = state.rowMap.get(r);
    if (hit) return hit;
    const dh = defaultRowH();
    let bestK = -1, bestR = null;
    for (const [k, rect] of state.rowMap) {
      if (k <= r && k > bestK) { bestK = k; bestR = rect; }
    }
    if (bestR) return { top: bestR.top + bestR.height + (r - bestK - 1) * dh, height: dh };
    return { top: r * dh, height: dh };
  }

  function colAtPlaneX(px) {
    if (px <= 0) return 0;
    let bestK = -1, bestR = null;
    for (const [k, r] of state.colMap) {
      if (px >= r.left && px < r.left + r.width) return k;
      if (r.left <= px && k > bestK) { bestK = k; bestR = r; }
    }
    const dw = defaultColW();
    if (bestR) {
      const idx = bestK + 1 + Math.floor(Math.max(0, px - (bestR.left + bestR.width)) / dw);
      return Math.max(0, Math.min(MAX_COL, idx));
    }
    return Math.max(0, Math.min(MAX_COL, Math.floor(px / dw)));
  }

  function rowAtPlaneY(py) {
    if (py <= 0) return 0;
    let bestK = -1, bestR = null;
    for (const [k, r] of state.rowMap) {
      if (py >= r.top && py < r.top + r.height) return k;
      if (r.top <= py && k > bestK) { bestK = k; bestR = r; }
    }
    const dh = defaultRowH();
    if (bestR) {
      const idx = bestK + 1 + Math.floor(Math.max(0, py - (bestR.top + bestR.height)) / dh);
      return Math.max(0, Math.min(MAX_ROW, idx));
    }
    return Math.max(0, Math.min(MAX_ROW, Math.floor(py / dh)));
  }

  function readGridMetrics() {
    const root = el._r && el._r.querySelector(".xg-root");
    if (!root) return false;
    const d = el.document;
    const zoomTxt = el._ch?.querySelector(".z")?.textContent || "";
    const zPos = d?._position?.zoom;
    const nextZoom = parseInt(zoomTxt, 10) || (Number.isFinite(zPos) && zPos > 0 ? Math.round(zPos) : 100);
    const nextSheet = d?._position?.sheet ?? 0;
    if (nextZoom !== state.zoom || nextSheet !== state.sheet) {
      state.zoom = nextZoom;
      state.sheet = nextSheet;
      state.colMap.clear();
      state.rowMap.clear();
      state.merges = [];
    }

    const vr = root.getBoundingClientRect();
    state.viewW = root.clientWidth || Math.round(vr.width) || 800;
    state.viewH = root.clientHeight || Math.round(vr.height) || 600;

    const corner = root.querySelector(".xg-corner");
    state.headerW = corner ? parseFloat(corner.style.width) || 0 : 0;
    state.headerH = corner ? parseFloat(corner.style.height) || 0 : 0;

    const qTop = root.querySelector(".xg-q-top");
    state.frozenH = qTop ? parseFloat(qTop.style.height) || 0 : 0;
    const freezeV = root.querySelector(".xg-freeze-v");
    state.frozenW = freezeV ? Math.max(0, (parseFloat(freezeV.style.left) || 0) + 1 - state.headerW) : 0;

    const hdrCol = root.querySelector(".xg-hdr-col");
    const hdrRow = root.querySelector(".xg-hdr-row");
    const mainPlane = root.querySelector(".xg-q-main .xg-plane");
    if (hdrCol) {
      state.scrollX = Math.max(0, Math.round(-parseTranslate3d(hdrCol.style.transform).x));
    } else if (mainPlane) {
      state.scrollX = Math.max(0, Math.round(-parseTranslate3d(mainPlane.style.transform).x - state.frozenW));
    }
    if (hdrRow) {
      state.scrollY = Math.max(0, Math.round(-parseTranslate3d(hdrRow.style.transform).y));
    } else if (mainPlane) {
      state.scrollY = Math.max(0, Math.round(-parseTranslate3d(mainPlane.style.transform).y - state.frozenH));
    }

    if (hdrCol) {
      for (const hd of hdrCol.children) {
        const idx = parseInt(hd.dataset.index, 10);
        if (!Number.isFinite(idx)) continue;
        const left = parseFloat(hd.style.left);
        const width = parseFloat(hd.style.width);
        if (Number.isFinite(left) && Number.isFinite(width)) state.colMap.set(idx, { left, width });
      }
    }
    if (hdrRow) {
      for (const hd of hdrRow.children) {
        const idx = parseInt(hd.dataset.index, 10);
        if (!Number.isFinite(idx)) continue;
        const top = parseFloat(hd.style.top);
        const height = parseFloat(hd.style.height);
        if (Number.isFinite(top) && Number.isFinite(height)) state.rowMap.set(idx, { top, height });
      }
    }

    state.frozenCols = 0;
    if (state.frozenW > 0) {
      for (const [k, r] of state.colMap) {
        if (r.left < state.frozenW && k + 1 > state.frozenCols) state.frozenCols = k + 1;
      }
    }
    state.frozenRows = 0;
    if (state.frozenH > 0) {
      for (const [k, r] of state.rowMap) {
        if (r.top < state.frozenH && k + 1 > state.frozenRows) state.frozenRows = k + 1;
      }
    }

    // Collect visible merged cell extents from `.xg-merged`
    const seenM = new Set();
    const merges = [];
    for (const cell of root.querySelectorAll(".xg-merged")) {
      const x = parseFloat(cell.style.left), y = parseFloat(cell.style.top);
      const w = parseFloat(cell.style.width), h = parseFloat(cell.style.height);
      if (!Number.isFinite(x) || !Number.isFinite(y) || w <= 0 || h <= 0) continue;
      const c0 = colAtPlaneX(x + 1), c1 = colAtPlaneX(x + w - 2);
      const r0 = rowAtPlaneY(y + 1), r1 = rowAtPlaneY(y + h - 2);
      const key = `${r0}:${c0}:${r1}:${c1}`;
      if (!seenM.has(key) && (r1 > r0 || c1 > c0)) {
        seenM.add(key);
        merges.push({ r0, c0, r1, c1 });
      }
    }
    state.merges = merges;
    return true;
  }

  function expandForMerges(rg) {
    if (!state.merges.length || (rg.r0 === 0 && rg.r1 === MAX_ROW) || (rg.c0 === 0 && rg.c1 === MAX_COL)) {
      return { ...rg };
    }
    const out = { ...rg };
    for (let pass = 0; pass < 8; pass++) {
      let grew = false;
      for (const m of state.merges) {
        if (m.r0 <= out.r1 && m.r1 >= out.r0 && m.c0 <= out.c1 && m.c1 >= out.c0) {
          const nr0 = Math.min(out.r0, m.r0), nc0 = Math.min(out.c0, m.c0);
          const nr1 = Math.max(out.r1, m.r1), nc1 = Math.max(out.c1, m.c1);
          if (nr0 !== out.r0 || nc0 !== out.c0 || nr1 !== out.r1 || nc1 !== out.c1) {
            out.r0 = nr0; out.c0 = nc0; out.r1 = nr1; out.c1 = nc1;
            grew = true;
          }
        }
      }
      if (!grew) break;
    }
    return out;
  }

  function mergeAt(row, col) {
    for (const m of state.merges) {
      if (row >= m.r0 && row <= m.r1 && col >= m.c0 && col <= m.c1) return m;
    }
    return { r0: row, c0: col, r1: row, c1: col };
  }

  function selectionText() {
    return state.ranges.map(rangeRef).join(",");
  }

  function adoptSingleOrMultiRef(str) {
    const parsed = parseSelectionString(str);
    if (!parsed.length) return;
    state.ranges = parsed;
    const last = parsed[parsed.length - 1];
    state.active = { row: last.r0, col: last.c0 };
    state.anchor = { row: last.r0, col: last.c0 };
    state.anchors = parsed.map((r) => ({ row: r.r0, col: r.c0 }));
  }

  function updateNameBox() {
    const nb = el._ch && el._ch.querySelector(".xfb-name");
    if (!nb) return;
    const txt = selectionText();
    if (nb.textContent !== txt) {
      nb.textContent = txt;
      nb.setAttribute("aria-label", "Name Box " + txt);
    }
  }

  function updateHeaderClasses() {
    const root = el._r && el._r.querySelector(".xg-root");
    if (!root) return;
    const hdrCol = root.querySelector(".xg-hdr-col");
    if (hdrCol) {
      for (const hd of hdrCol.children) {
        const c = parseInt(hd.dataset.index, 10);
        if (!Number.isFinite(c)) continue;
        let mode = 0; // 0 none, 1 part, 2 full
        for (const rg of state.ranges) {
          if (c >= rg.c0 && c <= rg.c1) {
            if (rg.r0 === 0 && rg.r1 === MAX_ROW) { mode = 2; break; }
            mode = 1;
          }
        }
        hd.classList.toggle("xg-hd-full", mode === 2);
        hd.classList.toggle("xg-hd-sel", mode === 1);
      }
    }
    const hdrRow = root.querySelector(".xg-hdr-row");
    if (hdrRow) {
      for (const hd of hdrRow.children) {
        const r = parseInt(hd.dataset.index, 10);
        if (!Number.isFinite(r)) continue;
        let mode = 0;
        for (const rg of state.ranges) {
          if (r >= rg.r0 && r <= rg.r1) {
            if (rg.c0 === 0 && rg.c1 === MAX_COL) { mode = 2; break; }
            mode = 1;
          }
        }
        hd.classList.toggle("xg-hd-full", mode === 2);
        hd.classList.toggle("xg-hd-sel", mode === 1);
      }
    }
  }

  function quadrants() {
    const { headerW: hw, headerH: hh, viewW: vw, viewH: vh, frozenW: fw, frozenH: fh, scrollX: sx, scrollY: sy } = state;
    const cw = Math.max(0, vw - hw), ch = Math.max(0, vh - hh);
    const out = [];
    if (fw > 0 && fh > 0) out.push({ x: hw, y: hh, w: Math.min(fw, cw), h: Math.min(fh, ch), sx: 0, sy: 0 });
    if (fh > 0 && cw > fw) out.push({ x: hw + fw, y: hh, w: cw - fw, h: Math.min(fh, ch), sx, sy: 0 });
    if (fw > 0 && ch > fh) out.push({ x: hw, y: hh + fh, w: Math.min(fw, cw), h: ch - fh, sx: 0, sy });
    if (cw > fw && ch > fh) out.push({ x: hw + fw, y: hh + fh, w: cw - fw, h: ch - fh, sx, sy });
    return out;
  }

  function boxInQuad(rg, q) {
    const { headerW: hw, headerH: hh } = state;
    const c0 = colRect(rg.c0);
    const r0 = rowRect(rg.r0);
    const x0 = hw + c0.left - q.sx - q.x;
    const y0 = hh + r0.top - q.sy - q.y;
    let x1;
    if (rg.c1 === MAX_COL) x1 = q.w + 8;
    else {
      const c1 = colRect(rg.c1);
      x1 = hw + c1.left + c1.width - q.sx - q.x;
    }
    let y1;
    if (rg.r1 === MAX_ROW) y1 = q.h + 8;
    else {
      const r1 = rowRect(rg.r1);
      y1 = hh + r1.top + r1.height - q.sy - q.y;
    }
    return { x0, y0, x1, y1 };
  }

  function addBox(parent, cls, l, t, w, h) {
    if (w <= 0 || h <= 0) return null;
    const d = document.createElement("div");
    d.className = cls;
    d.style.cssText = `left:${l}px;top:${t}px;width:${w}px;height:${h}px`;
    parent.appendChild(d);
    return d;
  }

  function paintOverlayNow() {
    if (!readGridMetrics() && !state.colMap.size) return;
    const qs = quadrants();
    const frag = document.createDocumentFragment();
    const activeRg = mergeAt(state.active.row, state.active.col);
    const singleRange = state.ranges.length === 1;
    const editable = el._readonly === false;

    for (const q of qs) {
      const qEl = document.createElement("div");
      qEl.className = "nx-sel-q";
      qEl.style.cssText = `left:${q.x}px;top:${q.y}px;width:${q.w}px;height:${q.h}px`;

      const aBox = boxInQuad(activeRg, q);
      let hasContent = false;

      for (let idx = 0; idx < state.ranges.length; idx++) {
        const rg = state.ranges[idx];
        const b = boxInQuad(rg, q);
        if (b.x1 <= -4 || b.x0 >= q.w + 4 || b.y1 <= -4 || b.y0 >= q.h + 4) continue;
        hasContent = true;

        const isSingleCell = rg.r0 === activeRg.r0 && rg.r1 === activeRg.r1 && rg.c0 === activeRg.c0 && rg.c1 === activeRg.c1;
        const containsActive = rg.r0 <= activeRg.r0 && rg.r1 >= activeRg.r1 && rg.c0 <= activeRg.c0 && rg.c1 >= activeRg.c1;

        // Shaded fill (with active cell left white/unshaded)
        const sx0 = Math.max(0, b.x0), sy0 = Math.max(0, b.y0);
        const sx1 = Math.min(q.w, b.x1), sy1 = Math.min(q.h, b.y1);
        if (!isSingleCell && sx1 > sx0 && sy1 > sy0) {
          if (containsActive) {
            const ax0 = Math.max(sx0, Math.min(sx1, aBox.x0));
            const ay0 = Math.max(sy0, Math.min(sy1, aBox.y0));
            const ax1 = Math.max(sx0, Math.min(sx1, aBox.x1));
            const ay1 = Math.max(sy0, Math.min(sy1, aBox.y1));
            addBox(qEl, "nx-sel-shade", sx0, sy0, sx1 - sx0, ay0 - sy0);
            addBox(qEl, "nx-sel-shade", sx0, ay0, ax0 - sx0, ay1 - ay0);
            addBox(qEl, "nx-sel-shade", ax1, ay0, sx1 - ax1, ay1 - ay0);
            addBox(qEl, "nx-sel-shade", sx0, ay1, sx1 - sx0, sy1 - ay1);
          } else {
            addBox(qEl, "nx-sel-shade", sx0, sy0, sx1 - sx0, sy1 - sy0);
          }
        }

        // Outline box (clipped to quadrant + small margin so off-screen sides stay open)
        const ol = Math.max(-4, b.x0 - 1), ot = Math.max(-4, b.y0 - 1);
        const or = Math.min(q.w + 4, b.x1), ob = Math.min(q.h + 4, b.y1);
        const isSingleOutline = singleRange && isSingleCell;
        addBox(qEl, isSingleOutline ? "nx-sel-outline xg-active" : "nx-sel-outline", ol, ot, or - ol, ob - ot);

        if (editable && singleRange && rg.kind === "cell" && b.x1 >= 0 && b.x1 <= q.w + 4 && b.y1 >= 0 && b.y1 <= q.h + 4) {
          addBox(qEl, "nx-sel-handle", b.x1 - 4, b.y1 - 4, 7, 7);
        }
      }

      // Active cell inner border when selection has >1 cell or >1 range
      const soleSingle = singleRange &&
        state.ranges[0].r0 === activeRg.r0 && state.ranges[0].r1 === activeRg.r1 &&
        state.ranges[0].c0 === activeRg.c0 && state.ranges[0].c1 === activeRg.c1;
      if (!soleSingle && aBox.x1 > -4 && aBox.x0 < q.w + 4 && aBox.y1 > -4 && aBox.y0 < q.h + 4) {
        hasContent = true;
        const al = Math.max(-4, aBox.x0 - 1), at = Math.max(-4, aBox.y0 - 1);
        const ar = Math.min(q.w + 4, aBox.x1), ab = Math.min(q.h + 4, aBox.y1);
        addBox(qEl, "nx-sel-active xg-active", al, at, ar - al, ab - at);
      }

      if (hasContent) frag.appendChild(qEl);
    }

    layer.replaceChildren(frag);
    updateHeaderClasses();
    updateNameBox();
    updateSizeTip();
  }

  function updateSizeTip() {
    if (!state.dragging) {
      tip.hidden = true;
      return;
    }
    const rg = state.ranges[state.ranges.length - 1];
    const rows = rg.r1 - rg.r0 + 1;
    const cols = rg.c1 - rg.c0 + 1;
    tip.textContent = `${rows}R × ${cols}C`;
    tip.hidden = false;
    const vr = v.getBoundingClientRect();
    const tw = tip.offsetWidth || 64;
    const th = tip.offsetHeight || 22;
    const lx = Math.max(4, Math.min(state.dragging.x - vr.left + 14, Math.max(4, vr.width - tw - 4)));
    const ly = Math.max(4, Math.min(state.dragging.y - vr.top + 18, Math.max(4, vr.height - th - 4)));
    tip.style.left = lx + "px";
    tip.style.top = ly + "px";
  }

  function schedulePaint() {
    if (state.paintRaf) return;
    state.paintRaf = requestAnimationFrame(() => {
      state.paintRaf = 0;
      paintOverlayNow();
    });
  }

  function showToast(msg) {
    clearTimeout(state.toastTimer);
    toast.textContent = msg;
    toast.hidden = false;
    state.toastTimer = setTimeout(() => { toast.hidden = true; }, 3500);
  }

  function hitTestClient(clientX, clientY) {
    const root = el._r && el._r.querySelector(".xg-root");
    if (!root) return null;
    const gr = root.getBoundingClientRect();
    const vx = clientX - gr.left, vy = clientY - gr.top;
    const { headerW: hw, headerH: hh, frozenW: fw, frozenH: fh, scrollX: sx, scrollY: sy } = state;
    const clampedX = Math.max(hw + 1, Math.min(vx, state.viewW - 2));
    const clampedY = Math.max(hh + 1, Math.min(vy, state.viewH - 2));
    const cx = Math.max(0, clampedX - hw), cy = Math.max(0, clampedY - hh);
    const planeX = cx < fw ? cx : cx + sx;
    const planeY = cy < fh ? cy : cy + sy;
    const col = colAtPlaneX(planeX);
    const row = rowAtPlaneY(planeY);

    let area = "cell";
    if (hw > 0 && hh > 0 && vx < hw && vy < hh) area = "corner";
    else if (hh > 0 && vy < hh && vx >= hw) area = "col";
    else if (hw > 0 && vx < hw && vy >= hh) area = "row";
    return { area, row, col, vx, vy, gr };
  }

  function hitsFillHandle(vx, vy) {
    if (el._readonly !== false || state.ranges.length !== 1) return false;
    const rg = state.ranges[0];
    if (rg.kind !== "cell") return false;
    for (const q of quadrants()) {
      const b = boxInQuad(rg, q);
      const hx = q.x + b.x1, hy = q.y + b.y1;
      if (Math.abs(vx - hx) <= 5 && Math.abs(vy - hy) <= 5) return true;
    }
    return false;
  }

  function applyPointerAt(clientX, clientY, mode) {
    const d = state.dragging;
    if (!d) return;
    const h = hitTestClient(clientX, clientY);
    if (!h) return;
    if (d.area === "cell") {
      const rg = expandForMerges({
        r0: Math.min(state.anchor.row, h.row),
        c0: Math.min(state.anchor.col, h.col),
        r1: Math.max(state.anchor.row, h.row),
        c1: Math.max(state.anchor.col, h.col),
        kind: "cell",
      });
      state.ranges[state.ranges.length - 1] = rg;
    } else if (d.area === "col") {
      const c0 = Math.min(state.anchor.col, h.col), c1 = Math.max(state.anchor.col, h.col);
      state.ranges[state.ranges.length - 1] = { r0: 0, c0, r1: MAX_ROW, c1, kind: "col" };
    } else if (d.area === "row") {
      const r0 = Math.min(state.anchor.row, h.row), r1 = Math.max(state.anchor.row, h.row);
      state.ranges[state.ranges.length - 1] = { r0, c0: 0, r1, c1: MAX_COL, kind: "row" };
    }
    if (mode !== "raf") schedulePaint();
  }

  function updateAutoScroll() {
    const d = state.dragging;
    if (!d) { stopAutoScroll(); return; }
    const root = el._r && el._r.querySelector(".xg-root");
    if (!root) { stopAutoScroll(); return; }
    const gr = root.getBoundingClientRect();
    const minX = gr.left + state.headerW + (d.area === "col" ? 0 : state.frozenW);
    const maxX = gr.right;
    const minY = gr.top + state.headerH + (d.area === "row" ? 0 : state.frozenH);
    const maxY = gr.bottom;

    const dxOut = d.area === "row" ? 0 : (d.x < minX ? d.x - minX : d.x > maxX ? d.x - maxX : 0);
    const dyOut = d.area === "col" ? 0 : (d.y < minY ? d.y - minY : d.y > maxY ? d.y - maxY : 0);
    if (!dxOut && !dyOut) { stopAutoScroll(); return; }
    if (state.autoRaf) return;

    const step = () => {
      state.autoRaf = 0;
      const cur = state.dragging;
      if (!cur) return;
      const r = el._r && el._r.querySelector(".xg-root")?.getBoundingClientRect();
      if (!r) return;
      const lx0 = r.left + state.headerW + (cur.area === "col" ? 0 : state.frozenW);
      const ly0 = r.top + state.headerH + (cur.area === "row" ? 0 : state.frozenH);
      const ox = cur.area === "row" ? 0 : (cur.x < lx0 ? cur.x - lx0 : cur.x > r.right ? cur.x - r.right : 0);
      const oy = cur.area === "col" ? 0 : (cur.y < ly0 ? cur.y - ly0 : cur.y > r.bottom ? cur.y - r.bottom : 0);
      if (!ox && !oy) return;

      const sx = autoScrollStep(ox);
      const sy = autoScrollStep(oy);
      const nextX = Math.max(0, state.scrollX + sx);
      const nextY = Math.max(0, state.scrollY + sy);
      const deltaX = nextX - state.scrollX;
      const deltaY = nextY - state.scrollY;
      state.scrollX = nextX;
      state.scrollY = nextY;

      applyPointerAt(cur.x, cur.y, "raf");
      paintOverlayNow();

      const doc = el.document;
      if (doc && (deltaX || deltaY) && !state.scrollTickBusy) {
        doc.pushEvent({ kind: EK.Wheel, time_ms: performance.now(), x: state.headerW + 10, y: state.headerH + 10, dx: deltaX, dy: deltaY });
        state.scrollTickBusy = true;
        Promise.resolve(origElTick()).finally(() => { state.scrollTickBusy = false; });
      }
      state.autoRaf = requestAnimationFrame(step);
    };
    state.autoRaf = requestAnimationFrame(step);
  }

  function stopAutoScroll() {
    if (state.autoRaf) {
      cancelAnimationFrame(state.autoRaf);
      state.autoRaf = 0;
    }
  }

  async function syncSelectionToWorker() {
    const doc = el.document;
    if (!doc || !doc._worker || typeof doc._request !== "function") return;
    const seq = ++state.syncSeq;
    const last = state.ranges[state.ranges.length - 1];
    const ref = rangeRef(last);
    const run = async () => {
      if (seq !== state.syncSeq) return;
      doc._events && doc._events.flushNow && doc._events.flushNow();
      const msg = await doc._request("sa", () => doc._worker.postMessage({ type: "sa", op: "select", arg: ref }));
      if (seq !== state.syncSeq) return;
      try {
        const r = JSON.parse(msg.result || "{}");
        if (el._ch && (r.h != null || r.hf != null)) applyChromePatch(el._ch, r);
        updateNameBox();
      } catch {}
    };
    state.syncPending = state.syncPending.then(run, run);
    return state.syncPending;
  }

  // ---- pointer handlers (capture on v, clear suppressViewer on sr bubble) ----
  v.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || !el.document) return;
    wrapDoc(el.document);
    if (sr.querySelector(".xfb-editing")) return;
    if (e.target && e.target.closest && e.target.closest(".xg-obj")) return;
    readGridMetrics();
    const h = hitTestClient(e.clientX, e.clientY);
    if (!h) return;
    if (hitsFillHandle(h.vx, h.vy)) return;

    suppressViewer = true;
    const ctrl = (e.ctrlKey || e.metaKey) && !e.altKey;
    const shift = e.shiftKey && !ctrl;

    if (h.area === "corner") {
      state.active = { row: 0, col: 0 };
      state.anchor = { row: 0, col: 0 };
      state.ranges = [{ r0: 0, c0: 0, r1: MAX_ROW, c1: MAX_COL, kind: "all" }];
      state.anchors = [{ row: 0, col: 0 }];
      state.dragging = null;
      schedulePaint();
      return;
    }

    if (h.area === "col") {
      if (shift) {
        const ac = state.anchor.col;
        const rg = { r0: 0, c0: Math.min(ac, h.col), r1: MAX_ROW, c1: Math.max(ac, h.col), kind: "col" };
        state.ranges[state.ranges.length - 1] = rg;
      } else {
        const firstVisRow = rowAtPlaneY(state.scrollY + 1);
        const rg = { r0: 0, c0: h.col, r1: MAX_ROW, c1: h.col, kind: "col" };
        state.active = { row: firstVisRow, col: h.col };
        state.anchor = { row: 0, col: h.col };
        if (ctrl) {
          state.ranges.push(rg);
          state.anchors.push({ ...state.active });
        } else {
          state.ranges = [rg];
          state.anchors = [{ ...state.active }];
        }
      }
      state.dragging = { area: "col", pointerId: e.pointerId, ctrl, x: e.clientX, y: e.clientY };
      try { v.setPointerCapture(e.pointerId); } catch {}
      schedulePaint();
      return;
    }

    if (h.area === "row") {
      if (shift) {
        const ar = state.anchor.row;
        const rg = { r0: Math.min(ar, h.row), c0: 0, r1: Math.max(ar, h.row), c1: MAX_COL, kind: "row" };
        state.ranges[state.ranges.length - 1] = rg;
      } else {
        const firstVisCol = colAtPlaneX(state.scrollX + 1);
        const rg = { r0: h.row, c0: 0, r1: h.row, c1: MAX_COL, kind: "row" };
        state.active = { row: h.row, col: firstVisCol };
        state.anchor = { row: h.row, col: 0 };
        if (ctrl) {
          state.ranges.push(rg);
          state.anchors.push({ ...state.active });
        } else {
          state.ranges = [rg];
          state.anchors = [{ ...state.active }];
        }
      }
      state.dragging = { area: "row", pointerId: e.pointerId, ctrl, x: e.clientX, y: e.clientY };
      try { v.setPointerCapture(e.pointerId); } catch {}
      schedulePaint();
      return;
    }

    // Cell area
    if (shift) {
      const rg = expandForMerges({
        r0: Math.min(state.anchor.row, h.row),
        c0: Math.min(state.anchor.col, h.col),
        r1: Math.max(state.anchor.row, h.row),
        c1: Math.max(state.anchor.col, h.col),
        kind: "cell",
      });
      state.ranges[state.ranges.length - 1] = rg;
    } else {
      const rg = expandForMerges({ r0: h.row, c0: h.col, r1: h.row, c1: h.col, kind: "cell" });
      state.active = { row: h.row, col: h.col };
      state.anchor = { row: h.row, col: h.col };
      if (ctrl) {
        state.ranges.push(rg);
        state.anchors.push({ row: h.row, col: h.col });
      } else {
        state.ranges = [rg];
        state.anchors = [{ row: h.row, col: h.col }];
      }
    }
    state.dragging = { area: "cell", pointerId: e.pointerId, ctrl, x: e.clientX, y: e.clientY };
    try { v.setPointerCapture(e.pointerId); } catch {}
    schedulePaint();
  }, true);

  v.addEventListener("pointermove", (e) => {
    if (!state.dragging) return;
    suppressViewer = true;
    state.dragging.x = e.clientX;
    state.dragging.y = e.clientY;
    applyPointerAt(e.clientX, e.clientY, "move");
    updateAutoScroll();
  }, true);

  const endDrag = (e) => {
    if (suppressViewer && e.type === "pointerup") {
      // Corner click also suppressed ViewerElement on pointerdown; sync to worker.
      void syncSelectionToWorker();
    }
    if (!state.dragging) return;
    suppressViewer = true;
    stopAutoScroll();
    try { v.releasePointerCapture(state.dragging.pointerId); } catch {}
    state.dragging = null;
    schedulePaint();
    void syncSelectionToWorker();
  };
  v.addEventListener("pointerup", endDrag, true);
  v.addEventListener("pointercancel", endDrag, true);

  const clearSuppress = () => { suppressViewer = false; };
  sr.addEventListener("pointerdown", clearSuppress, false);
  sr.addEventListener("pointermove", clearSuppress, false);
  sr.addEventListener("pointerup", clearSuppress, false);
  sr.addEventListener("pointercancel", clearSuppress, false);

  // ---- Keyboard navigation, Delete, and Ctrl+A sync ----
  function moveArrow(key, shift) {
    const step = key === "ArrowLeft" ? [0, -1] : key === "ArrowRight" ? [0, 1] : key === "ArrowUp" ? [-1, 0] : [1, 0];
    const cur = state.ranges[state.ranges.length - 1] || { r0: 0, c0: 0, r1: 0, c1: 0, kind: "cell" };
    if (shift) {
      const endR = cur.r0 === state.anchor.row ? cur.r1 : cur.r0;
      const endC = cur.c0 === state.anchor.col ? cur.c1 : cur.c0;
      const nr = Math.max(0, Math.min(MAX_ROW, endR + step[0]));
      const nc = Math.max(0, Math.min(MAX_COL, endC + step[1]));
      state.ranges[state.ranges.length - 1] = expandForMerges({
        r0: Math.min(state.anchor.row, nr),
        c0: Math.min(state.anchor.col, nc),
        r1: Math.max(state.anchor.row, nr),
        c1: Math.max(state.anchor.col, nc),
        kind: "cell",
      });
      revealCell(nr, nc);
    } else {
      const m = mergeAt(state.active.row, state.active.col);
      const baseR = step[0] > 0 ? m.r1 : step[0] < 0 ? m.r0 : state.active.row;
      const baseC = step[1] > 0 ? m.c1 : step[1] < 0 ? m.c0 : state.active.col;
      const nr = Math.max(0, Math.min(MAX_ROW, baseR + step[0]));
      const nc = Math.max(0, Math.min(MAX_COL, baseC + step[1]));
      state.active = { row: nr, col: nc };
      state.anchor = { row: nr, col: nc };
      state.ranges = [expandForMerges({ r0: nr, c0: nc, r1: nr, c1: nc, kind: "cell" })];
      state.anchors = [{ row: nr, col: nc }];
      revealCell(nr, nc);
    }
    schedulePaint();
    void syncSelectionToWorker();
  }

  function revealCell(row, col) {
    readGridMetrics();
    const cr = colRect(col), rr = rowRect(row);
    const viewColsW = Math.max(0, state.viewW - state.headerW - state.frozenW);
    const viewRowsH = Math.max(0, state.viewH - state.headerH - state.frozenH);
    let dx = 0, dy = 0;
    if (col >= state.frozenCols) {
      const x = cr.left - state.frozenW;
      if (x < state.scrollX) dx = x - state.scrollX;
      else if (x + cr.width > state.scrollX + viewColsW) dx = Math.min(x, x + cr.width - viewColsW) - state.scrollX;
    }
    if (row >= state.frozenRows) {
      const y = rr.top - state.frozenH;
      if (y < state.scrollY) dy = y - state.scrollY;
      else if (y + rr.height > state.scrollY + viewRowsH) dy = Math.min(y, y + rr.height - viewRowsH) - state.scrollY;
    }
    if (dx || dy) {
      state.scrollX = Math.max(0, state.scrollX + dx);
      state.scrollY = Math.max(0, state.scrollY + dy);
      const doc = el.document;
      if (doc) {
        doc.pushEvent({ kind: EK.Wheel, time_ms: performance.now(), x: state.headerW + 10, y: state.headerH + 10, dx, dy });
        void origElTick();
      }
    }
  }

  async function clearRanges(ranges) {
    const doc = el.document;
    if (!doc || el._readonly !== false || typeof doc.applyEdit !== "function") return;
    await state.syncPending;
    const sheet = state.sheet;
    let lastPatch = null;
    for (const rg of ranges) {
      const rows = rg.r1 - rg.r0 + 1, cols = rg.c1 - rg.c0 + 1;
      if (rows * cols <= CLEAR_CAP) {
        const empty = Array.from({ length: rows }, () => Array(cols).fill(""));
        lastPatch = await doc.applyEdit(JSON.stringify({ command: "setCellBlock", sheet, row: rg.r0, col: rg.c0, rows: empty }));
      } else {
        // Large/whole-col/whole-row selection: clear up to CLEAR_CAP non-null cells reported by host-selection
        const msg = await doc._request("sa", () => doc._worker.postMessage({ type: "sa", op: "host-selection", arg: `${sheet}\t${rangeRef(rg)}` }));
        const data = JSON.parse(msg.result || "{}");
        const vals = data.values || [];
        for (let dr = 0; dr < vals.length; dr++) {
          for (let dc = 0; dc < vals[dr].length; dc++) {
            if (vals[dr][dc] !== null) {
              lastPatch = await doc.applyEdit(JSON.stringify({ command: "setCellInput", sheet, row: rg.r0 + dr, col: rg.c0 + dc, text: "" }));
            }
          }
        }
        if (data.truncated) showToast("Cleared cells within the 10,000-cell viewport cap.");
      }
    }
    if (lastPatch && el._ch) {
      try { applyChromePatch(el._ch, JSON.parse(lastPatch)); } catch {}
    }
    updateNameBox();
    doc._nxPainted = 0;
    origElTick();
  }

  sr.addEventListener("keydown", (e) => {
    if (!el.document) return;
    if (sr.querySelector(".xfb-editing")) return;
    if (e.target && e.target.classList && e.target.classList.contains("q")) return;
    wrapDoc(el.document);
    if (!e.ctrlKey && !e.metaKey && !e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight" || e.key === "ArrowUp" || e.key === "ArrowDown")) {
      e.preventDefault();
      e.stopPropagation();
      suppressViewer = true;
      moveArrow(e.key, e.shiftKey);
      queueMicrotask(clearSuppress);
      return;
    }
    if (!e.ctrlKey && !e.metaKey && !e.altKey && (e.key === "Delete" || e.key === "Backspace")) {
      if (el._readonly === false) {
        e.preventDefault();
        e.stopPropagation();
        void clearRanges(state.ranges);
      }
      return;
    }
    if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && (e.key === "a" || e.key === "A")) {
      e.preventDefault();
      e.stopPropagation();
      const doc = el.document;
      if (!doc || !doc._worker) return;
      doc.pushEvent({
        kind: EK.Keyboard,
        time_ms: performance.now(),
        key_id: e.key === "A" ? 6 : 5,
        code_id: 0,
        type: 0,
        ctrlKey: !!e.ctrlKey,
        metaKey: !!e.metaKey,
        shiftKey: false,
        altKey: false,
      });
      doc._events && doc._events.flushNow && doc._events.flushNow();
      void doc._request("sa", () => doc._worker.postMessage({ type: "sa", op: "ui", arg: "" })).then((msg) => {
        try {
          const r = JSON.parse(msg.result || "{}");
          if (r.ui && r.ui.selection) {
            adoptSingleOrMultiRef(r.ui.selection);
            schedulePaint();
          }
        } catch {}
      });
    }
  }, true);
  sr.addEventListener("keydown", clearSuppress, false);
  sr.addEventListener("cut", (e) => {
    if (state.ranges.length > 1 && !sr.querySelector(".xfb-editing")) {
      e.preventDefault();
      e.stopPropagation();
      showToast("That command cannot be used on multiple selections.");
    }
  }, true);

  // ---- Multi-range shellAction commands (Bold, numFmt, Clear, Copy, Cut) ----
  const origShell = el.shellAction.bind(el);
  function alignedCopyCheck(ranges) {
    if (ranges.length <= 1) return "single";
    const sameRows = ranges.every((r) => r.r0 === ranges[0].r0 && r.r1 === ranges[0].r1);
    if (sameRows) return "rows";
    const sameCols = ranges.every((r) => r.c0 === ranges[0].c0 && r.c1 === ranges[0].c1);
    if (sameCols) return "cols";
    return null;
  }

  el.shellAction = async (op, arg = "") => {
    await state.syncPending;
    if (op === "cmd" && arg === "home.clear-contents") {
      await clearRanges(state.ranges);
      return { ui: null };
    }
    if ((op === "copy" || (op === "cmd" && arg === "home.copy")) && state.ranges.length > 1) {
      const align = alignedCopyCheck(state.ranges);
      if (!align) {
        showToast("That command cannot be used on multiple selections.");
        return { ui: null, c: null };
      }
      // Aligned multi-range copy: fetch TSV per range and combine cleanly
      const parts = [];
      for (const rg of state.ranges) {
        await origShell("select", rangeRef(rg));
        const r = await origShell("copy");
        parts.push((r && r.c) || "");
      }
      await origShell("select", rangeRef(state.ranges[state.ranges.length - 1]));
      const text = align === "rows"
        ? parts.map((p) => p.split("\n")).reduce((acc, rows) => acc.map((line, i) => line + "\t" + (rows[i] ?? "")))
            .join("\n")
        : parts.join("\n");
      navigator.clipboard && navigator.clipboard.writeText && navigator.clipboard.writeText(text);
      el.dispatchEvent(new CustomEvent("nitro-copy", { detail: text, bubbles: true }));
      updateNameBox();
      schedulePaint();
      return { ui: null, c: text };
    }
    if (op === "cmd" && arg === "home.cut" && state.ranges.length > 1) {
      showToast("That command cannot be used on multiple selections.");
      return { ui: null };
    }
    if (op === "cmd" && state.ranges.length > 1) {
      if (STRUCT_CMDS.has(arg)) {
        showToast("That command cannot be used on multiple selections.");
        return { ui: null };
      }
      if (FORMAT_CMDS.has(arg)) {
        let lastRes = { ui: null };
        for (const rg of state.ranges) {
          await origShell("select", rangeRef(rg));
          lastRes = await origShell("cmd", arg);
        }
        await origShell("select", rangeRef(state.ranges[state.ranges.length - 1]));
        if (el.document) el.document._nxPainted = 0;
        origElTick();
        updateNameBox();
        schedulePaint();
        return lastRes;
      }
    }
    const res = await origShell(op, arg);
    if (op === "cmd" && el.document) {
      el.document._nxPainted = 0;
      origElTick();
    }
    if (op === "sheet" || op === "zoom-in" || op === "zoom-out" || op === "zoom-set") {
      state.colMap.clear();
      state.rowMap.clear();
      if (op === "sheet" && res && res.ui && res.ui.selection) adoptSingleOrMultiRef(res.ui.selection);
      schedulePaint();
    }
    updateNameBox();
    return res;
  };

  // Observe `.dom` replacements (scroll, zoom, edit, sheet switch) to keep overlay glued
  const mo = new MutationObserver(() => {
    readGridMetrics();
    paintOverlayNow();
  });
  mo.observe(el._r, { childList: true });

  // Observe `.chrome` patches so Name Box keeps normalized header/multi-range refs
  const chMo = new MutationObserver(() => {
    if (!sr.querySelector(".xfb-editing")) updateNameBox();
  });
  if (el._ch) chMo.observe(el._ch, { childList: true, subtree: true });

  const ctrlObj = {
    getRangeRefs: () => state.ranges.map(rangeRef),
    anchorOf: (idx) => {
      const a = state.anchors[idx];
      if (a) return formatAddr(a.row, a.col);
      const rg = state.ranges[idx];
      return rg ? formatAddr(rg.r0, rg.c0) : null;
    },
    sync: () => syncSelectionToWorker(),
    onDocAttach: () => {
      if (el.document && el.document !== state.lastDoc) {
        state.lastDoc = el.document;
        wrapDoc(el.document);
        state.ranges = [{ r0: 0, c0: 0, r1: 0, c1: 0, kind: "cell" }];
        state.active = { row: 0, col: 0 };
        state.anchor = { row: 0, col: 0 };
        state.anchors = [{ row: 0, col: 0 }];
        state.colMap.clear();
        state.rowMap.clear();
        state.merges = [];
        schedulePaint();
      }
    },
  };
  el._nxSel = ctrlObj;
  ctrlObj.onDocAttach();
}
