// SPDX-License-Identifier: MIT
/** `<nitro-docx>` — thin shell host; page nav via Rust `data-hk` (#23/#29). */
import { defineTag, ShellViewerElement } from "./element.js";

const P = /\.(docx|docm|dotx|dotm)$/i;

export function defineNitroDocx() {
  defineTag("nitro-docx", NitroDocxElement);
}

export class NitroDocxElement extends ShellViewerElement {
  constructor() {
    super();
    this._pg = null;
  }
  _ok(n) {
    return !!n && P.test(n);
  }
  _css() {
    return super._css() + ".dom{overflow:auto}";
  }
  _tick() {
    if (this._raf) return;
    this._raf = requestAnimationFrame(async () => {
      this._raf = 0;
      try {
        const d = this._d;
        if (!d) return;
        if (d._root) d._root.replaceChildren();
        if (d._nodes) d._nodes.clear();
        await d.tick();
        const cur = d._root && d._root.querySelector("[data-nitro-current]");
        const idx = cur && cur.getAttribute("data-page-index");
        if (cur && idx != null && idx !== this._pg) {
          this._pg = idx;
          cur.scrollIntoView({ block: "nearest" });
        }
      } catch (e) {
        this._fail(e);
      }
    });
  }
}
