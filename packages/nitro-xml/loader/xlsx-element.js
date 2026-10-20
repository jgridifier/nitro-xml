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
}
