// SPDX-License-Identifier: MIT
/** `<nitro-xlsx>` — thin shell host (#19/#29). */
import { defineTag, ShellViewerElement } from "./element.js";

const X = /\.(xlsx|xlsm|xltx|xltm)$/i;
const MACRO_EXT = /\.(xlsm|xltm)$/i;
/** Upstream `formatNotes` for xlsm (`pages-info.ts`). */
const MACRO_TOAST = "Macros are kept in the file but do not run in this editor.";
/** Compact toast CSS matching upstream `popups.css` + lightTheme tokens. */
const TOAST_CSS =
  ":host{position:relative}.xve-toasts{position:absolute;z-index:90;right:16px;bottom:40px;display:flex;max-width:min(420px,calc(100% - 32px));flex-direction:column;gap:8px;pointer-events:none;font:13px/1.35 'Segoe UI',Arial,sans-serif}.xve-toast{display:flex;align-items:flex-start;gap:8px;padding:10px 12px;border:1px solid #e1dfdd;border-left:4px solid #217346;border-radius:6px;background:#fff;box-shadow:0 8px 24px #0003;color:#1f1f1f;pointer-events:auto}.xve-toast-text{flex:1}.xve-toasts .xve-icon-button{display:inline-flex;align-items:center;justify-content:center;min-width:28px;height:28px;padding:0 6px;border:0;border-radius:4px;background:none;color:inherit;font:inherit;cursor:pointer}";

export function defineNitroXlsx() {
  defineTag("nitro-xlsx", NitroXlsxElement);
}

export class NitroXlsxElement extends ShellViewerElement {
  _ok(n) {
    return !!n && X.test(n);
  }
  _surf() {
    super._surf();
    if (!this._s || this._s.querySelector("style[data-toast]")) return;
    const st = document.createElement("style");
    st.dataset.toast = "1";
    st.textContent = TOAST_CSS;
    this._s.appendChild(st);
  }
  async _ao() {
    await super._ao();
    this._syncMacroToast();
  }
  _syncMacroToast() {
    const root = this._s;
    if (!root) return;
    const prev = root.querySelector(".xve-toasts");
    if (prev) prev.remove();
    const name = (this._d && this._d._name) || this._fileName() || "";
    if (!MACRO_EXT.test(name)) return;
    const stack = document.createElement("div");
    stack.className = "xve-toasts";
    stack.setAttribute("aria-live", "polite");
    const toast = document.createElement("div");
    toast.className = "xve-toast";
    toast.dataset.kind = "info";
    toast.setAttribute("role", "status");
    const text = document.createElement("span");
    text.className = "xve-toast-text";
    text.textContent = MACRO_TOAST;
    const close = document.createElement("button");
    close.type = "button";
    close.className = "xve-icon-button";
    close.textContent = "x";
    close.setAttribute("aria-label", "Close");
    close.addEventListener("click", () => toast.remove());
    toast.append(text, close);
    stack.append(toast);
    root.append(stack);
    // Match upstream `createToaster` info timeout (visual capture is within this window).
    setTimeout(() => toast.remove(), 6000);
  }
}
