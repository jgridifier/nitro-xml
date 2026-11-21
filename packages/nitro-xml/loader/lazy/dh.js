// SPDX-License-Identifier: MIT
import { trackDirty } from "./ribbon/dirty.js";

export async function attach(el) {
  const proto = Object.getPrototypeOf(el);
  if (!Object.getOwnPropertyDescriptor(proto, "dirty")) {
    Object.defineProperty(proto, "dirty", {
      get() { return !!this._dy; },
      configurable: true,
    });
  }
  if (el._dT) {
    if (el._dirtyDoc !== el.document) {
      el._dirtyDoc = el.document;
      await el._dT.rebase(true);
      el._dy = false;
    }
    return;
  }
  const origSave = proto.save;
  el.save = async function() {
    const bytes = await origSave.call(this);
    this._dT?.saved(bytes);
    return bytes;
  };
  el._dT = trackDirty(el, (d) => {
    el._dy = d;
    const ev = { detail: { dirty: d }, bubbles: true, composed: true };
    el.dispatchEvent(new CustomEvent("nitro-dirty", ev));
    el.dispatchEvent(new CustomEvent("nitro-change", ev));
  });
  el._dirtyDoc = el.document;
  await el._dT.rebase(true);
  el._dy = false;
}
