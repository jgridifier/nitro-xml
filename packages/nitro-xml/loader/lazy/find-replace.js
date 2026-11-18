// SPDX-License-Identifier: MIT
// Word's replace controls load with the editing input proxy; no eager loader bytes.
export function attachReplace(el) {
  if (el.localName !== 'nitro-docx' || el._replaceAttached) return;
  el._replaceAttached = true;
  const root = el.shadowRoot;
  let expanded = false, replacement = '', message = '', busy = false;
  const style = document.createElement('style');
  style.textContent = '.nx-replace{flex-basis:100%;display:flex;align-items:center;gap:.4rem;flex-wrap:wrap}.nx-replace input{min-width:8rem}.nx-replace-status{font:inherit}';
  root.append(style);
  const render = () => {
    const bar = root.querySelector('.f');
    if (!bar || bar.querySelector('[data-replace-toggle]')) return;
    const toggle = document.createElement('button');
    toggle.type = 'button';
    toggle.textContent = 'Replace';
    toggle.dataset.replaceToggle = '';
    toggle.setAttribute('aria-expanded', String(expanded));
    toggle.onclick = () => { expanded = !expanded; message = ''; refresh(); };
    bar.querySelector('.q').after(toggle);
    if (!expanded) return;
    const row = document.createElement('div');
    row.className = 'nx-replace';
    row.innerHTML = '<input type="text" aria-label="Replace with" placeholder="Replace with"><button type="button">Replace</button><button type="button">Replace all</button><span class="nx-replace-status" role="status" aria-live="polite"></span>';
    const input = row.querySelector('input');
    input.value = replacement;
    input.oninput = () => { replacement = input.value; };
    row.querySelector('[role=status]').textContent = message;
    row.querySelectorAll('button').forEach((button, i) => {
      button.disabled = busy;
      button.onclick = () => replace(i === 1);
    });
    row.addEventListener('keydown', e => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'z') {
        e.preventDefault(); e.stopPropagation(); message = '';
        void el.shellAction('cmd', e.shiftKey ? 'edit.redo' : 'edit.undo');
      } else if (e.key === 'Escape') {
        e.preventDefault(); e.stopPropagation(); void el.shellAction('find-close');
      } else if (e.key === 'Enter') {
        e.preventDefault(); e.stopPropagation(); void replace(false);
      } else e.stopPropagation();
    });
    bar.append(row);
  };
  const refresh = () => {
    root.querySelector('[data-replace-toggle]')?.remove();
    root.querySelector('.nx-replace')?.remove();
    render();
  };
  const replace = async all => {
    if (busy || el.readonly || !el.document) return;
    const query = root.querySelector('.f .q')?.value || '';
    if (!query) return;
    busy = true;
    message = '';
    try {
      const result = await el.document.applyEdit(JSON.stringify({
        command: all ? 'search.replaceAll' : 'search.replaceOne', query, replacement,
        index: Math.max(0, (parseInt(root.querySelector('.fs')?.textContent, 10) || 1) - 1),
      }));
      const data = typeof result === 'string' ? JSON.parse(result) : result;
      await el.shellAction('find-query', query);
      message = `Replaced ${data.replaced}`;
    } catch (error) { message = error.message || String(error); }
    finally {
      busy = false; refresh();
      root.querySelector('[aria-label="Replace with"]')?.focus();
    }
  };
  el.addEventListener('keydown', e => {
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === 'h') {
      e.preventDefault(); e.stopImmediatePropagation();
      expanded = true;
      void el.shellAction('find-open').then(() => {
        refresh(); root.querySelector('[aria-label="Replace with"]')?.focus();
      });
    }
  }, true);
  root.addEventListener('input', e => {
    if (e.target.matches('.f .q')) {
      message = '';
      const status = root.querySelector('.nx-replace-status');
      if (status) status.textContent = '';
    }
  });
  new MutationObserver(render).observe(el._ch, { childList: true, subtree: true });
  render();
}
