// SPDX-License-Identifier: MIT
/** Lazy EMF/WMF → PNG for `nitro-metafile:` img src. */
const RE = /^nitro-metafile:(?:emf|wmf);base64,(.+)$/i;
let cvt;
const pend = new WeakSet();

async function loadConverter() {
  if (cvt) return cvt;
  // Bundlers / Node resolve the bare specifier; the visual static host cannot,
  // so fall back to the package-local browser build next to the loader.
  try {
    const x = await import("emf-converter");
    cvt = x.convertMetafileToDataUrl;
  } catch {
    const x = await import("../node_modules/emf-converter/dist/browser.mjs");
    cvt = x.convertMetafileToDataUrl;
  }
  return cvt;
}

export function scheduleMetafileAttr(el, v) {
  const m = v && RE.exec(v);
  if (!m || pend.has(el)) return;
  pend.add(el);
  const b = atob(m[1]),
    u = new Uint8Array(b.length);
  for (let i = 0; i < b.length; i++) u[i] = b.charCodeAt(i);
  void loadConverter()
    .then((fn) => fn(u.buffer))
    .then((url) => {
      if (url && el.isConnected) el.src = url;
    })
    .catch(() => {})
    .finally(() => pend.delete(el));
}
