// SPDX-License-Identifier: MIT
/**
 * Pptx text-box caret + selection (lazy; off the eager budget): hit-test within
 * `.pptxv-para` spans, blinking caret overlay, visible selection highlights (#561).
 *
 * Pptx shape text is a flat string with '\n' between paragraphs. The DOM maps it as
 * `[data-pptx-kind="text"] .pptxv-para > span` (normal inline flow, no print-line
 * wrappers). Char offsets are code-point counts in that flat string.
 *
 * Selection state is exposed on the element as `el._nxPptxSel = { anchorPos, caretPos, text }`
 * so s.js can assemble the getSelection() payload.
 */
const STYLE =
  ".nxpc{position:absolute;z-index:5;pointer-events:none;width:1.5px;margin-left:-.75px;background:#000;" +
  "animation:nxpcb 1.06s steps(1) infinite;display:none}@keyframes nxpcb{50%{opacity:0}}" +
  ".nxph{position:absolute;z-index:4;pointer-events:none;top:0;left:0;width:100%;height:100%;overflow:hidden}" +
  ".nxph-r{position:absolute;background:rgba(0,120,215,.35)}";

export function attach(el) {
  if (el._nxpc) return;
  const sr = el.shadowRoot;
  const v = sr && sr.querySelector(".v");
  const dom = sr && sr.querySelector(".dom");
  if (!v || !dom) return;
  el._nxpc = 1;
  const listeners = new AbortController();
  const listen = (target, type, fn, opts = {}) =>
    target.addEventListener(type, fn, { ...(typeof opts === "boolean" ? { capture: opts } : opts), signal: listeners.signal });
  const originalAction = el.shellAction;

  const st = document.createElement("style");
  st.textContent = STYLE;
  sr.appendChild(st);
  const bar = document.createElement("div");
  bar.className = "nxpc";
  bar.part = "pptx-caret";
  v.appendChild(bar);
  const hl = document.createElement("div");
  hl.className = "nxph";
  v.appendChild(hl);

  let caret = null; // { shapeId, offset } — code-point offset in shape flat text
  let anchor = null; // { shapeId, offset } — null = collapsed
  let caretK = null; // [shapeId, offset] from last Rust response
  let anchorK = null;
  let dragStarted = false; // true once pointermove fires during a drag
  let textDragActive = false; // true when drag started directly on text (suppresses shape move)
  let syncSeq = 0; // generation counter; stale syncModel responses are discarded
  let lastDownAt = 0;
  let lastDownX = 0;
  let lastDownY = 0;
  let clickCount = 0;

  // ---- DOM helpers ----

  const shapeOf = (node) => {
    const el = node.closest && node.closest('[data-pptx-kind="text"]');
    return dom.contains(el) ? el : null;
  };

  const parasOf = (shapeEl) => Array.from(shapeEl.querySelectorAll(".pptxv-para"));

  /** All text spans in a paragraph. */
  const spansOf = (paraEl) =>
    Array.from(paraEl.querySelectorAll("span")).filter(
      (s) => s.firstChild && s.firstChild.nodeType === 3
    );

  /** Count code points in UTF-16 string s up to utf16Off. */
  const cpCount = (s, utf16Off) => {
    let cps = 0;
    for (let i = 0; i < utf16Off && i < s.length; ) {
      const cp = s.codePointAt(i);
      i += cp > 0xffff ? 2 : 1;
      cps++;
    }
    return cps;
  };

  /** Convert code-point offset within para text to UTF-16 offset. */
  const cpToUtf16 = (s, cpOff) => {
    let u = 0;
    let cps = 0;
    for (; cps < cpOff && u < s.length; cps++) {
      const cp = s.codePointAt(u);
      u += cp > 0xffff ? 2 : 1;
    }
    return u;
  };

  /** Code-point length of a string. */
  const cpLen = (s) => {
    let n = 0;
    for (const _ of s) n++;
    return n;
  };

  /** Compute code-point offset within shape flat text given a DOM position. */
  const domToShapeOffset = (shapeEl, domNode, utf16Off) => {
    let off = 0;
    let paraIdx = 0;
    for (const para of parasOf(shapeEl)) {
      let paraOff = 0;
      for (const span of spansOf(para)) {
        const tn = span.firstChild;
        if (tn === domNode) {
          return off + paraOff + cpCount(tn.data, utf16Off);
        }
        paraOff += cpLen(tn.data);
      }
      off += paraOff;
      paraIdx++;
      off += 1; // '\n' separator
    }
    return off;
  };

  /** Convert shape flat-text code-point offset to a DOM {node, utf16Off} position. */
  const shapeToDomPos = (shapeEl, offset) => {
    let rem = offset;
    for (const para of parasOf(shapeEl)) {
      for (const span of spansOf(para)) {
        const tn = span.firstChild;
        const n = cpLen(tn.data);
        if (rem <= n) {
          return { node: tn, utf16Off: cpToUtf16(tn.data, rem) };
        }
        rem -= n;
      }
      // past newline
      if (rem === 0) {
        // position at end of last span of this para
        const spans = spansOf(para);
        if (spans.length) {
          const tn = spans[spans.length - 1].firstChild;
          return { node: tn, utf16Off: tn.data.length };
        }
      }
      rem -= 1; // consume '\n'
      if (rem < 0) rem = 0;
    }
    // After all text: end of last span
    const allSpans = Array.from(shapeEl.querySelectorAll(".pptxv-para span")).filter(
      (s) => s.firstChild && s.firstChild.nodeType === 3
    );
    if (!allSpans.length) return null;
    const last = allSpans[allSpans.length - 1];
    const tn = last.firstChild;
    return { node: tn, utf16Off: tn.data.length };
  };

  /** Build the flat text of a shape from DOM spans. */
  const shapeTextOf = (shapeId) => {
    const shapeEl = shapeElOf(shapeId);
    if (!shapeEl) return "";
    let text = "";
    for (const para of parasOf(shapeEl)) {
      for (const span of spansOf(para)) text += span.firstChild.data;
      text += "\n";
    }
    return text.endsWith("\n") ? text.slice(0, -1) : text;
  };

  /** Hit-test a span at client x using character boundaries (like docx-caret.js boundaryRect). */
  const hitSpanAt = (spanEl, x) => {
    const tn = spanEl.firstChild;
    if (!tn || tn.nodeType !== 3) return null;
    const r = spanEl.getBoundingClientRect();
    if (r.width < 1) return null;
    const cps = Array.from(tn.data);
    for (let k = 0; k < cps.length; k++) {
      let leftX, rightX;
      try {
        const rng = document.createRange();
        const u0 = cpToUtf16(tn.data, k);
        rng.setStart(tn, u0);
        rng.setEnd(tn, u0 + cps[k].length);
        const b = rng.getBoundingClientRect();
        leftX = b.left;
        rightX = b.right;
      } catch (_) {
        leftX = r.left + (r.width * k) / cps.length;
        rightX = r.left + (r.width * (k + 1)) / cps.length;
      }
      const mid = (leftX + rightX) / 2;
      if (x <= mid) return k;
    }
    return cps.length;
  };

  /** Hit-test at (x, y) using shadow-DOM-aware elementsFromPoint. */
  const hitAt = (x, y) => {
    // elementsFromPoint on the shadow root pierces shadow boundaries correctly.
    const hits = sr.elementsFromPoint(x, y);
    for (const el of hits) {
      if (!dom.contains(el)) continue;
      const actualSpan = el.tagName === "SPAN" && el.firstChild && el.firstChild.nodeType === 3 ? el : null;
      if (!actualSpan) continue;
      const shapeEl = shapeOf(actualSpan);
      if (!shapeEl) continue;
      const shapeId = shapeEl.dataset.elementId;
      if (!shapeId) continue;
      const ci = hitSpanAt(actualSpan, x);
      if (ci === null) continue;
      const utf16Off = cpToUtf16(actualSpan.firstChild.data, ci);
      const off = domToShapeOffset(shapeEl, actualSpan.firstChild, utf16Off);
      return { shapeId, offset: off, direct: true };
    }
    // Fallback: find nearest span by Y distance, breaking ties on the same line by X distance.
    let best = null, bd = Infinity;
    for (const shapeEl of dom.querySelectorAll('[data-pptx-kind="text"]')) {
      for (const span of shapeEl.querySelectorAll('.pptxv-para > span')) {
        const tn = span.firstChild;
        if (!tn || tn.nodeType !== 3) continue;
        const r = span.getBoundingClientRect();
        if (!r.height) continue;
        const dy = y < r.top ? r.top - y : y > r.bottom ? y - r.bottom : 0;
        const dx = x < r.left ? r.left - x : x > r.right ? x - r.right : 0;
        const d = dy * 10000 + dx;
        if (d < bd) { bd = d; best = { span, shapeEl }; }
      }
    }
    if (!best) return null;
    const { span, shapeEl } = best;
    const shapeId = shapeEl.dataset.elementId;
    if (!shapeId) return null;
    const r = span.getBoundingClientRect();
    const ci = x <= r.left ? 0 : x >= r.right ? cpLen(span.firstChild.data) : hitSpanAt(span, x) ?? cpLen(span.firstChild.data);
    const utf16Off = cpToUtf16(span.firstChild.data, ci);
    const off = domToShapeOffset(shapeEl, span.firstChild, utf16Off);
    return { shapeId, offset: off, direct: false };
  };

  /** Find the shape element for a given shapeId. */
  const shapeElOf = (shapeId) =>
    dom.querySelector(`[data-pptx-kind="text"][data-element-id="${CSS.escape(shapeId)}"]`);

  // ---- Caret placement ----

  /** Convert shape flat-text offset to a {x, top, h} client rect for caret rendering. */
  const caretRect = () => {
    if (!caret) return null;
    const shapeEl = shapeElOf(caret.shapeId);
    if (!shapeEl) return null;
    const pos = shapeToDomPos(shapeEl, caret.offset);
    if (!pos) return null;
    try {
      const r = document.createRange();
      r.setStart(pos.node, pos.utf16Off);
      r.setEnd(pos.node, pos.utf16Off);
      // For a collapsed range, getBoundingClientRect may be 0-width
      const b = r.getBoundingClientRect();
      if (b.height > 0) return { x: b.left, top: b.top, h: b.height };
      // Fallback: use the span rect
      const span = pos.node.parentNode;
      if (span) {
        const sb = span.getBoundingClientRect();
        // Estimate x by character position
        const paraText = pos.node.data;
        const frac = pos.utf16Off / (paraText.length || 1);
        return { x: sb.left + sb.width * frac, top: sb.top, h: sb.height };
      }
    } catch (_) {}
    return null;
  };

  let placeTries = 0;
  const place = (fresh) => {
    if (el.readonly || listeners.signal.aborted) { bar.style.display = "none"; return; }
    // Hide caret bar during non-collapsed selection (#561 highlight clarity).
    if (anchor && caret && anchor.shapeId === caret.shapeId && anchor.offset !== caret.offset) {
      bar.style.display = "none";
      return;
    }
    if (fresh) placeTries = 0;
    const r = caretRect();
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
    const vr = v.getBoundingClientRect();
    const dr = dom.getBoundingClientRect();
    const visible = r.top + r.h > dr.top && r.top < dr.bottom && r.x >= dr.left && r.x <= dr.right;
    bar.style.display = visible ? "block" : "none";
    bar.style.left = r.x - vr.left + "px";
    bar.style.top = r.top - vr.top + "px";
    bar.style.height = r.h + "px";
    for (const a of bar.getAnimations()) { a.cancel(); a.play(); }
  };

  // ---- Highlight overlay ----

  const mergeLineRects = (rects) => {
    const out = [];
    for (const r of rects) {
      let merged = false;
      for (const g of out) {
        const vOverlap = Math.min(r.bottom, g.bottom) - Math.max(r.top, g.top);
        const minH = Math.min(r.bottom - r.top, g.bottom - g.top);
        if (vOverlap > minH * 0.4 && r.left <= g.right + 2 && g.left <= r.right + 2) {
          g.left = Math.min(g.left, r.left);
          g.top = Math.min(g.top, r.top);
          g.right = Math.max(g.right, r.right);
          g.bottom = Math.max(g.bottom, r.bottom);
          merged = true;
          break;
        }
      }
      if (!merged) out.push({ left: r.left, top: r.top, right: r.right, bottom: r.bottom });
    }
    return out;
  };

  const drawHighlights = () => {
    hl.textContent = "";
    if (!anchor || !caret) return;
    if (anchor.shapeId !== caret.shapeId) return; // cross-shape not supported
    if (anchor.offset === caret.offset) return;
    const shapeEl = shapeElOf(anchor.shapeId);
    if (!shapeEl) return;
    const from = anchor.offset < caret.offset ? anchor.offset : caret.offset;
    const to = anchor.offset < caret.offset ? caret.offset : anchor.offset;
    const vr = v.getBoundingClientRect();
    let off = 0;
    for (const para of parasOf(shapeEl)) {
      const rects = [];
      for (const span of spansOf(para)) {
        const tn = span.firstChild;
        const n = cpLen(tn.data);
        if (off + n > from && off < to) {
          const s0 = cpToUtf16(tn.data, Math.max(0, from - off));
          const s1 = cpToUtf16(tn.data, Math.min(n, to - off));
          if (s1 > s0) {
            try {
              const rng = document.createRange();
              rng.setStart(tn, s0);
              rng.setEnd(tn, s1);
              for (const r of rng.getClientRects()) {
                if (r.width >= 0.5 && r.height >= 1) rects.push(r);
              }
            } catch (_) {}
          }
        }
        off += n;
      }
      off += 1; // '\n'
      rects.sort((a, b) => a.left - b.left);
      for (const m of mergeLineRects(rects)) {
        const d = document.createElement("div");
        d.className = "nxph-r";
        d.style.left = m.left - vr.left + "px";
        d.style.top = m.top - vr.top + "px";
        d.style.width = m.right - m.left + "px";
        d.style.height = m.bottom - m.top + "px";
        hl.appendChild(d);
      }
    }
  };

  // ---- Selection state ----

  const selText = () => {
    if (!anchor || !caret || anchor.shapeId !== caret.shapeId) return "";
    if (anchor.offset === caret.offset) return "";
    const shapeEl = shapeElOf(anchor.shapeId);
    if (!shapeEl) return "";
    const from = Math.min(anchor.offset, caret.offset);
    const to = Math.max(anchor.offset, caret.offset);
    const sp = shapeToDomPos(shapeEl, from);
    const ep = shapeToDomPos(shapeEl, to);
    if (!sp || !ep) return "";
    try {
      const r = document.createRange();
      r.setStart(sp.node, sp.utf16Off);
      r.setEnd(ep.node, ep.utf16Off);
      return r.toString();
    } catch (_) { return ""; }
  };

  const selArg = () => {
    if (!anchor || !caret || anchor.shapeId !== caret.shapeId) return null;
    if (anchor.offset === caret.offset) return null;
    return `${anchor.shapeId}:${anchor.offset}:${caret.offset}`;
  };

  const updateSel = () => {
    if (!anchor || !caret || anchor.shapeId !== caret.shapeId ||
        anchor.offset === caret.offset) {
      el._nxPptxSel = null;
      return;
    }
    el._nxPptxSel = {
      anchorK,
      // Raw JS positions used by s.js (available immediately, no WASM round-trip needed)
      anchorPos: { shapeId: anchor.shapeId, offset: anchor.offset },
      caretPos: { shapeId: caret.shapeId, offset: caret.offset },
      text: selText(),
    };
  };

  const clearSel = () => {
    anchor = null; anchorK = null; el._nxPptxSel = null;
    drawHighlights();
  };

  // ---- Sync model ----

  const syncModel = async (hit) => {
    // Generation counter: discard responses from stale (superseded) calls.
    const mySeq = ++syncSeq;
    const r = await el.shellAction("pptx-caret", `${hit.shapeId}:${hit.offset}`);
    if (syncSeq !== mySeq) return; // stale — a newer syncModel started
    if (r && r.k) {
      caretK = r.k;
      const [sid, off] = r.k;
      caret = { shapeId: sid, offset: off };
      updateSel();
      drawHighlights();
      place(true);
    }
  };

  const setFromK = (k) => {
    caretK = k || null;
    if (!k) { caret = null; drawHighlights(); place(true); return; }
    const [sid, off] = k;
    caret = { shapeId: sid, offset: off };
    updateSel();
    drawHighlights();
    place(true);
  };

  const editable = () => el.readonly === false && el.document;

  // ---- Pointer events ----

  listen(v, "pointerdown", (e) => {
    if (e.button !== 0 || !editable()) return;
    // Wrap editorKey now so the selection-aware path is live before any
    // beforeinput/insertText event that follows a click (soft keyboard, IME).
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

    const hit = hitAt(e.clientX, e.clientY);
    if (!hit) { clearSel(); caret = null; caretK = null; place(true); return; }

    dragStarted = false; // reset for this new click/drag sequence

    if (detail >= 3) {
      // Triple-click: select entire shape text
      e.preventDefault();
      const mySeq = ++syncSeq;
      const shapeEl = shapeElOf(hit.shapeId);
      if (!shapeEl) return;
      let total = 0;
      for (const para of parasOf(shapeEl)) {
        for (const span of spansOf(para)) total += cpLen(span.firstChild.data);
        total += 1; // '\n'
      }
      if (total > 0) total -= 1; // no trailing '\n'
      anchor = { shapeId: hit.shapeId, offset: 0 };
      anchorK = null;
      caret = { shapeId: hit.shapeId, offset: total };
      caretK = null;
      v.setPointerCapture(e.pointerId);
      updateSel(); place(true); drawHighlights();
      void (async () => {
        const [ra, rc] = await Promise.all([
          el.shellAction("pptx-caret", `${hit.shapeId}:0`),
          el.shellAction("pptx-caret", `${hit.shapeId}:${total}`),
        ]);
        if (syncSeq !== mySeq) return;
        anchorK = ra && ra.k;
        if (rc && rc.k) { caretK = rc.k; const [, o] = rc.k; caret = { shapeId: hit.shapeId, offset: o }; }
        updateSel(); drawHighlights();
      })();
      return;
    }

    if (detail === 2) {
      // Double-click: select word
      e.preventDefault();
      const mySeq = ++syncSeq;
      const shapeEl = shapeElOf(hit.shapeId);
      if (!shapeEl) return;
      const text = shapeTextOf(hit.shapeId);
      const cps = [...text];
      const pos = Math.min(hit.offset, cps.length);
      const isW = (ch) => /\w/.test(ch);
      let start = pos, end = pos;
      const inW = (pos < cps.length && isW(cps[pos])) || (pos > 0 && isW(cps[pos - 1]));
      if (inW) {
        start = pos > 0 && isW(cps[pos - 1]) ? pos - 1 : pos;
        while (start > 0 && isW(cps[start - 1])) start--;
        while (end < cps.length && isW(cps[end])) end++;
      }
      if (start === end && start < cps.length) end = start + 1;
      anchor = { shapeId: hit.shapeId, offset: start };
      anchorK = null;
      caret = { shapeId: hit.shapeId, offset: end };
      caretK = null;
      v.setPointerCapture(e.pointerId);
      updateSel(); place(true); drawHighlights();
      void (async () => {
        const [ra, rc] = await Promise.all([
          el.shellAction("pptx-caret", `${hit.shapeId}:${start}`),
          el.shellAction("pptx-caret", `${hit.shapeId}:${end}`),
        ]);
        if (syncSeq !== mySeq) return;
        anchorK = ra && ra.k;
        if (rc && rc.k) { caretK = rc.k; const [, o] = rc.k; caret = { shapeId: hit.shapeId, offset: o }; }
        updateSel(); drawHighlights();
      })();
      return;
    }

    if (e.shiftKey && caret && caret.shapeId === hit.shapeId) {
      // Shift+click: extend text selection without letting pptx_drag toggle-deselect the shape.
      e.stopPropagation();
      e.preventDefault();
      el._nxpTa?.focus({ preventScroll: true });
      if (!anchor) { anchor = { shapeId: caret.shapeId, offset: caret.offset }; anchorK = caretK; }
      caret = { shapeId: hit.shapeId, offset: hit.offset };
      updateSel(); drawHighlights(); place(true);
      void syncModel(hit).then(() => { drawHighlights(); updateSel(); });
      return;
    }

    // Single click: set anchor optimistically
    textDragActive = !!hit.direct;
    anchor = { shapeId: hit.shapeId, offset: hit.offset };
    anchorK = null;
    el._nxPptxSel = null;
    caret = { shapeId: hit.shapeId, offset: hit.offset };
    v.setPointerCapture(e.pointerId);
    place(true);
    const clickSeq = syncSeq + 1;
    void syncModel(hit).then(() => {
      if (syncSeq !== clickSeq) return;
      // After model confirms, anchorK = caretK for accurate selection reporting.
      // Do NOT set anchor = caret: pointerup already cleared anchor for simple
      // clicks. Re-setting it here triggers selection-delete on the next typed
      // char (#561 typing fix). For drags, anchor is maintained by pointermove.
      anchorK = caretK;
    });
  }, true);

  listen(v, "pointermove", (e) => {
    if (!v.hasPointerCapture(e.pointerId) || !editable()) return;
    if (textDragActive) e.stopPropagation();
    dragStarted = true; // a drag is in progress
    const hit = hitAt(e.clientX, e.clientY);
    if (!hit || !anchor || hit.shapeId !== anchor.shapeId) return;
    caret = { shapeId: hit.shapeId, offset: hit.offset };
    place(true); drawHighlights(); updateSel();
    void syncModel(hit);
  }, true);

  listen(v, "pointerup", (e) => {
    if (!v.hasPointerCapture(e.pointerId)) return;
    textDragActive = false;
    v.releasePointerCapture(e.pointerId);
    if (anchor && caret && anchor.shapeId === caret.shapeId && anchor.offset === caret.offset) {
      anchor = null; anchorK = null; el._nxPptxSel = null; drawHighlights();
    }
  }, true);

  // ---- Keyboard handling ----

  // Saved pre-replace state for single-step undo restoration (#561).
  let preReplaceState = null;

  const wrapDoc = () => {
    const d = el.document;
    if (!d || d._nxpc) return;
    d._nxpc = 1;
    const orig = d.editorKey.bind(d);
    d.editorKey = async (ev, mac) => {
      const hasSelection = anchor && caret && anchor.shapeId === caret.shapeId &&
        anchor.offset !== caret.offset;
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
          el._nxPptxSel = saved.pptxSel;
          updateSel();
          drawHighlights();
          place(true);
        }
        return r;
      }

      if (hasSelection && isTyping) {
        // Atomic replace: delete selection + insert char in one undo step (#561).
        const arg = selArg();
        if (arg) {
          preReplaceState = {
            anchor: anchor ? { ...anchor } : null,
            caret: caret ? { ...caret } : null,
            anchorK,
            caretK,
            pptxSel: el._nxPptxSel ? { ...el._nxPptxSel, anchorPos: el._nxPptxSel.anchorPos && { ...el._nxPptxSel.anchorPos }, caretPos: el._nxPptxSel.caretPos && { ...el._nxPptxSel.caretPos } } : null,
          };
          const cr = await el.shellAction("pptx-sel-replace", `${arg}:${ev.key.codePointAt(0)}`);
          anchor = null; anchorK = null; el._nxPptxSel = null;
          drawHighlights();
          if (cr && cr.k) setFromK(cr.k);
          return { handled: true, k: cr && cr.k };
        }
      }

      if (hasSelection && (isDelete || isEnter)) {
        const arg = selArg();
        if (arg) {
          preReplaceState = isDelete ? {
            anchor: anchor ? { ...anchor } : null,
            caret: caret ? { ...caret } : null,
            anchorK,
            caretK,
            pptxSel: el._nxPptxSel ? { ...el._nxPptxSel, anchorPos: el._nxPptxSel.anchorPos && { ...el._nxPptxSel.anchorPos }, caretPos: el._nxPptxSel.caretPos && { ...el._nxPptxSel.caretPos } } : null,
          } : null;
          const dr = await el.shellAction("pptx-sel-delete", arg);
          anchor = null; anchorK = null; el._nxPptxSel = null;
          drawHighlights();
          if (dr && dr.k) setFromK(dr.k);
          if (isDelete) return { handled: true, k: dr && dr.k };
        }
      }
      if (!hasSelection && !isUndo && (isTyping || isDelete || isEnter)) {
        preReplaceState = null;
      }
      const r = await orig(ev, mac);
      if (r && r.handled) {
        if (r.k) setFromK(r.k);
        // Clear selection on any handled non-typing key
        if (anchor) { anchor = null; anchorK = null; el._nxPptxSel = null; drawHighlights(); }
      }
      return r;
    };
  };

  listen(v, "keydown", (e) => {
    if (!editable()) return;
    wrapDoc();

    // Handle arrow-key caret navigation within a text box directly (pptx type_key
    // treats Arrow keys as slide navigation, not text caret movement).
    if (!caret || e.ctrlKey || e.metaKey || e.altKey) return;
    if (!["ArrowLeft", "ArrowRight"].includes(e.key)) return;

    e.preventDefault();
    e.stopImmediatePropagation(); // prevent slide navigation via editorKey

    const delta = e.key === "ArrowRight" ? 1 : -1;
    const text = shapeTextOf(caret.shapeId);
    const len = [...text].length;
    const newOff = Math.max(0, Math.min(len, caret.offset + delta));

    if (e.shiftKey) {
      // Shift+Arrow: extend selection
      if (!anchor) { anchor = { shapeId: caret.shapeId, offset: caret.offset }; anchorK = caretK; }
      if (newOff !== caret.offset) {
        caret = { shapeId: caret.shapeId, offset: newOff };
        void syncModel(caret);
        updateSel(); drawHighlights(); place(true);
      }
    } else {
      // Plain arrow: move caret, clear selection
      if (anchor) { anchor = null; anchorK = null; el._nxPptxSel = null; drawHighlights(); }
      if (newOff !== caret.offset) {
        caret = { shapeId: caret.shapeId, offset: newOff };
        void syncModel(caret);
        place(true);
      }
    }
  }, true);

  // ---- Observers ----

  const mo = new MutationObserver(() => { updateSel(); place(true); drawHighlights(); });
  mo.observe(dom, { childList: true, subtree: true });
  listen(dom, "scroll", () => { place(true); drawHighlights(); }, { passive: true });
  const resize = typeof ResizeObserver !== "undefined" ? new ResizeObserver(() => { place(true); drawHighlights(); }) : null;
  resize?.observe(v);

  // ---- Focus ----

  let focusGen = 0;
  listen(v, "focusin", () => { focusGen++; place(true); });
  listen(v, "focusout", () => {
    const gen = ++focusGen;
    let left = 0;
    const step = () => {
      if (gen !== focusGen) return;
      const root = v.getRootNode();
      const cur = root && root.activeElement;
      const inside = cur && (cur === v || v.contains(cur));
      if (inside) { place(true); return; }
      if (left++ < 10) { requestAnimationFrame(step); return; }
      if (gen === focusGen) bar.style.display = "none";
    };
    requestAnimationFrame(step);
  });

  // ---- Shell action interception (undo selection restoration + caret sync) ----

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
      el._nxPptxSel = saved.pptxSel;
      drawHighlights();
      place(true);
      if (caret) void origAction("pptx-caret", `${caret.shapeId}:${caret.offset}`);
      return r;
    }
    if (op === "cmd" && editable()) preReplaceState = null;
    const r = await origAction(op, arg);
    if (op === "cmd" && editable()) {
      anchor = null; anchorK = null; el._nxPptxSel = null;
      const c = await origAction("pptx-caret", "");
      setFromK(c && c.k);
    }
    return r;
  };

  // ---- Detach ----

  el._detachPptxCaret = () => {
    listeners.abort(); mo.disconnect(); resize?.disconnect();
    st.remove(); bar.remove(); hl.remove();
    el.shellAction = originalAction;
    el._nxpc = 0; el._nxPptxSel = null;
  };
}
