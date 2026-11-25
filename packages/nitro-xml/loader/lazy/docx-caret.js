// SPDX-License-Identifier: MIT
/**
 * Docx caret + selection (lazy; off the eager budget): pointer hit-test, blinking caret overlay,
 * geometry-based keys (Up/Down/Home/End), and visible text-range selection (#561).
 *
 * Blocks carry `data-bid`; a paragraph split across pages has several blocks with the same id.
 * Rendered text can start with a list label the model does not have, so positions are exchanged
 * as (chars before, rendered total) and Rust subtracts the difference from the model length.
 *
 * Selection state is exposed on the element as `el._nxSel = { anchorK, text }` so s.js can
 * assemble the getSelection() payload without re-reading the DOM.
 */
const STYLE =
  ".nxc{position:absolute;z-index:5;pointer-events:none;width:1.5px;margin-left:-.75px;background:#000;" +
  "animation:nxcb 1.06s steps(1) infinite;display:none}@keyframes nxcb{50%{opacity:0}}" +
  ".nxh{position:absolute;z-index:4;pointer-events:none;top:0;left:0;width:100%;height:100%;overflow:hidden}" +
  ".nxh-r{position:absolute;background:rgba(0,120,215,.35)}";

export function attach(el) {
  if (el._nxc) return;
  const sr = el.shadowRoot;
  const v = sr && sr.querySelector(".v");
  const dom = sr && sr.querySelector(".dom");
  if (!v || !dom) return;
  el._nxc = 1;
  const listeners = new AbortController();
  const listen = (target, type, fn, opts = {}) => target.addEventListener(type, fn, { ...(typeof opts === 'boolean' ? {capture:opts} : opts), signal:listeners.signal });
  const originalAction = el.shellAction;

  const st = document.createElement("style");
  st.textContent = STYLE;
  sr.appendChild(st);
  const bar = document.createElement("div");
  bar.className = "nxc";
  bar.part = "caret";
  v.appendChild(bar);
  const hl = document.createElement("div");
  hl.className = "nxh";
  v.appendChild(hl);

  let caret = null; // { bid, shown } shown = rendered chars before the caret in the block
  let anchor = null; // { bid, shown } - null = collapsed/no selection; same as caret = collapsed
  let caretK = null; // [bid, chars, model] from most recent Rust caret response
  let anchorK = null; // [bid, chars, model] when anchor was last synced
  let caretRuns = null; // [len0, ...] code-point run lengths for caretK paragraph
  let anchorRuns = null; // [len0, ...] code-point run lengths for anchorK paragraph
  let syncSeq = 0; // generation counter; stale async caret responses are discarded
  let lastDownAt = 0;
  let lastDownX = 0;
  let lastDownY = 0;
  let clickCount = 0;
  let lineStart = false;
  let goalX = null;
  let reveal = false;

  const sel = (bid) => `[data-bid="${CSS_escape(bid)}"]`;
  const CSS_escape = (s) => (window.CSS && CSS.escape ? CSS.escape(s) : s.replace(/["\\]/g, "\\$&"));
  const spanText = (s) => (s.firstChild && s.firstChild.nodeType === 3 ? s.firstChild.data : "");
  const cpLen = (s) => {
    let n = 0;
    for (const _ of s) n++;
    return n;
  };
  const linesOf = () => Array.from(dom.querySelectorAll(".dve-print-line")).filter((l) => l.closest("[data-bid]"));
  const spansOf = (line) => Array.from(line.children).filter((c) => c.tagName === "SPAN" && spanText(c));
  /** Rendered spans of a paragraph in order, with cumulative char starts. */
  const shownIndex = (bid) => {
    const spans = [];
    for (const b of dom.querySelectorAll(sel(bid))) spans.push(...b.querySelectorAll(".dve-print-line > span"));
    let acc = 0;
    const list = [];
    for (const s of spans) {
      const t = spanText(s);
      if (!t) continue;
      const n = cpLen(t);
      list.push({ s, start: acc, n });
      acc += n;
    }
    return { list, total: acc };
  };
  /** Client rect of the boundary before code point `ci` of a span (after the last one when ci == n). */
  const boundaryRect = (span, ci) => {
    const node = span.firstChild;
    const cps = Array.from(node.data);
    let u = 0;
    for (let i = 0; i < Math.min(ci, cps.length); i++) u += cps[i].length;
    const r = document.createRange();
    if (ci < cps.length) {
      r.setStart(node, u);
      r.setEnd(node, u + cps[ci].length);
      const b = r.getBoundingClientRect();
      return { x: b.left, top: b.top, h: b.height };
    }
    r.setStart(node, Math.max(0, u - (cps[cps.length - 1] || "").length));
    r.setEnd(node, u);
    const b = r.getBoundingClientRect();
    return { x: b.right, top: b.top, h: b.height };
  };

  /** Ordered unique block ids currently in the DOM. */
  const domBids = () => {
    const bids = [];
    const seen = new Set();
    for (const node of dom.querySelectorAll("[data-bid]")) {
      const id = node.dataset.bid;
      if (!seen.has(id)) { seen.add(id); bids.push(id); }
    }
    return bids;
  };

  /** Compare two (bid, shown) positions by DOM order. Returns negative/0/positive. */
  const posCompare = (a, b) => {
    if (a.bid === b.bid) return a.shown - b.shown;
    const bids = domBids();
    return bids.indexOf(a.bid) - bids.indexOf(b.bid);
  };

  /** Convert code-point offset in s to UTF-16 offset. */
  const cpToUtf16 = (s, cpOff) => {
    let u = 0;
    for (let cps = 0; cps < cpOff && u < s.length; cps++) {
      const cp = s.codePointAt(u);
      u += cp > 0xffff ? 2 : 1;
    }
    return u;
  };

  /** Extract selected text across paragraphs using DOM order (paragraphs joined by '\n'). */
  const selText = () => {
    if (!anchor || !caret) return "";
    if (anchor.bid === caret.bid && anchor.shown === caret.shown) return "";
    const cmp = posCompare(anchor, caret);
    const [from, to] = cmp <= 0 ? [anchor, caret] : [caret, anchor];
    const bids = domBids();
    const i0 = bids.indexOf(from.bid);
    const i1 = bids.indexOf(to.bid);
    if (i0 < 0 || i1 < 0) return "";
    const parts = [];
    for (let i = i0; i <= i1; i++) {
      const bid = bids[i];
      const { list, total } = shownIndex(bid);
      const pStart = bid === from.bid ? from.shown : 0;
      const pEnd = bid === to.bid ? to.shown : total;
      let chunk = "";
      for (const e of list) {
        if (e.start + e.n <= pStart || e.start >= pEnd) continue;
        const t = spanText(e.s);
        const s0 = cpToUtf16(t, Math.max(0, pStart - e.start));
        const s1 = cpToUtf16(t, Math.min(e.n, pEnd - e.start));
        chunk += t.slice(s0, s1);
      }
      parts.push(chunk);
    }
    return parts.join("\n");
  };

  /** Update el._nxSel for s.js. */
  const updateSel = () => {
    if (!anchor || !caret || (anchor.bid === caret.bid && anchor.shown === caret.shown)) {
      el._nxSel = null;
      return;
    }
    el._nxSel = { anchorK, caretK, anchorRuns, caretRuns, text: selText() };
  };

  /** Build the sel-delete arg string "bid1:shown1:total1:bid2:shown2:total2". */
  const selArg = () => {
    if (!anchor || !caret) return null;
    if (anchor.bid === caret.bid && anchor.shown === caret.shown) return null;
    const { total: at } = shownIndex(anchor.bid);
    const { total: ct } = shownIndex(caret.bid);
    return `${anchor.bid}:${anchor.shown}:${at}:${caret.bid}:${caret.shown}:${ct}`;
  };

  /** Clear selection, keeping just the caret. */
  const clearSel = () => {
    anchor = null; anchorK = null; anchorRuns = null; el._nxSel = null;
    drawHighlights();
  };

  /** Merge text-node rects on the same visual line so multi-span lines get one even highlight. */
  const mergeLineRects = (rects) => {
    const sorted = [...rects].sort((a, b) => a.left - b.left);
    const merged = [];
    for (const r of sorted) {
      const last = merged[merged.length - 1];
      if (last && r.left <= last.right + 2 && last.left <= r.right + 2) {
        last.left = Math.min(last.left, r.left);
        last.top = Math.min(last.top, r.top);
        last.right = Math.max(last.right, r.right);
        last.bottom = Math.max(last.bottom, r.bottom);
      } else {
        merged.push({ left: r.left, top: r.top, right: r.right, bottom: r.bottom });
      }
    }
    return merged;
  };

  /** Draw one even highlight rect per visual line between anchor and caret. */
  const drawHighlights = () => {
    hl.textContent = "";
    if (!anchor || !caret) return;
    if (anchor.bid === caret.bid && anchor.shown === caret.shown) return;
    const cmp = posCompare(anchor, caret);
    const [from, to] = cmp <= 0 ? [anchor, caret] : [caret, anchor];
    const bids = domBids();
    const i0 = bids.indexOf(from.bid);
    const i1 = bids.indexOf(to.bid);
    if (i0 < 0 || i1 < 0) return;
    const vr = v.getBoundingClientRect();
    for (let i = i0; i <= i1; i++) {
      const bid = bids[i];
      const { list, total } = shownIndex(bid);
      const pStart = bid === from.bid ? from.shown : 0;
      const pEnd = bid === to.bid ? to.shown : total;
      if (pStart >= pEnd) continue;
      // Group overlapping text-node rects per .dve-print-line to avoid duplicate/overlapping boxes.
      const byLine = new Map();
      for (const e of list) {
        if (e.start + e.n <= pStart || e.start >= pEnd) continue;
        const tn = e.s.firstChild;
        if (!tn || tn.nodeType !== 3) continue;
        const lineEl = e.s.parentNode;
        const s0 = cpToUtf16(tn.data, Math.max(0, pStart - e.start));
        const s1 = cpToUtf16(tn.data, Math.min(e.n, pEnd - e.start));
        if (s1 <= s0) continue;
        try {
          const rng = document.createRange();
          rng.setStart(tn, s0);
          rng.setEnd(tn, s1);
          let arr = byLine.get(lineEl);
          if (!arr) { arr = []; byLine.set(lineEl, arr); }
          for (const r of rng.getClientRects()) {
            if (r.width >= 0.5 && r.height >= 1) arr.push(r);
          }
        } catch (_) {}
      }
      for (const rects of byLine.values()) {
        for (const m of mergeLineRects(rects)) {
          const d = document.createElement("div");
          d.className = "nxh-r";
          d.style.left = m.left - vr.left + "px";
          d.style.top = m.top - vr.top + "px";
          d.style.width = m.right - m.left + "px";
          d.style.height = m.bottom - m.top + "px";
          hl.appendChild(d);
        }
      }
    }
  };

  /** Hit-test inside one line at client x -> { bid, shown, lineStart, x }. */
  const hitLine = (line, x) => {
    const block = line.closest("[data-bid]");
    const bid = block.dataset.bid;
    const { list, total } = shownIndex(bid);
    const mine = new Set(spansOf(line));
    const first = list.findIndex((e) => mine.has(e.s));
    if (first < 0) {
      const before = list.length ? list[list.length - 1].start + list[list.length - 1].n : 0;
      return { bid, shown: before, total, lineStart: false };
    }
    const entries = list.filter((e) => mine.has(e.s));
    let pick = null;
    let ci = 0;
    for (const e of entries) {
      const r = e.s.getBoundingClientRect();
      if (x < r.left) {
        pick = e;
        ci = 0;
        break;
      }
      if (x <= r.right) {
        pick = e;
        ci = e.n;
        const cps = Array.from(spanText(e.s));
        for (let k = 0; k < cps.length; k++) {
          const b = boundaryRect(e.s, k);
          const nb = k + 1 < cps.length ? boundaryRect(e.s, k + 1).x : r.right;
          const w = nb - b.x;
          if (x < b.x + w / 2) {
            ci = k;
            break;
          }
        }
        break;
      }
    }
    if (!pick) {
      pick = entries[entries.length - 1];
      ci = pick.n;
    }
    const shown = pick.start + ci;
    return { bid, shown, total, lineStart: pick === entries[0] && ci === 0 && first > 0 };
  };

  const lineAt = (x, y) => {
    for (const n of sr.elementsFromPoint(x, y)) {
      const l = n.closest && n.closest(".dve-print-line");
      if (l && l.closest("[data-bid]") && dom.contains(l)) return l;
    }
    let best = null;
    let bd = Infinity;
    for (const l of linesOf()) {
      const r = l.getBoundingClientRect();
      if (!r.height) continue;
      const d = y < r.top ? r.top - y : y > r.bottom ? y - r.bottom : 0;
      if (d < bd) { bd = d; best = l; }
    }
    return best;
  };

  const rectOf = () => {
    if (!caret) return null;
    const { list, total } = shownIndex(caret.bid);
    if (!list.length) {
      const b = dom.querySelector(sel(caret.bid));
      const l = b && b.querySelector(".dve-print-line");
      if (!l) return null;
      const r = l.getBoundingClientRect();
      return { x: r.left, top: r.top, h: r.height };
    }
    const t = Math.min(caret.shown, total);
    let k = -1;
    for (let i = 0; i < list.length; i++) {
      const e = list[i];
      if (t > e.start && t <= e.start + e.n) { k = i; break; }
      if (t === 0) { k = 0; break; }
    }
    if (k < 0) k = list.length - 1;
    let e = list[k];
    const next = list[k + 1];
    if (lineStart && next && t === e.start + e.n && next.s.parentNode !== e.s.parentNode) e = next;
    return boundaryRect(e.s, Math.max(0, Math.min(e.n, t - e.start)));
  };

  let placeTries = 0;
  const place = (fresh) => {
    if (el.readonly || listeners.signal.aborted) { bar.style.display = "none"; return; }
    // Hide caret bar during non-collapsed selection (#561 highlight clarity).
    if (anchor && caret && (anchor.bid !== caret.bid || anchor.shown !== caret.shown)) {
      bar.style.display = "none";
      return;
    }
    if (fresh) placeTries = 0;
    const r = rectOf();
    const d = dom.getBoundingClientRect();
    if (!r || !r.h) {
      if (caret && placeTries < 40) {
        placeTries++;
        requestAnimationFrame(() => place(false));
        return;
      }
      bar.style.display = "none";
      return;
    }
    placeTries = 0;
    if (reveal) {
      reveal = false;
      const pad = 24;
      if (r.top < d.top + pad) dom.scrollTop -= d.top + pad - r.top;
      else if (r.top + r.h > d.bottom - pad) dom.scrollTop += r.top + r.h - (d.bottom - pad);
      return place(false);
    }
    const vr = v.getBoundingClientRect();
    const visible = r.top + r.h > d.top && r.top < d.bottom && r.x >= d.left && r.x <= d.right;
    bar.style.display = visible ? "block" : "none";
    bar.style.left = r.x - vr.left + "px";
    bar.style.top = r.top - vr.top + "px";
    bar.style.height = r.h + "px";
    for (const a of bar.getAnimations()) { a.cancel(); a.play(); }
  };

  const setFromK = (k, keepGoal, runs) => {
    caretK = k || null;
    if (runs !== undefined) caretRuns = runs || null;
    else if (!k || (caret && caret.bid !== k[0])) caretRuns = null;
    if (!keepGoal) goalX = null;
    if (!k) {
      caret = null;
      drawHighlights();
      return place(true);
    }
    const [bid, chars, model] = k;
    const { total } = shownIndex(bid);
    const shown = Math.max(0, total - model) + chars;
    if (caret && caret.bid !== bid) lineStart = false;
    caret = { bid, shown };
    updateSel();
    drawHighlights();
    place(true);
  };

  const syncModel = async (hit) => {
    const mySeq = ++syncSeq;
    const r = await el.shellAction("docx-caret", `${hit.bid}:${hit.shown}:${hit.total}`);
    const k = r && r.k;
    if (syncSeq === mySeq) setFromK(k, true, r && r.runs);
    return r;
  };

  /** Move the caret to a hit result: optimistic overlay first, then the model. */
  const commit = (hit) => {
    caret = { bid: hit.bid, shown: hit.shown };
    lineStart = hit.lineStart;
    place(true);
    return syncModel(hit);
  };

  /** Select the word around the given hit position. */
  const selectWord = async (hit) => {
    const mySeq = ++syncSeq;
    const { list, total } = shownIndex(hit.bid);
    const text = list.map((e) => spanText(e.s)).join("");
    const cps = Array.from(text);
    const pos = Math.min(hit.shown, cps.length);
    const isW = (ch) => /\w/.test(ch);
    let start = pos, end = pos;
    // extend backward then forward over the word class at the cursor
    const ch = pos < cps.length ? cps[pos] : (pos > 0 ? cps[pos - 1] : "");
    const inWord = isW(ch);
    if (inWord || (pos > 0 && isW(cps[pos - 1]))) {
      let s = pos > 0 && isW(cps[pos - 1]) ? pos - 1 : pos;
      while (s > 0 && isW(cps[s - 1])) s--;
      start = s;
      end = pos > 0 && isW(cps[pos - 1]) ? pos : pos;
      while (end < cps.length && isW(cps[end])) end++;
    }
    if (start === end && start < cps.length) end = start + 1;
    anchor = { bid: hit.bid, shown: start };
    anchorK = null;
    anchorRuns = null;
    caret = { bid: hit.bid, shown: end };
    caretK = null;
    caretRuns = null;
    lineStart = false;
    updateSel();
    place(true);
    drawHighlights();
    const [ra, rc] = await Promise.all([
      el.shellAction("docx-caret", `${hit.bid}:${start}:${total}`),
      el.shellAction("docx-caret", `${hit.bid}:${end}:${total}`),
    ]);
    if (syncSeq !== mySeq) return;
    anchorK = ra && ra.k;
    anchorRuns = (ra && ra.runs) || null;
    if (ra && ra.k) {
      const [bid, chars, model] = ra.k;
      const { total: t } = shownIndex(bid);
      anchor = { bid, shown: Math.max(0, t - model) + chars };
    }
    if (rc && rc.k) {
      caretK = rc.k;
      caretRuns = (rc && rc.runs) || null;
      const [bid, chars, model] = rc.k;
      const { total: t } = shownIndex(bid);
      caret = { bid, shown: Math.max(0, t - model) + chars };
    }
    updateSel();
    drawHighlights();
    place(true);
  };

  const editable = () => el.readonly === false && el.document;

  listen(v,
    "pointerdown",
    (e) => {
      if (e.button !== 0 || !editable()) return;
      wrapDoc();
      preReplaceState = null;
      const now = performance.now();
      if (now - lastDownAt < 500 && Math.hypot(e.clientX - lastDownX, e.clientY - lastDownY) <= 5) {
        clickCount++;
      } else {
        clickCount = 1;
      }
      lastDownAt = now;
      lastDownX = e.clientX;
      lastDownY = e.clientY;
      const detail = e.detail || clickCount;

      if (el.document) el.document._nxCaretK = null;
      const d = dom.getBoundingClientRect();
      if (e.clientX - d.left > dom.clientWidth || e.clientY - d.top > dom.clientHeight) return;
      const line = lineAt(e.clientX, e.clientY);
      if (!line) return;
      goalX = null;
      reveal = false;

      const hit = hitLine(line, e.clientX);

      if (detail >= 3) {
        // Triple-click: select entire paragraph
        e.preventDefault();
        const mySeq = ++syncSeq;
        const { total } = shownIndex(hit.bid);
        anchor = { bid: hit.bid, shown: 0 };
        anchorK = null;
        anchorRuns = null;
        caret = { bid: hit.bid, shown: total };
        caretK = null;
        caretRuns = null;
        lineStart = false;
        updateSel();
        place(true);
        drawHighlights();
        v.setPointerCapture(e.pointerId);
        void (async () => {
          const [ra, rc] = await Promise.all([
            el.shellAction("docx-caret", `${hit.bid}:0:${total}`),
            el.shellAction("docx-caret", `${hit.bid}:${total}:${total}`),
          ]);
          if (syncSeq !== mySeq) return;
          anchorK = ra && ra.k;
          anchorRuns = (ra && ra.runs) || null;
          if (rc && rc.k) { caretK = rc.k; caretRuns = (rc && rc.runs) || null; }
          updateSel();
          drawHighlights();
        })();
        return;
      }

      if (detail === 2) {
        // Double-click: select word
        e.preventDefault();
        v.setPointerCapture(e.pointerId);
        void selectWord(hit);
        return;
      }

      if (e.shiftKey && caret) {
        // Shift+click: extend selection, keep existing anchor (or pin at current caret)
        if (!anchor) { anchor = { bid: caret.bid, shown: caret.shown }; anchorK = caretK; anchorRuns = caretRuns; }
        caret = { bid: hit.bid, shown: hit.shown };
        updateSel(); drawHighlights(); place(true);
        void commit(hit).then(() => { drawHighlights(); updateSel(); });
        return;
      }

      // Single click: set anchor optimistically, then commit caret
      const myAnchor = { bid: hit.bid, shown: hit.shown };
      anchor = myAnchor;
      anchorK = null;
      anchorRuns = null;
      el._nxSel = null;
      v.setPointerCapture(e.pointerId);
      void commit(hit).then((r) => {
        if (anchor === myAnchor && r && r.k) {
          anchorK = r.k;
          anchorRuns = r.runs || null;
          updateSel();
        }
      });
    },
    true,
  );

  listen(v,
    "pointermove",
    (e) => {
      if (!v.hasPointerCapture(e.pointerId)) return;
      if (!editable()) return;
      const line = lineAt(e.clientX, e.clientY);
      if (!line) return;
      const hit = hitLine(line, e.clientX);
      if (!anchor) return;
      // Update caret (focus) while keeping anchor
      caret = { bid: hit.bid, shown: hit.shown };
      lineStart = hit.lineStart;
      place(true);
      drawHighlights();
      updateSel();
      void syncModel(hit);
    },
    true,
  );

  listen(v,
    "pointerup",
    (e) => {
      if (!v.hasPointerCapture(e.pointerId)) return;
      v.releasePointerCapture(e.pointerId);
      // Simple click (no drag): anchor === caret, clear selection
      if (anchor && caret && anchor.bid === caret.bid && anchor.shown === caret.shown) {
        anchor = null; anchorK = null; anchorRuns = null; el._nxSel = null;
        drawHighlights();
      }
    },
    true,
  );

  let restoreKey;
  // Saved pre-replace state for single-step undo restoration (#561).
  let preReplaceState = null;
  const wrapDoc = () => {
    const d = el.document;
    if (!d || d._nxc) return;
    d._nxc = 1;
    const orig = d.editorKey.bind(d);
    restoreKey = () => { d.editorKey = orig; d._nxc = 0; d._nxReCaret = null; };
    d._nxCaretK = null;
    d._nxCaretTries = 0;
    d._nxReCaret = () => {
      const k = d._nxCaretK;
      if (!k) return;
      if (anchor && caret && (anchor.bid !== caret.bid || anchor.shown !== caret.shown)) {
        d._nxCaretK = null;
        bar.style.display = "none";
        return;
      }
      setFromK(k, false);
      if (bar.style.display === "block") {
        d._nxCaretK = null;
        return;
      }
      if (d._nxCaretTries < 8) {
        d._nxCaretTries++;
        requestAnimationFrame(() => d._nxReCaret());
      }
    };
    d.editorKey = async (ev, mac) => {
      const hasSelection = anchor && caret && (anchor.bid !== caret.bid || anchor.shown !== caret.shown);
      const isShiftArrow = ev.shiftKey && ["ArrowLeft","ArrowRight","ArrowUp","ArrowDown","Home","End"].includes(ev.key);
      const isNavKey = !ev.shiftKey && ["ArrowLeft","ArrowRight","ArrowUp","ArrowDown","Home","End"].includes(ev.key);
      const isTyping = !ev.ctrlKey && !ev.metaKey && !ev.altKey && ev.key.length === 1;
      const isDelete = ev.key === "Backspace" || ev.key === "Delete";
      const isEnter = ev.key === "Enter";
      const isUndo = !ev.shiftKey && (ev.ctrlKey || ev.metaKey) && (ev.key === "z" || ev.key === "Z");

      // Undo: restore pre-replace selection state if available (#561).
      if (isUndo && preReplaceState) {
        const saved = preReplaceState;
        preReplaceState = null;
        const r = await orig(ev, mac);
        if (r && r.handled) {
          anchor = saved.anchor;
          caret = saved.caret;
          anchorK = saved.anchorK;
          caretK = saved.caretK;
          anchorRuns = saved.anchorRuns;
          caretRuns = saved.caretRuns;
          el._nxSel = saved.nxSel;
          updateSel();
          drawHighlights();
          place(true);
        }
        return r;
      }

      // Atomic typing replace: delete selection + insert char in one undo step (#561).
      if (hasSelection && isTyping) {
        const arg = selArg();
        if (arg) {
          preReplaceState = {
            anchor: anchor ? { ...anchor } : null,
            caret: caret ? { ...caret } : null,
            anchorK,
            caretK,
            anchorRuns,
            caretRuns,
            nxSel: el._nxSel ? { ...el._nxSel } : null,
          };
          const cr = await el.shellAction("docx-sel-replace", `${arg}:${ev.key.codePointAt(0)}`);
          anchor = null; anchorK = null; anchorRuns = null; el._nxSel = null;
          drawHighlights();
          if (cr && cr.k) { caretK = cr.k; setFromK(cr.k, false); }
          return { handled: true, k: cr && cr.k };
        }
      }

      // Selection replace for delete/enter: delete selection before typing/deleting
      if (hasSelection && (isDelete || isEnter)) {
        const arg = selArg();
        if (arg) {
          preReplaceState = isDelete ? {
            anchor: anchor ? { ...anchor } : null,
            caret: caret ? { ...caret } : null,
            anchorK,
            caretK,
            anchorRuns,
            caretRuns,
            nxSel: el._nxSel ? { ...el._nxSel } : null,
          } : null;
          const dr = await el.shellAction("docx-sel-delete", arg);
          anchor = null; anchorK = null; anchorRuns = null; el._nxSel = null;
          drawHighlights();
          if (dr && dr.k) { caretK = dr.k; setFromK(dr.k, false); }
          if (isDelete) return { handled: true, k: dr && dr.k };
          // For enter: fall through so Enter creates a paragraph break at the new caret
        }
      }

      // Any non-replace, non-undo key clears saved pre-replace undo state.
      if (!hasSelection && !isUndo && (isTyping || isDelete || isEnter)) {
        preReplaceState = null;
      }

      // Plain nav: clear selection; Shift+nav: set anchor before move
      if (isNavKey && anchor) {
        anchor = null; anchorK = null; anchorRuns = null; el._nxSel = null; drawHighlights();
      } else if (isShiftArrow && caret && !anchor) {
        anchor = { bid: caret.bid, shown: caret.shown };
        anchorK = caretK;
        anchorRuns = caretRuns;
      }

      const r = await orig(ev, mac);
      if (r && r.handled) {
        reveal = true;
        if (!isShiftArrow) goalX = null;
        lineStart = false;
        caretK = r.k || null;
        d._nxCaretK = r.k || null;
        d._nxCaretTries = 0;
        setFromK(r.k, isShiftArrow && (ev.key === "ArrowUp" || ev.key === "ArrowDown"));
        if (bar.style.display === "block") d._nxCaretK = null;
      }
      return r;
    };
  };

  listen(v,
    "keydown",
    (e) => {
      if (!editable()) return;
      wrapDoc();
      const nav = ["ArrowUp", "ArrowDown", "Home", "End"].includes(e.key);
      if (!nav || e.shiftKey || e.altKey || !caret) return;
      const cmd = e.ctrlKey || e.metaKey;
      if (cmd && (e.key === "ArrowUp" || e.key === "ArrowDown")) return;
      const lines = linesOf();
      const r = rectOf();
      if (!r || !lines.length) return;
      const cy = r.top + r.h / 2;
      let idx = lines.findIndex((l) => {
        const b = l.getBoundingClientRect();
        return cy >= b.top && cy <= b.bottom;
      });
      if (idx < 0) idx = 0;
      let target;
      let x = goalX == null ? r.x : goalX;
      if (e.key === "ArrowUp") target = lines[Math.max(0, idx - 1)];
      else if (e.key === "ArrowDown") target = lines[Math.min(lines.length - 1, idx + 1)];
      else if (cmd) {
        target = e.key === "Home" ? lines[0] : lines[lines.length - 1];
        x = e.key === "Home" ? -Infinity : Infinity;
      } else {
        target = lines[idx];
        x = e.key === "Home" ? -Infinity : Infinity;
      }
      e.preventDefault();
      e.stopPropagation();
      const vertical = e.key === "ArrowUp" || e.key === "ArrowDown";
      reveal = true;
      // Clear selection on plain nav keys
      if (anchor) { anchor = null; anchorK = null; anchorRuns = null; el._nxSel = null; drawHighlights(); }
      void commit(hitLine(target, x)).then(() => {
        goalX = vertical ? x : null;
      });
      if (vertical) goalX = x;
    },
    true,
  );

  const mo = new MutationObserver(() => { updateSel(); place(true); drawHighlights(); });
  mo.observe(dom, { childList: true, subtree: true });
  listen(dom, "scroll", () => { place(true); drawHighlights(); }, { passive: true });
  const resize = typeof ResizeObserver !== "undefined" ? new ResizeObserver(() => { place(true); drawHighlights(); }) : null;
  resize?.observe(v);

  let focusGen = 0;
  const focusInside = () => {
    const root = v.getRootNode();
    const cur = root && root.activeElement;
    if (cur && (cur === v || v.contains(cur))) return true;
    const host = root && root.host;
    return !!(host && document.activeElement === host);
  };
  listen(v, "focusin", () => { focusGen++; place(true); });
  listen(v, "focusout", () => {
    const gen = ++focusGen;
    let left = 0;
    const step = () => {
      if (gen !== focusGen) return;
      if (focusInside()) { place(true); return; }
      if (left++ < 10) { requestAnimationFrame(step); return; }
      if (gen === focusGen && !focusInside()) {
        bar.style.display = "none";
        // Preserve highlight while focus is outside the viewer
      }
    };
    requestAnimationFrame(step);
  });

  const origAction = el.shellAction.bind(el);
  el.shellAction = async (op, arg) => {
    if (op === "cmd" && editable() && arg === "edit.undo" && preReplaceState) {
      const saved = preReplaceState;
      preReplaceState = null;
      const r = await origAction(op, arg);
      anchor = saved.anchor;
      caret = saved.caret;
      anchorK = saved.anchorK;
      caretK = saved.caretK;
      anchorRuns = saved.anchorRuns;
      caretRuns = saved.caretRuns;
      el._nxSel = saved.nxSel;
      drawHighlights();
      place(true);
      if (caret) {
        const { total } = shownIndex(caret.bid);
        void origAction("docx-caret", `${caret.bid}:${caret.shown}:${total}`);
      }
      return r;
    }
    if (op === "cmd" && editable()) preReplaceState = null;
    const r = await origAction(op, arg);
    if (op === "cmd" && editable()) {
      anchor = null; anchorK = null; anchorRuns = null; el._nxSel = null;
      const c = await origAction("docx-caret", "");
      setFromK(c && c.k, false, c && c.runs);
    }
    return r;
  };

  el._detachCaret = () => {
    listeners.abort(); mo.disconnect(); resize?.disconnect(); restoreKey?.();
    st.remove(); bar.remove(); hl.remove(); el.shellAction = originalAction; el._nxc = 0;
    el._nxSel = null;
  };
}
