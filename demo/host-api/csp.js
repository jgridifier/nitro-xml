// SPDX-License-Identifier: MIT
import * as loader from '../nitro-xml/loader/index.js';
const params = new URLSearchParams(location.search);
const format = params.get('f') || 'docx';
const samples = { docx: 'notes.docx', xlsx: 'sample.xlsx', pptx: 'deck.pptx' };
window.violations = [];
window.links = [];
window.linkFlags = [];
document.addEventListener("nitro-hyperlink", e => window.linkFlags.push({ bubbles: e.bubbles, composed: e.composed }));
window.errors = [];
document.addEventListener('securitypolicyviolation', e => window.violations.push(`${e.violatedDirective}: ${e.blockedURI}`));
try {
  await loader.initCore({ fontSource: false });
  loader[{docx:'defineNitroDocx',xlsx:'defineNitroXlsx',pptx:'defineNitroPptx'}[format]]();
  const el = window.viewer = document.createElement(`nitro-${format}`);
  el.id = 'viewer';
  el.setAttribute('readonly', 'false');
  el.setAttribute('mode', params.get('mode') || 'full');
  document.querySelector('#editor').append(el);
  if (params.get('mode') === 'full') {
    const { mountOffice } = await import('../nitro-xml/loader/lazy/ribbon/index.js');
    mountOffice({ container: document.querySelector('#editor'), el, format, mode: 'full', edit: true, onMode() {}, onFile() {}, onSample() {} });
  }
  el.addEventListener('error', e => window.errors.push(String(e.detail?.message || e.detail)));
  el.addEventListener('nitro-hyperlink', e => window.links.push(e.detail));
  el.addEventListener('nitro-change', e => document.querySelector('#dirty').textContent = e.detail.dirty ? 'dirty' : 'clean');
  document.querySelector('#selection').onclick = async () => document.querySelector('#log').textContent = JSON.stringify(await el.getSelection());
  document.querySelector('#save').onclick = async () => { window.saved = Array.from(await el.save()); };
  const source = params.get('file') || `../samples/${samples[format]}`;
  const response = await fetch(source);
  if (!response.ok) throw new Error(`Document fetch: ${response.status}`);
  await el.loadBuffer(await response.arrayBuffer(), source.split('/').pop());
  while (!el.document.whenEditReady) await new Promise(requestAnimationFrame);
  await el.document.whenEditReady();
  window.ready = true;
} catch (error) { window.errors.push(String(error)); document.querySelector('#log').textContent = String(error); }
