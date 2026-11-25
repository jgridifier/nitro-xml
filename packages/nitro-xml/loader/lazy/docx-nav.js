// SPDX-License-Identifier: MIT
// Docx Navigation pane (#607): Headings, Pages, and Results tabs.
// Loaded via dynamic import() from the Office ribbon frame; never on the eager budget.

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => "&#" + c.charCodeAt(0) + ";");
const svgUp = '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" style="width:14px;height:14px;fill:none;stroke:currentColor;stroke-width:1.8;stroke-linecap:round;stroke-linejoin:round"><path d="M18 15l-6-6-6 6"/></svg>';
const svgDown = '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" style="width:14px;height:14px;fill:none;stroke:currentColor;stroke-width:1.8;stroke-linecap:round;stroke-linejoin:round"><path d="M6 9l6 6 6-6"/></svg>';
const svgClose = '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" style="width:14px;height:14px;fill:none;stroke:currentColor;stroke-width:1.8;stroke-linecap:round;stroke-linejoin:round"><path d="M6 6l12 12M18 6L6 18"/></svg>';

const TABS = ['Headings', 'Pages', 'Results'];

/**
 * Mount the navigation pane into an already-created <aside class="nav-pane"> element.
 * @param {{ pane: HTMLElement, el: HTMLElement, act: (op:string, arg?:string)=>Promise<any> }} opts
 */
