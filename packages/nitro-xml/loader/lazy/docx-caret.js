// SPDX-License-Identifier: MIT
/**
 * Docx caret (lazy; off the eager budget): pointer hit-test, blinking caret overlay, and the
 * geometry-based keys (Up/Down/Home/End). The model caret lives in Rust (`docx-caret` shell op);
 * this module only maps pixels to "N rendered chars into block B" and back.
 *
 * Blocks carry `data-bid`; a paragraph split across pages has several blocks with the same id.
 * Rendered text can start with a list label the model does not have, so positions are exchanged
 * as (chars before, rendered total) and Rust subtracts the difference from the model length.
 */
const STYLE =
  ".nxc{position:absolute;z-index:5;pointer-events:none;width:1.5px;margin-left:-.75px;background:#000;" +
  "animation:nxcb 1.06s steps(1) infinite;display:none}@keyframes nxcb{50%{opacity:0}}";

export function attach(el) {
  if (el._nxc) return;
  const sr = el.shadowRoot;
  const v = sr && sr.querySelector(".v");
  const dom = sr && sr.querySelector(".dom");
  if (!v || !dom) return;
  el._nxc = 1;
  const st = document.createElement("style");
  st.textContent = STYLE;
  sr.appendChild(st);
  const bar = document.createElement("div");
  bar.className = "nxc";
  bar.part = "caret";
  v.appendChild(bar);

  let caret = null; // { bid, shown } shown = rendered chars before the caret in the block
  let lineStart = false; // caret sits at the start of a wrapped line (not the end of the previous)
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

  /** Hit-test inside one line at client x -> { bid, shown, lineStart, x }. */
  const hitLine = (line, x) => {
    const block = line.closest("[data-bid]");
    const bid = block.dataset.bid;
    const { list, total } = shownIndex(bid);
    const mine = new Set(spansOf(line));
    const first = list.findIndex((e) => mine.has(e.s));
    if (first < 0) {
      // Empty line: caret at the start of the block (or after the preceding text).
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
          let w = 0;
          // Char width via the next boundary; the last char falls back to the span's right edge.
          const nb = k + 1 < cps.length ? boundaryRect(e.s, k + 1).x : r.right;
          w = nb - b.x;
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
      if (d < bd) {
        bd = d;
        best = l;
      }
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
      if (t > e.start && t <= e.start + e.n) {
        k = i;
        break;
      }
      if (t === 0) {
        k = 0;
        break;
      }
    }
    if (k < 0) k = list.length - 1;
    let e = list[k];
    const next = list[k + 1];
    if (lineStart && next && t === e.start + e.n && next.s.parentNode !== e.s.parentNode) e = next;
    return boundaryRect(e.s, Math.max(0, Math.min(e.n, t - e.start)));
  };

  // WebKit often reports a 0-height range on the same turn the page is replaced.
  // One failed place() used to leave the bar at display:none for the click check.
  // 40 frames covers a slow layout without giving up while the click round-trip is in flight.
  let placeTries = 0;
  const place = () => {
    const r = rectOf();
    const d = dom.getBoundingClientRect();
    if (!r || !r.h) {
      bar.style.display = "none";
      if (caret && placeTries < 40) {
        placeTries++;
        requestAnimationFrame(() => place());
      }
      return;
    }
    placeTries = 0;
    if (reveal) {
      reveal = false;
      const pad = 24;
      if (r.top < d.top + pad) dom.scrollTop -= d.top + pad - r.top;
      else if (r.top + r.h > d.bottom - pad) dom.scrollTop += r.top + r.h - (d.bottom - pad);
      return place();
    }
    const vr = v.getBoundingClientRect();
    const visible = r.top + r.h > d.top && r.top < d.bottom && r.x >= d.left && r.x <= d.right;
    bar.style.display = visible ? "block" : "none";
    bar.style.left = r.x - vr.left + "px";
    bar.style.top = r.top - vr.top + "px";
    bar.style.height = r.h + "px";
    for (const a of bar.getAnimations()) {
      a.cancel();
      a.play();
    }
  };

  const setFromK = (k, keepGoal) => {
    if (!keepGoal) goalX = null;
    if (!k) {
      caret = null;
      return place();
    }
    const [bid, chars, model] = k;
    const { total } = shownIndex(bid);
    const shown = Math.max(0, total - model) + chars;
    if (caret && caret.bid !== bid) lineStart = false;
    caret = { bid, shown };
    place();
  };

  const syncModel = async (hit) => {
    const r = await el.shellAction("docx-caret", `${hit.bid}:${hit.shown}:${hit.total}`);
    setFromK(r && r.k, true);
  };

  /** Move the caret to a hit result: optimistic overlay first, then the model. */
  const commit = (hit) => {
    caret = { bid: hit.bid, shown: hit.shown };
    lineStart = hit.lineStart;
    placeTries = 0;
    place();
    return syncModel(hit);
  };

  const editable = () => el.readonly === false && el.document;

  v.addEventListener(
    "pointerdown",
    (e) => {
      if (e.button !== 0 || !editable()) return;
      if (el.document) el.document._nxCaretK = null;
      const d = dom.getBoundingClientRect();
      if (e.clientX - d.left > dom.clientWidth || e.clientY - d.top > dom.clientHeight) return; // scrollbar
      const line = lineAt(e.clientX, e.clientY);
      if (!line) return;
      goalX = null;
      reveal = false;
      void commit(hitLine(line, e.clientX));
    },
    true,
  );

  const wrapDoc = () => {
    const d = el.document;
    if (!d || d._nxc) return;
    d._nxc = 1;
    const orig = d.editorKey.bind(d);
    // The key result arrives before the measured frame replaces the page. Placing
    // against that pre-measure root hides the bar; the follow-up tick calls
    // _nxReCaret once the spans exist so the caret survives the repaint.
    d._nxCaretK = null;
    d._nxCaretTries = 0;
    d._nxReCaret = () => {
      const k = d._nxCaretK;
      if (!k) return;
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
      const r = await orig(ev, mac);
      if (r && r.handled) {
        reveal = true;
        goalX = null;
        lineStart = false;
        d._nxCaretK = r.k || null;
        d._nxCaretTries = 0;
        setFromK(r.k, false);
        if (bar.style.display === "block") d._nxCaretK = null;
      }
      return r;
    };
  };

  v.addEventListener(
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
      void commit(hitLine(target, x)).then(() => {
        goalX = vertical ? x : null;
      });
      if (vertical) goalX = x;
    },
    true,
  );

  const mo = new MutationObserver(place);
  mo.observe(dom, { childList: true });
  dom.addEventListener("scroll", place, { passive: true });
  if (typeof ResizeObserver !== "undefined") new ResizeObserver(place).observe(v);
  // focusin/out: focus may move between .v and the input proxy without leaving the viewer.
  // WebKit often reports relatedTarget as null for that move inside the shadow tree, which
  // used to hide the caret after a click and leave it hidden.
  v.addEventListener("focusin", place);
  v.addEventListener("focusout", (e) => {
    if (e.relatedTarget && v.contains(e.relatedTarget)) return;
    requestAnimationFrame(() => {
      const cur = v.getRootNode() && v.getRootNode().activeElement;
      if (cur && v.contains(cur)) {
        place();
        return;
      }
      bar.style.display = "none";
    });
  });

  const origAction = el.shellAction.bind(el);
  el.shellAction = async (op, arg) => {
    const r = await origAction(op, arg);
    if (op === "cmd" && editable()) {
      const c = await origAction("docx-caret", "");
      setFromK(c && c.k, false);
    }
    return r;
  };
}
