// SPDX-License-Identifier: MIT
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