export function mountNavPane(opts) {
  const { pane, el, act } = opts;

  // Build pane structure
  pane.innerHTML = `
<div class="nav-tabs" role="tablist" aria-label="Navigation tabs">
  ${TABS.map((t, i) => `<button type="button" role="tab" class="nav-tab" id="nx-nt-${i}" aria-controls="nx-np-${i}" aria-selected="${i === 0 ? 'true' : 'false'}" tabindex="${i === 0 ? 0 : -1}">${t}</button>`).join('')}
</div>
<div class="nav-panels">
  ${TABS.map((_, i) => `<div class="nav-panel" id="nx-np-${i}" role="tabpanel" aria-labelledby="nx-nt-${i}"${i > 0 ? ' hidden' : ''}></div>`).join('')}
</div>`;

  const tabs = [...pane.querySelectorAll('[role=tab]')];
  const panels = TABS.map((_, i) => pane.querySelector(`#nx-np-${i}`));

  let activeTab = 0;
  let headings = [];
  let currentBid = '';

  // ---- tab switching ----
  function selectTab(i, focus) {
    activeTab = i;
    tabs.forEach((t, j) => {
      const on = j === i;
      t.setAttribute('aria-selected', String(on));
      t.tabIndex = on ? 0 : -1;
      if (on && focus) t.focus();
    });
    panels.forEach((p, j) => { p.hidden = j !== i; });
    if (i === 0) refreshHeadings();
    else if (i === 1) refreshPages();
    else if (i === 2) focusResultsSearch();
  }

  // ---- keyboard roving tabindex on tab strip ----
  pane.querySelector('.nav-tabs').addEventListener('keydown', (e) => {
    const idx = tabs.indexOf(e.target);
    if (idx < 0) return;
    if (e.key === 'ArrowRight') { e.preventDefault(); selectTab((idx + 1) % TABS.length, true); }
    else if (e.key === 'ArrowLeft') { e.preventDefault(); selectTab((idx - 1 + TABS.length) % TABS.length, true); }
    else if (e.key === 'Home') { e.preventDefault(); selectTab(0, true); }
    else if (e.key === 'End') { e.preventDefault(); selectTab(TABS.length - 1, true); }
  });
  pane.querySelector('.nav-tabs').addEventListener('click', (e) => {
    const t = e.target.closest('[role=tab]');
    if (t) selectTab(tabs.indexOf(t), false);
  });

  // ---- Headings tab ----
  async function refreshHeadings() {
    if (activeTab !== 0) return;
    try {
      const res = await act('nav-headings');
      headings = (res && res.headings) || [];
    } catch { headings = []; }
    renderHeadings();
  }

  function renderHeadings() {
    const p = panels[0];
    if (!headings.length) {
      p.innerHTML = '<p class="nav-empty">No headings found in this document.</p>';
      return;
    }
    p.innerHTML = headings.map((h) => {
      const cur = h.id === currentBid ? ' aria-current="true"' : '';
      return `<button type="button" class="nav-h-item" data-bid="${esc(h.id)}" data-lv="${h.level}"${cur}>${esc(h.text || '(empty heading)')}</button>`;
    }).join('');
  }

  panels[0].addEventListener('click', (e) => {
    const b = e.target.closest('.nav-h-item');
    if (b) jumpToBid(b.dataset.bid, b);
  });
  panels[0].addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      const b = e.target.closest('.nav-h-item');
      if (b) jumpToBid(b.dataset.bid, b);
    }
  });

  function jumpToBid(bid, btn) {
    if (!bid) return;
    currentBid = bid;
    // Update aria-current without full re-render
    panels[0].querySelectorAll('.nav-h-item').forEach((b) => {
      b.setAttribute('aria-current', b.dataset.bid === bid ? 'true' : 'false');
    });
    // Scroll the WASM document view to the paragraph
    const dom = el._r;
    if (!dom) return;
    const block = dom.querySelector(`[data-bid="${CSS.escape(bid)}"]`);
    if (block) {
      const domRect = dom.getBoundingClientRect();
      const blockRect = block.getBoundingClientRect();
      dom.scrollTop += blockRect.top - domRect.top - 24;
    }
  }

  // ---- Pages tab ----
  function refreshPages() {
    if (activeTab !== 1) return;
    const p = panels[1];
    // Page count from the WASM chrome state tabs
    const ch = el._ch;
    const tb = ch ? [...ch.querySelectorAll('.t button')] : [];
    const at = tb.findIndex((b) => b.getAttribute('aria-current') === 'true');
    if (!tb.length) {
      // Fallback: read from DOM attribute
      const dom = el._r;
      const pages = dom && dom.querySelector('.dve-print-pages');
      const n = pages ? parseInt(pages.getAttribute('data-page-count'), 10) || 1 : 1;
      p.innerHTML = Array.from({ length: n }, (_, i) =>
        `<button type="button" class="nav-pg-item" data-pi="${i}"${i === 0 ? ' aria-current="true"' : ''}><span class="n">${i + 1}</span>Page ${i + 1}</button>`,
      ).join('');
    } else {
      p.innerHTML = tb.map((b, i) =>
        `<button type="button" class="nav-pg-item" data-g="${esc(b.dataset.g || String(i))}"${i === at ? ' aria-current="true"' : ''}><span class="n">${i + 1}</span>${esc(b.textContent || `Page ${i + 1}`)}</button>`,
      ).join('');
    }
  }

  panels[1].addEventListener('click', (e) => {
    const b = e.target.closest('.nav-pg-item');
    if (!b) return;
    // Navigate using the chrome tab's data-g or fallback index
    const g = b.dataset.g;
    const pi = b.dataset.pi;
    if (g != null) void act('sheet', g);
    else if (pi != null) void act('sheet', pi);
  });
  panels[1].addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      const b = e.target.closest('.nav-pg-item');
      if (!b) return;
      const g = b.dataset.g;
      const pi = b.dataset.pi;
      if (g != null) void act('sheet', g);
      else if (pi != null) void act('sheet', pi);
    }
  });

  // ---- Results tab ----
  const resultsPanel = panels[2];

  const srchDiv = document.createElement('div');
  srchDiv.className = 'nav-srch';
  const srchInput = document.createElement('input');
  srchInput.type = 'search';
  srchInput.autocomplete = 'off';
  srchInput.placeholder = 'Search document';
  srchInput.setAttribute('aria-label', 'Search in document');
  srchDiv.appendChild(srchInput);
  resultsPanel.appendChild(srchDiv);

  const srchRow = document.createElement('div');
  srchRow.className = 'nav-srch-row';

  const prevBtn = document.createElement('button');
  prevBtn.type = 'button';
  prevBtn.className = 'nav-srch-prev';
  prevBtn.setAttribute('aria-label', 'Find previous');
  prevBtn.disabled = true;
  prevBtn.innerHTML = svgUp;

  const nextBtn = document.createElement('button');
  nextBtn.type = 'button';
  nextBtn.className = 'nav-srch-next';
  nextBtn.setAttribute('aria-label', 'Find next');
  nextBtn.disabled = true;
  nextBtn.innerHTML = svgDown;

  const statusSpan = document.createElement('span');
  statusSpan.className = 'nav-srch-st';
  statusSpan.setAttribute('aria-live', 'polite');

  const closeBtn = document.createElement('button');
  closeBtn.type = 'button';
  closeBtn.className = 'nav-srch-close';
  closeBtn.setAttribute('aria-label', 'Clear search');
  closeBtn.innerHTML = svgClose;

  srchRow.append(prevBtn, nextBtn, statusSpan, closeBtn);
  resultsPanel.appendChild(srchRow);

  const resList = document.createElement('div');
  resList.className = 'nav-res-list';
  resList.setAttribute('role', 'list');
  resList.setAttribute('aria-label', 'Search results');
  resultsPanel.appendChild(resList);

  let findOpened = false;
  let lastQuery = '';

  async function runFind(query) {
    if (!findOpened) {
      findOpened = true;
      await act('find-open');
    }
    await act('find-query', query);
    // Read ui state for status and matches
    try {
      const r = await act('ui');
      const f = r && r.ui && r.ui.find;
      if (f) {
        statusSpan.textContent = f.status || (f.count ? `${f.count} result${f.count === 1 ? '' : 's'}` : query ? 'No results' : '');
        const hasResults = f.count > 0;
        prevBtn.disabled = !hasResults;
        nextBtn.disabled = !hasResults;
      }
    } catch { /* ignore */ }
  }

  async function closeFindPane() {
    if (findOpened) {
      findOpened = false;
      await act('find-close');
    }
    srchInput.value = '';
    lastQuery = '';
    statusSpan.textContent = '';
    prevBtn.disabled = true;
    nextBtn.disabled = true;
    resList.innerHTML = '';
  }

  srchInput.addEventListener('input', async () => {
    const q = srchInput.value;
    lastQuery = q;
    if (!q) {
      await closeFindPane();
      return;
    }
    await runFind(q);
  });

  srchInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      if (!e.shiftKey) void act('find-next');
      else void act('find-prev');
    } else if (e.key === 'Escape') {
      e.preventDefault();
      void closeFindPane();
    }
    e.stopPropagation();
  });
  srchInput.addEventListener('keyup', (e) => e.stopPropagation());

  nextBtn.addEventListener('click', () => void act('find-next'));
  prevBtn.addEventListener('click', () => void act('find-prev'));
  closeBtn.addEventListener('click', () => void closeFindPane());

  function focusResultsSearch() {
    if (activeTab === 2) {
      // Small delay to allow hidden→visible transition
      setTimeout(() => srchInput.focus(), 50);
    }
  }

  // ---- live update (MutationObserver on chrome) ----
  const ch = el._ch;
  let moTimer = 0;
  function scheduleRefresh() {
    clearTimeout(moTimer);
    moTimer = setTimeout(() => {
      if (activeTab === 0) refreshHeadings();
      else if (activeTab === 1) refreshPages();
      // Update current-page indicator on pages tab
      updateCurrentPage();
    }, 120);
  }

  const mo = new MutationObserver(scheduleRefresh);
  if (ch) mo.observe(ch, { childList: true, subtree: false });

  // Also observe DOM for heading position tracking
  const domEl = el._r;
  let ioTimer = 0;
  if (domEl && typeof IntersectionObserver !== 'undefined') {
    // Track which heading is currently visible
    function findVisibleHeading() {
      if (activeTab !== 0 || !headings.length) return;
      const domRect = domEl.getBoundingClientRect();
      const threshold = domRect.top + 60;
      let visible = '';
      for (const h of headings) {
        const block = domEl.querySelector(`[data-bid="${CSS.escape(h.id)}"]`);
        if (!block) continue;
        const r = block.getBoundingClientRect();
        if (r.top <= threshold) visible = h.id;
        else break;
      }
      if (visible && visible !== currentBid) {
        currentBid = visible;
        panels[0].querySelectorAll('.nav-h-item').forEach((b) => {
          b.setAttribute('aria-current', b.dataset.bid === visible ? 'true' : 'false');
        });
        // Scroll the active heading into view in the nav pane
        const activeBtn = panels[0].querySelector(`.nav-h-item[data-bid="${CSS.escape(visible)}"]`);
        if (activeBtn) activeBtn.scrollIntoView({ block: 'nearest' });
      }
    }
    domEl.addEventListener('scroll', () => {
      clearTimeout(ioTimer);
      ioTimer = setTimeout(findVisibleHeading, 80);
    }, { passive: true });
  }

  function updateCurrentPage() {
    if (activeTab !== 1) return;
    const c = el._ch;
    const tb = c ? [...c.querySelectorAll('.t button')] : [];
    const at = tb.findIndex((b) => b.getAttribute('aria-current') === 'true');
    panels[1].querySelectorAll('.nav-pg-item').forEach((b, i) => {
      b.setAttribute('aria-current', i === at ? 'true' : 'false');
    });
  }

  // Initial load
  refreshHeadings();

  return {
    /** Switch to a named tab ('Headings', 'Pages', 'Results') and optionally focus. */
    switchTo(name, focus = false) {
      const i = TABS.indexOf(name);
      if (i >= 0) selectTab(i, focus);
    },
    /** Focus the Results search input (Ctrl+F handler). */
    focusSearch() {
      selectTab(2, false);
      setTimeout(() => srchInput.focus(), 50);
    },
    /** Refresh all visible state (call after edit/undo/redo). */
    refresh() { scheduleRefresh(); },
    destroy() {
      mo.disconnect();
      clearTimeout(moTimer);
      clearTimeout(ioTimer);
      if (findOpened) void act('find-close');
    },
  };
}
