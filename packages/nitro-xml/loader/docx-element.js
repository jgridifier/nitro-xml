// SPDX-License-Identifier: MIT
import { defineTag, ShellViewerElement } from "./element.js";
const P = /\.(docx|docm|dotx|dotm)$/i;
export function defineNitroDocx() {
  defineTag("nitro-docx", NitroDocxElement);
}
export class NitroDocxElement extends ShellViewerElement {
  _ok(n) {
    return !!n && P.test(n);
  }
  async _ao() {
    // Start the caret module before shellAction("ui"). The page paints during that
    // round trip; a click in the gap used to find no listener (WebKit's first load).
    if (!this._readonly) import("./lazy/docx-caret.js").then((m) => m.attach(this)).catch(() => {});
    await super._ao();
  }
  _css() {
    return super._css() + ".dom{overflow:auto}";
  }
}
