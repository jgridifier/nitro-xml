// SPDX-License-Identifier: MIT
/** Find through the engine; map the resulting paragraph to its rendered range. */
export async function scrollToText(el, text, { occurrence = 0, matchCase = false, select = false } = {}) {
  if (typeof text !== 'string') throw new TypeError('text must be a string');
  if (!Number.isInteger(occurrence) || occurrence < 0) throw new RangeError('occurrence must be a non-negative integer');
  if (!text || !el.document) return null;
  await el.shellAction('find-opts', `case=${matchCase ? 1 : 0}&entire=0`);
  await el.shellAction('find-query', text);
  let result = await el.shellAction('ui');
  if (occurrence >= result.ui.find.count) return null;
  for (let i = 0; i < occurrence; i++) await el.shellAction('find-next');
  result = await el.shellAction('ui');
  const hit = result.ui.find.current;
  await el.document.tick();
  // Find chrome repaint must finish before retaining DOM ranges.
  while (el._raf) await new Promise(requestAnimationFrame);
  const dom = el.shadowRoot.querySelector('.dom');
  const ids = [...new Set([...dom.querySelectorAll('.dve-print-column .dve-print-block[data-bid]')].map(n => n.dataset.bid).filter(Boolean))];
  const bid = ids[hit.row];
  const blocks = [...dom.querySelectorAll('[data-bid]')].filter(n => n.dataset.bid === bid);
  const nodes = blocks.flatMap(b => [...b.querySelectorAll('.dve-print-line > span')]).map(s => s.firstChild).filter(n => n?.nodeType === 3);
  // Engine offsets count Unicode scalars; DOM Range offsets count UTF-16 units.
  let offset = hit.col;
  let remaining = [...text].length;
  const range = document.createRange();
  let started = false;
  for (const node of nodes) {
    const chars = [...node.data];
    if (!started) {
      if (offset >= chars.length) { offset -= chars.length; continue; }
      range.setStart(node, chars.slice(0, offset).join('').length);
      started = true;
    }
    const take = Math.min(remaining, chars.length - offset);
    range.setEnd(node, chars.slice(0, offset + take).join('').length);
    remaining -= take;
    offset = 0;
    if (!remaining) break;
  }
  if (!started || remaining) throw new Error('Find match has no rendered text range');
  const rect = range.getBoundingClientRect();
  const view = dom.getBoundingClientRect();
  dom.scrollTop += rect.top - view.top - Math.max(0, (dom.clientHeight - rect.height) / 2);
  dom.scrollLeft += Math.min(0, rect.left - view.left);
  if (select) {
    const selection = el.shadowRoot.getSelection?.() || window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  }
  return { page: Number(range.startContainer.parentElement.closest('[data-page-index]').dataset.pageIndex), paragraph: hit.row, scrollTop: dom.scrollTop, scrollLeft: dom.scrollLeft, zoom: result.ui.zoom };
}
