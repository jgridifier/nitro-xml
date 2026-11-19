// SPDX-License-Identifier: MIT
import { rangeOf } from "./input-proxy.js";

const VALUES_CAP = 10000;

function esc(s) {
  return window.CSS && CSS.escape ? CSS.escape(s) : s.replace(/["\\]/g, "\\$&");
}

async function xlsxSel(el) {
  const r = await el.shellAction("ui");
  const ui = (r && r.ui) || {};
  const tab = (ui.tabs || []).find((t) => t.active) || (ui.tabs || [])[0] || null;
  const rangeStr = ui.selection || null;
  const rg = rangeOf(rangeStr || "");
  const anchor = rangeStr ? rangeStr.split(":")[0] : null;
  const count = rg ? (rg.r1 - rg.r0 + 1) * (rg.c1 - rg.c0 + 1) : 0;
  return {
    format: "xlsx",
    sheet: tab ? tab.name : "",
    sheetIndex: tab ? tab.index : 0,
    range: rangeStr,
    anchor,
    values: [],
    truncated: count > VALUES_CAP,
  };
}

async function docxSel(el) {
  const [cr, ur] = await Promise.all([
    el.shellAction("docx-caret", ""),
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
  const pos = { paragraph, run: 0, offset: chars };
  return { format: "docx", text, page, start: pos, end: pos, collapsed: true };
}

async function pptxSel(el) {
  const r = await el.shellAction("ui");
  const ui = (r && r.ui) || {};
  const tabs = ui.tabs || [];
  const tab = tabs.find((t) => t.active) || tabs[0] || null;
  const slide = tab ? tab.index : 0;
  return { format: "pptx", slide, shapeId: null, shapeName: null, text: null, textRange: null };
}

async function getSel(el) {
  const n = el.localName;
  if (n === "nitro-xlsx") return xlsxSel(el);
  if (n === "nitro-docx") return docxSel(el);
  if (n === "nitro-pptx") return pptxSel(el);
  return null;
}

export function attach(el) {
  el.getSelection = () => getSel(el);
  if (!el._readonly) import("./dh.js").then((m) => m.attach(el)).catch(() => {});
}
