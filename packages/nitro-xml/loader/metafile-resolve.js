// SPDX-License-Identifier: MIT
/** Lazy EMF/WMF → PNG for `nitro-metafile:` img src. */
const RE = /^nitro-metafile:(?:emf|wmf);base64,(.+)$/i;
let cvt;
const pend = new WeakSet();
export function scheduleMetafileAttr(el, v) {
  const m = v && RE.exec(v);
  if (!m || pend.has(el)) return;
  pend.add(el);
  const b = atob(m[1]),
    u = new Uint8Array(b.length);
  for (let i = 0; i < b.length; i++) u[i] = b.charCodeAt(i);
  void Promise.resolve(cvt || import("emf-converter").then((x) => (cvt = x.convertMetafileToDataUrl)))
    .then((fn) => fn(u.buffer))
    .then((url) => {
      if (url && el.isConnected) el.setAttribute("src", url);
    })
    .catch(() => {})
    .finally(() => pend.delete(el));
}
