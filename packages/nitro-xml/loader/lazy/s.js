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

/** Map a paragraph code-point offset to `{ run, offset }` (run-local code points).
 *  At a run boundary, range starts attach to the following run; range ends and
 *  collapsed carets attach to the preceding run. */
export function charOffsetToRunPos(charOff, runLens, isRangeStart = false) {
  if (!Array.isArray(runLens) || !runLens.length) {
    return { run: 0, offset: Math.max(0, charOff) };
  }
  let lastNonEmpty = -1;
  for (let i = 0; i < runLens.length; i++) {
    if ((runLens[i] || 0) > 0) lastNonEmpty = i;
  }
  if (lastNonEmpty < 0 || (!isRangeStart && charOff <= 0)) {
    return { run: 0, offset: 0 };
  }
  let acc = 0;
  for (let i = 0; i < runLens.length; i++) {
    const len = runLens[i] || 0;
    if (len > 0) {
      if (isRangeStart ? charOff < acc + len : charOff <= acc + len) {
        return { run: i, offset: Math.max(0, charOff - acc) };
      }
      acc += len;
    }
  }
  return { run: lastNonEmpty, offset: runLens[lastNonEmpty] || 0 };
}

/** Map a flat pptx shape code-point offset (paragraphs joined by '\n') to
 *  `{ paragraph, run, offset }`. */
export function shapeOffsetToParaRunPos(offset, paragraphs, isRangeStart = false) {
  if (!Array.isArray(paragraphs) || !paragraphs.length) {
    return { paragraph: 0, run: 0, offset: Math.max(0, offset) };
  }
  let acc = 0;
  for (let pi = 0; pi < paragraphs.length; pi++) {
    const runs = paragraphs[pi] || [];
    let pLen = 0;
    for (const n of runs) pLen += n || 0;
    if (offset <= acc + pLen || pi === paragraphs.length - 1) {
      const { run, offset: rOff } = charOffsetToRunPos(
        Math.max(0, offset - acc),
        runs,
        isRangeStart,
      );
      return { paragraph: pi, run, offset: rOff };
    }
    acc += pLen + 1; // '\n' separator between paragraphs
  }
  return { paragraph: 0, run: 0, offset: Math.max(0, offset) };
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
    for (const node of dom.querySelectorAll("[data-bid]")) {
      if (!seen.has(node.dataset.bid)) { seen.add(node.dataset.bid); uniq.push(node.dataset.bid); }
    }
    const idx = uniq.indexOf(String(b));
    return idx >= 0 ? idx : 0;
  };
  const caretParaIdx = bidToIdx(bid);
  const caretRO = charOffsetToRunPos(chars, cr.runs, false);
  const caretPos = { paragraph: caretParaIdx, ...caretRO };

  // Non-collapsed selection set by docx-caret.js
  const selState = el._nxSel;
  if (selState && selState.anchorK) {
    const [abid, achars] = selState.anchorK;
    const anchorParaIdx = bidToIdx(abid);
    // Prefer caretK from selState over WASM caret (accurate for undo-restored selections).
    const ck = selState.caretK || k;
    const [cbid, cchars] = ck;
    const selCaretParaIdx = bidToIdx(cbid);
    const collapsed = anchorParaIdx === selCaretParaIdx && achars === cchars;
    if (!collapsed) {
      let aRuns = selState.anchorRuns || (abid === bid ? cr.runs : null);
      let cRuns = selState.caretRuns || (cbid === bid ? cr.runs : null);
      if (!aRuns) aRuns = (await hostAction(el, "host-selection", abid)).runs || null;
      if (!cRuns) {
        cRuns = cbid === abid ? aRuns : ((await hostAction(el, "host-selection", cbid)).runs || null);
      }
      const isForward = selCaretParaIdx > anchorParaIdx ||
        (selCaretParaIdx === anchorParaIdx && achars <= cchars);
      const startPara = isForward ? anchorParaIdx : selCaretParaIdx;
      const startChars = isForward ? achars : cchars;
      const startRuns = isForward ? aRuns : cRuns;
      const endPara = isForward ? selCaretParaIdx : anchorParaIdx;
      const endChars = isForward ? cchars : achars;
      const endRuns = isForward ? cRuns : aRuns;
      const start = { paragraph: startPara, ...charOffsetToRunPos(startChars, startRuns, true) };
      const end = { paragraph: endPara, ...charOffsetToRunPos(endChars, endRuns, false) };
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
  // Check for non-collapsed text-range selection set by pptx-caret.js.
  // Use JS-side anchorPos/caretPos directly (available without a WASM round-trip).
  const selState = el._nxPptxSel;
  // A multi-shape selection reports its first shape; "A1" means nothing is selected.
  // Fall back to the active text-selection shapeId (e.g. overflowing/zero-height shapes).
  const id = (ui.selection && ui.selection !== "A1" ? ui.selection.split(",")[0] : null) ||
    (selState && selState.anchorPos && selState.anchorPos.shapeId) || null;
  const s = id ? await hostAction(el, "host-selection", id) : {};
  if (!s.shapeId) return { format: "pptx", slide, shapeId: null, shapeName: null, text: null, textRange: null, collapsed: true };

  if (selState && selState.text && selState.anchorPos && selState.caretPos) {
    const ap = selState.anchorPos;
    const cp = selState.caretPos;
    if (ap.shapeId === cp.shapeId && ap.offset !== cp.offset) {
      const from = Math.min(ap.offset, cp.offset);
      const to = Math.max(ap.offset, cp.offset);
      const start = shapeOffsetToParaRunPos(from, s.paragraphs, true);
      const end = shapeOffsetToParaRunPos(to, s.paragraphs, false);
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
