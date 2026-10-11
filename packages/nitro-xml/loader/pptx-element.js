// SPDX-License-Identifier: MIT
/** `<nitro-pptx>` — thin shell host; slide nav via Rust `data-hk` (#27/#29). */
import { defineTag, ShellViewerElement } from "./element.js";

const P = /\.(pptx|pptm|potx|potm)$/i;

export function defineNitroPptx() {
  defineTag("nitro-pptx", NitroPptxElement);
}

export class NitroPptxElement extends ShellViewerElement {
  _ok(n) {
    return !!n && P.test(n);
  }
}
