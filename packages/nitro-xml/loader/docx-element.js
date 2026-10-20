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
  _css() {
    return super._css() + ".dom{overflow:auto}";
  }
}
