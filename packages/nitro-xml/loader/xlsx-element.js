// SPDX-License-Identifier: MIT
/** `<nitro-xlsx>` — thin shell host (#19/#29). */
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
