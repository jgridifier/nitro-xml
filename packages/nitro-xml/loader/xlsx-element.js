// SPDX-License-Identifier: MIT
import { defineTag, ShellViewerElement } from "./element.js";
const X = /\.(xlsx|xlsm|xltx|xltm)$/i;
export function defineNitroXlsx() {
  defineTag("nitro-xlsx", NitroXlsxElement);
}
export class NitroXlsxElement extends ShellViewerElement {
  _ok(n) {
    return !!n && X.test(n);
  }
  _css() {
    // Reserve formula-bar height while .chrome is empty so the first mount does
    // not shift the grid under the pointer (audit theme 8 / S2-9, ~29px).
    return super._css() + ".chrome:empty{min-height:29px;box-sizing:border-box}";
  }
}
