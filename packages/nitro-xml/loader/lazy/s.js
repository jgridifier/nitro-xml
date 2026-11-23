// SPDX-License-Identifier: MIT
// getSelection() (#561). Lazy chunk: off the first-load budget.
// - xlsx: range from the shell UI, typed cell values from the engine ("host-selection").
// - docx: caret or text range; pptx: clicked shape (collapsed) or text range in a text box.
//   Text-range selection uses el._nxSel set by docx-caret.js / pptx-caret.js.
// Host reads go straight to the worker like host-position: they never paint, edit or unlock
// editor chrome, so getSelection() cannot mark the document dirty.
import { rangeOf } from "./input-proxy.js";

const VALUES_CAP = 10000;

function hostAction(el, op, arg = "") {
  const d = el.document;
  if (!d) return Promise.resolve({});
  if (!d._worker || typeof d._request !== "function") return Promise.resolve(d.shellAction(op, arg));
  d._events && d._events.flushNow && d._events.flushNow();
  return d
    ._request("sa", () => d._worker.postMessage({ type: "sa", op, arg: String(arg) }))
    .then((m) => JSON.parse(m.result || "{}"));
}

async function xlsxSel(el) {
  const r = await el.shellAction("ui");
  const ui = (r && r.ui) || {};
  const tab = (ui.tabs || []).find((t) => t.active) || (ui.tabs || [])[0] || null;
  const rangeStr = ui.selection || null;
  const rg = rangeOf(rangeStr || "");
  const anchor = rangeStr ? rangeStr.split(":")[0] : null;
  const sheetIndex = tab ? tab.index : 0;
  const count = rg ? (rg.r1 - rg.r0 + 1) * (rg.c1 - rg.c0 + 1) : 0;
  let values = [];
  let truncated = count > VALUES_CAP;
  if (rg) {
    const v = await hostAction(el, "host-selection", `${sheetIndex}\t${rangeStr}`);
    values = v.values || [];
    truncated = !!v.truncated;
  }
  return { format: "xlsx", sheet: tab ? tab.name : "", sheetIndex, range: rangeStr, anchor, values, truncated };
}

async function docxSel(el) {
  const [cr, ur] = await Promise.all([
    hostAction(el, "docx-caret", ""),
    el.shellAction("ui"),
  ]);
  const k = cr && cr.k;
  const ui = (ur && ur.ui) || {};
  const tabs = ui.tabs || [];
  const tab = tabs.find((t) => t.active) || tabs[0] || null;
  const page = tab ? tab.index : 0;
  if (!k) return { format: "docx", text: "", page, start: null, end: null, collapsed: true };
  const [bid, chars] = k;
  const sr = el.shadowRoot;
  const dom = sr && sr.querySelector(".dom");
  const bidToIdx = (b) => {
    if (!dom) return 0;
    const seen = new Set();
    const uniq = [];
    for (const el of dom.querySelectorAll("[data-bid]")) {
      if (!seen.has(el.dataset.bid)) { seen.add(el.dataset.bid); uniq.push(el.dataset.bid); }
    }
    const idx = uniq.indexOf(String(b));
    return idx >= 0 ? idx : 0;
  };
  const caretParaIdx = bidToIdx(bid);
  const caretPos = { paragraph: caretParaIdx, run: 0, offset: chars };

  // Non-collapsed selection set by docx-caret.js
  const selState = el._nxSel;
  if (selState && selState.anchorK) {
    const [abid, achars] = selState.anchorK;
    const anchorParaIdx = bidToIdx(abid);
    const anchorPos = { paragraph: anchorParaIdx, run: 0, offset: achars };
    const isForward = anchorParaIdx < caretParaIdx ||
      (anchorParaIdx === caretParaIdx && achars <= chars);
    const start = isForward ? anchorPos : caretPos;
    const end = isForward ? caretPos : anchorPos;
    const collapsed = start.paragraph === end.paragraph && start.offset === end.offset;
    if (!collapsed) {
      return { format: "docx", text: selState.text || "", page, start, end, collapsed: false };
    }
  }

  // Caret only
  return { format: "docx", text: "", page, start: caretPos, end: { ...caretPos }, collapsed: true };
}

async function pptxSel(el) {
  const r = await el.shellAction("ui");
  const ui = (r && r.ui) || {};
  const tabs = ui.tabs || [];
  const tab = tabs.find((t) => t.active) || tabs[0] || null;
  const slide = tab ? tab.index : 0;
  // A multi-shape selection reports its first shape; "A1" means nothing is selected.
  const id = ui.selection && ui.selection !== "A1" ? ui.selection.split(",")[0] : null;
  const s = id ? await hostAction(el, "host-selection", id) : {};
  if (!s.shapeId) return { format: "pptx", slide, shapeId: null, shapeName: null, text: null, textRange: null, collapsed: true };

  // Check for non-collapsed text-range selection set by pptx-caret.js.
  // Use JS-side anchorPos/caretPos directly (available without a WASM round-trip).
  const selState = el._nxPptxSel;
  if (selState && selState.text && selState.anchorPos && selState.caretPos) {
    const ap = selState.anchorPos;
    const cp = selState.caretPos;
    if (ap.shapeId === cp.shapeId && ap.offset !== cp.offset) {
      const isForward = ap.offset <= cp.offset;
      const start = { paragraph: 0, run: 0, offset: isForward ? ap.offset : cp.offset };
      const end = { paragraph: 0, run: 0, offset: isForward ? cp.offset : ap.offset };
      return {
        format: "pptx", slide, shapeId: s.shapeId, shapeName: s.shapeName || null,
        text: s.shapeText || "", textRange: { text: selState.text, start, end },
        collapsed: false,
      };
    }
  }

  return {
    format: "pptx", slide, shapeId: s.shapeId, shapeName: s.shapeName || null,
    text: s.shapeText || "", textRange: null, collapsed: true,
  };
}

async function getSel(el) {
  const n = el.localName;
  if (n === "nitro-xlsx") return xlsxSel(el);
  if (n === "nitro-docx") return docxSel(el);
  if (n === "nitro-pptx") return pptxSel(el);
  return null;
}

export async function attach(el) {
  el.getSelection = () => getSel(el);
  if (!el._readonly) await (await import("./dh.js")).attach(el);
}
