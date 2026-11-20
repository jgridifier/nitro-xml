// SPDX-License-Identifier: MIT
// getSelection() (#561). Lazy chunk: off the first-load budget.
// - xlsx: range from the shell UI, typed cell values from the engine ("host-selection").
// - docx / pptx: a double-click selects the word under the pointer (Word / PowerPoint behaviour);
//   offsets are code points into the paragraph (docx) or shape text with "\n" between
//   paragraphs (pptx). A single click, a key or any change to the selected text clears it.
// Host reads go straight to the worker like host-position: they never paint, edit or unlock
// editor chrome, so getSelection() cannot mark the document dirty.
import { rangeOf } from "./input-proxy.js";

const VALUES_CAP = 10000;
const WORD = /[\p{L}\p{N}\p{M}_'\u2019]/u;

function esc(s) {
  return window.CSS && CSS.escape ? CSS.escape(s) : s.replace(/["\\]/g, "\\$&");
}

/** Code points in `s` (offsets match the engine's char counts, not UTF-16 units). */
export const cpLen = (s) => [...s].length;

/** [start, end) UTF-16 range of the word (or single non-word char) at UTF-16 index `i`. */
export function wordAt(text, i) {
  if (!text || i < 0 || i >= text.length) return null;
  if (typeof Intl !== "undefined" && Intl.Segmenter) {
    for (const s of new Intl.Segmenter(undefined, { granularity: "word" }).segment(text)) {
      if (i >= s.index && i < s.index + s.segment.length) return [s.index, s.index + s.segment.length];
    }
  }
  const cp = text.codePointAt(i) > 0xffff ? 2 : 1;
  if (!WORD.test(text.slice(i, i + cp))) return [i, i + cp];
  let a = i;
  let b = i;
  while (a > 0 && WORD.test(text[a - 1])) a--;
  while (b < text.length && WORD.test(text[b])) b++;
  return [a, b];
}

/** Text nodes of `parts` in order, `sep` between parts; returns { text, nodes:[[node, start]] }. */
export function flatten(parts, sep) {
  let text = "";
  const nodes = [];
  parts.forEach((p, n) => {
    if (n && sep) text += sep;
    const w = p.ownerDocument.createTreeWalker(p, 4);
    for (let t = w.nextNode(); t; t = w.nextNode()) {
      nodes.push([t, text.length]);
      text += t.data;
    }
  });
  return { text, nodes };
}

/** UTF-16 index of the character of a text node under (x, y), or null. */
function charAt(node, x, y) {
  const r = node.ownerDocument.createRange();
  let best = null;
  let bestD = Infinity;
  for (let i = 0; i < node.data.length; i++) {
    r.setStart(node, i);
    r.setEnd(node, i + 1);
    for (const b of r.getClientRects()) {
      if (y < b.top || y >= b.bottom) continue;
      if (x >= b.left && x < b.right) return i;
      const d = Math.min(Math.abs(x - b.left), Math.abs(x - b.right));
      if (d < bestD) { bestD = d; best = i; }
    }
  }
  return best;
}

function hostAction(el, op, arg = "") {
  const d = el.document;
  if (!d) return Promise.resolve({});
  if (!d._worker || typeof d._request !== "function") return Promise.resolve(d.shellAction(op, arg));
  d._events && d._events.flushNow && d._events.flushNow();
  return d
    ._request("sa", () => d._worker.postMessage({ type: "sa", op, arg: String(arg) }))
    .then((m) => JSON.parse(m.result || "{}"));
}

/** The parts and separator that make up one docx paragraph / pptx shape's text. */
function partsOf(sr, docx, key) {
  if (docx) return { parts: [...sr.querySelectorAll(`.dom [data-bid="${esc(key)}"]`)], sep: "" };
  const shape = sr.querySelector(`.dom [data-element-id="${esc(key)}"]`);
  return { parts: shape ? [...shape.querySelectorAll(".pptxv-para")] : [], sep: "\n" };
}

function onDblclick(el, e) {
  el._hs = null;
  const sr = el.shadowRoot;
  const docx = el.localName === "nitro-docx";
  const sel = docx ? "[data-bid]" : "[data-element-id] .pptxv-para";
  const hit = sr.elementsFromPoint(e.clientX, e.clientY).find((n) => n.closest(sel) && n.textContent);
  const owner = hit && (docx ? hit.closest("[data-bid]") : hit.closest("[data-element-id]"));
  if (!owner) return;
  const key = docx ? owner.dataset.bid : owner.dataset.elementId;
  const { parts, sep } = partsOf(sr, docx, key);
  const { text, nodes } = flatten(parts, sep);
  for (const [node, base] of nodes) {
    if (!hit.contains(node)) continue;
    const i = charAt(node, e.clientX, e.clientY);
    if (i == null) continue;
    const w = wordAt(text, base + i);
    if (w) el._hs = { key, start: w[0], end: w[1], all: text, text: text.slice(w[0], w[1]) };
    return;
  }
}

/** The double-click selection if its paragraph / shape text is unchanged, else null. */
function liveHs(el, docx) {
  const hs = el._hs;
  if (!hs) return null;
  const { parts, sep } = partsOf(el.shadowRoot, docx, hs.key);
  if (flatten(parts, sep).text !== hs.all) return (el._hs = null);
  return hs;
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
  const hs = liveHs(el, true);
  if (!k && !hs) return { format: "docx", text: "", page, start: null, end: null, collapsed: true };
  const bid = hs ? hs.key : k[0];
  const sr = el.shadowRoot;
  const dom = sr && sr.querySelector(".dom");
  let paragraph = 0;
  let text = "";
  if (dom) {
    const seen = new Set();
    const uniq = [];
    for (const b of dom.querySelectorAll("[data-bid]")) {
      if (!seen.has(b.dataset.bid)) { seen.add(b.dataset.bid); uniq.push(b.dataset.bid); }
    }
    const idx = uniq.indexOf(String(bid));
    if (idx >= 0) paragraph = idx;
    const block = dom.querySelector(`[data-bid="${esc(bid)}"]`);
    if (block) text = block.textContent || "";
  }
  if (hs) {
    // Rendered text can carry a list label ahead of the model text (same rule as the caret).
    const shown = cpLen(hs.all);
    const label = k && String(k[0]) === String(bid) ? Math.max(0, shown - k[2]) : 0;
    const a = Math.max(0, cpLen(hs.all.slice(0, hs.start)) - label);
    const b = Math.max(0, cpLen(hs.all.slice(0, hs.end)) - label);
    return {
      format: "docx", text: hs.text, page,
      start: { paragraph, run: 0, offset: a }, end: { paragraph, run: 0, offset: b }, collapsed: a === b,
    };
  }
  const pos = { paragraph, run: 0, offset: k[1] };
  return { format: "docx", text, page, start: pos, end: pos, collapsed: true };
}

async function pptxSel(el) {
  const r = await el.shellAction("ui");
  const ui = (r && r.ui) || {};
  const tabs = ui.tabs || [];
  const tab = tabs.find((t) => t.active) || tabs[0] || null;
  const slide = tab ? tab.index : 0;
  const hs = liveHs(el, false);
  // A multi-shape selection reports its first shape; "A1" means nothing is selected.
  const sel = ui.selection && ui.selection !== "A1" ? ui.selection.split(",")[0] : null;
  const id = hs ? hs.key : sel;
  const s = id ? await hostAction(el, "host-selection", id) : {};
  if (!s.shapeId) return { format: "pptx", slide, shapeId: null, shapeName: null, text: null, textRange: null };
  return {
    format: "pptx", slide, shapeId: s.shapeId, shapeName: s.shapeName || null,
    text: hs ? hs.text : s.shapeText || "",
    textRange: hs ? { start: cpLen(hs.all.slice(0, hs.start)), end: cpLen(hs.all.slice(0, hs.end)) } : null,
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
  const v = el.shadowRoot && el.shadowRoot.querySelector(".v");
  if (v && !el._hsOn && (el.localName === "nitro-docx" || el.localName === "nitro-pptx")) {
    el._hsOn = 1;
    v.addEventListener("pointerdown", (e) => { if (e.detail < 2) el._hs = null; }, true);
    el.shadowRoot.addEventListener("keydown", () => (el._hs = null), true);
    v.addEventListener("dblclick", (e) => e.button === 0 && onDblclick(el, e));
  }
  if (!el._readonly) await (await import("./dh.js")).attach(el);
}
