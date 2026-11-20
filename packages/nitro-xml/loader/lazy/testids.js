// SPDX-License-Identifier: MIT
/** Stable host selectors; observes replacements without altering layout or focus. */
export function attachTestIds(root, format) {
  const put = (node, id) => { if (node.dataset.testid !== id) node.dataset.testid = id; };
  const scan = () => {
    const fixed = { '.v': 'nitro-surface', '.f': 'find-bar', '.q': 'find-input', '.fs': 'find-status', '.sb': 'status-bar', '.foot': 'status', '[data-pos]': 'status-position', '[data-find]': 'status-find', '.zr': 'zoom-control', '.z': 'zoom-value', '.file': 'file-menu', '.fm': 'file-items', '.sheets': 'sheet-tabs', '.nx': 'embedded-toolbar' };
    for (const [selector, id] of Object.entries(fixed)) for (const node of root.querySelectorAll(selector)) put(node, id);
    for (const node of root.querySelectorAll('[data-a], [data-k], [data-t], [data-f], [data-z], [data-q], .nx-b, [role="tab"]')) {
      const d = node.dataset;
      if (d.k && d.testid) continue;
      let id;
      if (d.f) id = `file-${d.f}`;
      else if (d.t || (node.matches('[role="tab"]') && d.a !== 'sheet' && d.g === undefined && d.tab === undefined)) id = `ribbon-tab-${(d.t || node.textContent.trim()).toLowerCase()}`;
      else if (d.a === 'sheet' || node.closest('.sheets')) id = `${format === 'xlsx' ? 'sheet-tab' : format === 'pptx' ? 'slide-thumb' : 'page-tab'}-${d.g}`;
      else if (d.k) id = `ribbon-command-${d.k}`;
      else if (d.a === 'cmd') id = `ribbon-command-${d.g || node.getAttribute('aria-label')}`;
      else if (d.a) id = `shell-${d.a}`;
      else if (d.z) id = `zoom-${d.z}`;
      else if (d.q) id = `host-${d.q}`;
      else if (node.matches('.nx-b')) id = `embedded-${node.getAttribute('aria-label').toLowerCase().replace(/[^a-z0-9]+/g, '-')}`;
      if (id) put(node, id);
    }
    for (const node of root.querySelectorAll('[data-r]')) put(node, `status-${node.dataset.r}`);
    for (const node of root.querySelectorAll('.pl [data-g]')) put(node, `slide-thumb-${node.dataset.g}`);
    for (const node of root.querySelectorAll('.zr input')) put(node, 'zoom-input');
  };
  scan();
  const observer = new MutationObserver(scan);
  observer.observe(root, { childList: true, subtree: true });
  return () => observer.disconnect();
}
