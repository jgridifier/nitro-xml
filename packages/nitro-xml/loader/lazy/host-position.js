// SPDX-License-Identifier: MIT
// Host operations deliberately bypass mut-frame shell wrappers: positioning isn't an edit.
async function action(el, op, arg = '') {
  const d = el.document;
  d._events.flushNow();
  const msg = await d._request('sa', () => d._worker.postMessage({type:'sa', op, arg:String(arg)}));
  const r = JSON.parse(msg.result);
  if (r.h != null) el._ch.innerHTML = r.h;
  return r;
}
export function tabIndex(tabs, value) {
  if (!tabs.length) return null;
  if (typeof value === 'string') {
    const tab = tabs.find(t => t.name === value);
    if (!tab) throw new RangeError('Unknown sheet: ' + value);
    return tab.index;
  }
  if (!Number.isFinite(value)) throw new TypeError('Position index must be finite');
  return tabs[Math.max(0, Math.min(tabs.length - 1, Math.trunc(value)))].index;
}
export async function setPosition(el, pos = {}) {
  const visibility = el._v?.style.visibility;
  if (el._openingPosition) el._v.style.visibility = 'hidden';
  try { await positionBody(el, pos); }
  finally { if (el._openingPosition) el._v.style.visibility = visibility; }
}
async function positionBody(el, pos) {
  if (!el.document) throw new Error('no document');
  const d = el.document;
  if (pos.zoom != null) {
    if (!Number.isFinite(pos.zoom)) throw new TypeError('Zoom must be finite');
    await action(el, 'zoom-set', pos.zoom);
  }
  // DOCX initially exposes only the first-page body slice. Finish layout and tail
  // materialization before clamping a requested page against its final tab count.
  {
    const deadline = performance.now() + 60000;
    for (let i = 0; ; i++) {
      await d.tick();
      await d._refining;
      if (i > 0 && d._positionReady !== false && !d._measureCount && !d._outlineCount) break;
      if (performance.now() > deadline) throw new Error('Position layout did not settle');
    }
  }
  const {ui} = await action(el, 'ui');
  const value = el.localName === 'nitro-xlsx' ? pos.sheet ?? pos.sheetName : el.localName === 'nitro-docx' ? pos.page : pos.slide;
  if (value != null) {
    const index = tabIndex(ui.tabs, value);
    if (index != null) await action(el, 'sheet', index);
  }
  for (const k of ['scrollTop', 'scrollLeft']) if (pos[k] != null && !Number.isFinite(pos[k])) throw new TypeError(k + ' must be finite');
  if (el.localName === 'nitro-xlsx') await action(el, 'host-scroll', JSON.stringify(pos));
  d._nxPainted = 0;
  await d.tick();
  if (el.localName === 'nitro-docx' && value != null && pos.scrollTop == null) {
    const page = el._r.querySelector('[data-nitro-current]');
    if (page) el._r.scrollTop += page.getBoundingClientRect().top - el._r.getBoundingClientRect().top;
  }
  if (el.localName !== 'nitro-xlsx') {
    for (const k of ['scrollTop', 'scrollLeft']) if (pos[k] != null) el._r[k] = Math.max(0, pos[k]);
  }
  d._position = await action(el, 'host-position');
  delete d._position._ready;
}
export async function reload(el, bytes, {keepPosition = true, name} = {}) {
  if (!el.document) throw new Error('no document');
  const position = keepPosition ? el.getPosition() : {scrollTop:0, scrollLeft:0};
  const d = el.document;
  el._dT?.pause();
  if (el._raf) cancelAnimationFrame(el._raf);
  el._raf = 0;
  await d.tick();
  await d._refining;
  await d._tickLock;
  d._rt = 0; d._xf = null; d._nxFollow = 0; d._positionReady = false;
  d._nodes.clear(); d._images.clear(); d._pendingLayoutReqs = null;
  d._eArmed = 0; d._eP = null; d._eR = null; d._nxPainted = 0;
  await d.open(new Uint8Array(bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes)), name || d._name, {readonly:el.readonly});
  await el._dT?.rebase(true);
  el._dy = false;
  await el._ao();
  await setPosition(el, position);
}
export async function setReadonly(el) {
  const d = el.document;
  if (!d || d._ro === el.readonly) return;
  const readonly = el.readonly;
  const position = el.getPosition();
  await d.tick();
  await d._request('host-mode', () => d._worker.postMessage({type:'host-mode', readonly}));
  d._ro = readonly;
  // The mode operation already initialized editing; don't wait for an open-only `er`.
  d._eArmed = 1; d._eR?.(); d._eR = null;
  if (readonly) { el._detachInput?.(); el._detachCaret?.(); }
  else await el._ao();
  if (el._nxpTa) el._nxpTa.disabled = readonly;
  const caret = el.shadowRoot.querySelector('.nxc');
  if (caret && readonly) caret.style.display = 'none';
  await setPosition(el, position);
  el.dispatchEvent(new CustomEvent('nitro-mode', {detail:{readonly}, bubbles:true, composed:true}));
}
