// SPDX-License-Identifier: MIT
// Word count for docx (lazy, off the eager budget). Attaches to a nitro-docx
// element already mounted in a ribbon frame; updates the [data-wc] status bar
// button and powers the Word Count dialog on click.
//
// Text is read from the rendered DOM (all pages are present for docx — no virtual
// scroll). Words are whitespace-separated tokens (matches Word's counting rule).

function countWords(text) {
  if (!text) return 0;
  return (text.match(/\S+/g) || []).length;
}

function blockText(block) {
  // Spans are absolutely positioned — no implicit whitespace at line boundaries.
  // "and" ending line N and "WebAssembly" starting line N+1 merge to
  // "andWebAssembly" in block.textContent. Join each .dve-print-line text
  // with a space to restore the missing inter-line separator.
  const parts = [];
  for (const line of block.querySelectorAll(".dve-print-line")) {
    const t = line.textContent;
    if (t) parts.push(t);
  }
  return parts.join(" ");
}

function getStats(el, pageCount) {
  const sr = el.shadowRoot;
  if (!sr) return null;
  const dom = sr.querySelector(".dom");
  if (!dom) return null;
  const lines = dom.querySelectorAll(".dve-print-line").length;
  if (!lines) return null;
  const seenBid = new Set();
  let text = "";
  for (const block of dom.querySelectorAll("[data-bid]")) {
    // Split-page continuations share a bid but have distinct text; join all.
    const bt = blockText(block);
    text += (text ? " " : "") + bt;
    seenBid.add(block.dataset.bid);
  }
  return {
    pages: pageCount || 1,
    words: countWords(text),
    charsNoSpaces: (text.match(/\S/g) || []).length,
    charsWithSpaces: text.replace(/\n/g, "").length,
    paragraphs: seenBid.size,
    lines,
  };
}

function getSelWords(el) {
  const sel = window.getSelection ? window.getSelection() : null;
  if (!sel || sel.isCollapsed) return null;
  const sr = el.shadowRoot;
  if (!sr) return null;
  const dom = sr.querySelector(".dom");
  if (!dom) return null;
  const range = sel.rangeCount > 0 ? sel.getRangeAt(0) : null;
  if (!range) return null;
  const anc = range.commonAncestorContainer;
  if (!dom.contains(anc)) return null;
  return countWords(sel.toString());
}

function showDialog(stats, anchor, container) {
  const existing = container.querySelector(".wcd-trap");
  if (existing) { existing.remove(); return; } // toggle

  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => "&#" + c.charCodeAt(0) + ";");
  const row = (lbl, val) =>
    `<div class="wcd-row"><span class="wcd-lbl">${esc(lbl)}</span><span class="wcd-val">${esc(String(val))}</span></div>`;

  const trap = document.createElement("div");
  trap.className = "wcd-trap";
  trap.setAttribute("role", "presentation");
  trap.innerHTML = `<div role="dialog" aria-modal="true" aria-labelledby="wcd-title" class="wcd">
<h2 class="wcd-h" id="wcd-title">Word Count</h2>
<div class="wcd-rows">
${row("Pages", stats.pages)}
${row("Words", stats.words)}
${row("Characters (no spaces)", stats.charsNoSpaces)}
${row("Characters (with spaces)", stats.charsWithSpaces)}
${row("Paragraphs", stats.paragraphs)}
${stats.lines != null ? row("Lines", stats.lines) : ""}
</div>
<div class="wcd-foot"><button type="button" class="wcd-close">Close</button></div>
</div>`;

  container.appendChild(trap);
  const dialog = trap.querySelector(".wcd");
  const closeBtn = trap.querySelector(".wcd-close");
  closeBtn.focus();

  function close() {
    trap.remove();
    if (anchor && anchor.isConnected) anchor.focus();
  }

  closeBtn.addEventListener("click", close);
  trap.addEventListener("click", (e) => { if (e.target === trap) close(); });

  // Minimal focus trap — only the Close button is focusable here
  dialog.addEventListener("keydown", (e) => {
    if (e.key === "Escape") { e.preventDefault(); close(); }
  });
}

export function attach(el, container) {
  if (el._nxWc) return;
  el._nxWc = 1;

  const btn = container.querySelector("[data-wc]");
  if (!btn) return;

  let totalWords = 0;
  let pageCount = 1;
  let timer = 0;

  function update() {
    // Read page count from the Rust chrome tab strip
    const ch = el._ch;
    if (ch) {
      const tb = [...ch.querySelectorAll(".t button")];
      pageCount = tb.length || pageCount;
    }
    const stats = getStats(el, pageCount);
    if (!stats) { btn.hidden = true; return; }
    totalWords = stats.words;
    const selW = getSelWords(el);
    btn.textContent = selW != null
      ? `${selW} of ${totalWords} words`
      : `${totalWords} words`;
    btn.hidden = false;
    btn._wcStats = stats;
  }

  function schedule() {
    clearTimeout(timer);
    timer = setTimeout(() => {
      if (typeof requestIdleCallback === "function") {
        requestIdleCallback(update, { timeout: 500 });
      } else {
        update();
      }
    }, 300);
  }

  const sr = el.shadowRoot;
  if (!sr) { el._nxWc = 0; return; }
  const dom = sr.querySelector(".dom");
  if (!dom) { el._nxWc = 0; return; }

  // Observe DOM mutations (edits, new page paints, re-layouts)
  const mo = new MutationObserver(schedule);
  mo.observe(dom, { childList: true, subtree: true, characterData: true });

  // Native text selection for read-only viewer
  const onSel = () => {
    if (btn.hidden) return;
    const selW = getSelWords(el);
    btn.textContent = selW != null
      ? `${selW} of ${totalWords} words`
      : `${totalWords} words`;
  };
  document.addEventListener("selectionchange", onSel);

  btn.addEventListener("click", () => {
    const stats = btn._wcStats || getStats(el, pageCount);
    if (stats) showDialog(stats, btn, container);
  });

  schedule(); // first pass once DOM may already have content

  el._nxWcDetach = () => {
    mo.disconnect();
    document.removeEventListener("selectionchange", onSel);
    clearTimeout(timer);
    el._nxWc = 0;
    el._nxWcDetach = null;
  };
}
