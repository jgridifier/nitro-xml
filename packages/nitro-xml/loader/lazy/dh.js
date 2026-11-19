// SPDX-License-Identifier: MIT
import { trackDirty } from "./ribbon/dirty.js";

export function attach(el) {
  const proto = Object.getPrototypeOf(el);
  if (!Object.getOwnPropertyDescriptor(proto, "dirty")) {
    Object.defineProperty(proto, "dirty", {
      get() { return !!this._dy; },
      configurable: true,
    });
  }
  if (el._dT) {
    el._dT.rebase();
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
    el.dispatchEvent(new CustomEvent("nitro-change", {
      detail: { dirty: d }, bubbles: true, composed: true,
    }));
  });
  void el._dT.rebase();
}
