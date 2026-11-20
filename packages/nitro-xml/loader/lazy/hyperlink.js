// SPDX-License-Identifier: MIT
/** Host owns navigation, including shell-triggered links. */
export function emit(el, link) {
  el.dispatchEvent(new CustomEvent('nitro-hyperlink', {
    detail: { href: link.href ?? link.target ?? (link.location ? `#${link.location}` : ''), text: link.text ?? '' },
    bubbles: true, composed: true,
  }));
}

export function attachHyperlinks(el) {
  if (el._hyperlinksAttached) return;
  el._hyperlinksAttached = true;
  const selector = '[data-nitro-href], a[href], .xg-link, [data-run]';
  let pressed = null;
  el.shadowRoot.addEventListener('pointerdown', e => { pressed = e.target.closest?.(selector); }, true);
  el.shadowRoot.addEventListener('click', async e => {
    const node = e.target.closest?.(selector) || pressed;
    pressed = null;
    if (!node || !node.closest('.dom')) return;
    e.preventDefault();
    const block = node.closest('[data-bid]');
    const text = node.dataset.run !== undefined && block
      ? [...block.querySelectorAll(`[data-run="${node.dataset.run}"]`)].map(n => n.textContent).join('')
      : node.textContent || '';
    const href = node.dataset.nitroHref || node.getAttribute('href');
    if (href) return emit(el, { href, text });
    let result;
    if (node.matches('.xg-link')) {
      const match = /^cell-([A-Z]+)(\d+)$/.exec(node.dataset.testid || '');
      if (!match) return;
      let col = 0;
      for (const c of match[1]) col = col * 26 + c.charCodeAt(0) - 64;
      const ui = await el.document.shellAction('ui');
      const sheet = ui.ui.tabs.find(t => t.active)?.index || 0;
      result = await el.document.shellAction('hyperlink-at', `${sheet},${+match[2] - 1},${col - 1}`);
    } else if (node.dataset.run !== undefined) {
      const block = node.closest('[data-bid]');
      if (block) result = await el.document.shellAction('hyperlink-block', `${block.dataset.bid},${node.dataset.run}`);
    }
    if (result?.p) emit(el, { ...result.p, text });
  }, true);
  el.shadowRoot.addEventListener('auxclick', e => {
    if (e.target.closest?.('a[href], [data-nitro-href]')) e.preventDefault();
  }, true);
}
