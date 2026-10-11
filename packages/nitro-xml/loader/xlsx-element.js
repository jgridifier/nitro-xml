// SPDX-License-Identifier: MIT
/** `<nitro-xlsx>`: apply Rust `chromeHtml`; forward events (#29). */
import { defineTag, ViewerElement } from "./element.js";

const X = /\.(xlsx|xlsm|xltx|xltm)$/i;

export function defineNitroXlsx() {
  defineTag("nitro-xlsx", NitroXlsxElement);
}

export class NitroXlsxElement extends ViewerElement {
  constructor() {
    super();
    this._ch = null;
    this._readonly = !0;
    this._wp = 1;
  }
  static get observedAttributes() {
    return ["src", "readonly"];
  }
  attributeChangedCallback(n, _, v) {
    if (n == "readonly") this._readonly = v != "false";
  }
  connectedCallback() {
    this._readonly = this.getAttribute("readonly") != "false";
    super.connectedCallback();
  }
  get readonly() {
    return this._readonly;
  }
  _ok(n) {
    return !!n && X.test(n);
  }
  async _ao() {
    await this.shellAction("ui");
  }
  _css() {
    return ":host{display:flex;flex-direction:column;min-height:12rem;background:#fff}.v{position:relative;flex:1;min-height:0}canvas,.dom{position:absolute;inset:0;width:100%;height:100%}.msg{padding:1rem;font:14px/1.4 system-ui,sans-serif}";
  }
  _body() {
    return '<div class="chrome"></div><div class="v"><canvas part="canvas"></canvas><div class="dom" part="dom"></div></div><div class="msg" hidden part="message"></div>';
  }
  _surf() {
    super._surf();
    this._ch = this._s.querySelector(".chrome");
  }
  _bx() {
    const g = (o, a) => this.shellAction(o, a || "");
    const ch = this._ch;
    ch.onclick = (e) => {
      const b = e.target.closest && e.target.closest("[data-a]");
      b && g(b.dataset.a, b.dataset.g);
    };
    ch.oninput = (e) => {
      e.target.classList.contains("q") && g("find-query", e.target.value);
    };
    this.onkeydown = (e) => {
      if (!this._d) return;
      const q = e.target.classList.contains("q");
      const m = e.ctrlKey || e.metaKey;
      let k = (m ? "m" : "") + e.key;
      if (q && e.key == "Enter") k = e.shiftKey ? "qSEnter" : "qEnter";
      else if (q && e.key == "Escape") k = "qEscape";
      for (const p of ((ch.firstElementChild && ch.firstElementChild.dataset.hk) || "").split("|")) {
        const i = p.indexOf(":");
        if (p.slice(0, i) === k) {
          e.preventDefault();
          return g(p.slice(i + 1));
        }
      }
    };
  }
  /** Apply Rust chrome + side effects. Short keys: `h` html, `c` copy, `p` hyperlink. */
  async shellAction(op, arg = "") {
    if (!this._d) return null;
    const r = await this._d.shellAction(op, arg);
    if (this._ch && r.h != null) this._ch.innerHTML = r.h;
    if (r.c != null) {
      navigator.clipboard && navigator.clipboard.writeText && navigator.clipboard.writeText(r.c);
      this.dispatchEvent(new CustomEvent("nitro-copy", { detail: r.c, bubbles: !0 }));
    }
    if (r.p) {
      r.p.open && r.p.target && window.open(r.p.target, "_blank", "noopener,noreferrer");
      this.dispatchEvent(new CustomEvent("nitro-hyperlink", { detail: r.p, bubbles: !0 }));
    }
    this._tick();
    return r;
  }
}
