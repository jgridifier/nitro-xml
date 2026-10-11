// SPDX-License-Identifier: MIT
/** Shared viewer host + `<nitro-xml>` (#19/#29). Docx/pptx extend this. */
import {
  UNSUPPORTED_BROWSER_MESSAGE as UB,
  UNSUPPORTED_FORMAT_MESSAGE as UF,
  isSupportedExtension as ok,
  ACCEPT,
} from "./formats.js";
import { prewarmWorker as pw, wasmSupported as ws, unsupportedBrowserError as ube } from "./bootstrap.js";
import { WorkerDocument as WD } from "./worker-host.js";
import { EventKind as EK } from "./events.js";

export function defineTag(t, C) {
  if (typeof customElements === "undefined" || customElements.get(t)) return;
  pw();
  customElements.define(t, C);
}

export class ViewerElement extends HTMLElement {
  constructor() {
    super();
    this._d = this._ro = this._raf = this._m = this._c = this._r = this._v = null;
  }
  connectedCallback() {
    if (!ws()) return this._show(UB);
    this._surf();
    this._bind();
    const s = this.getAttribute("src");
    if (s) this.loadUrl(s).catch((e) => this._fail(e));
  }
  disconnectedCallback() {
    if (this._raf) cancelAnimationFrame(this._raf);
    this._ro && this._ro.disconnect();
    this._d && this._d.dispose();
    this._d = null;
  }
  get document() {
    return this._d;
  }
  async loadFile(f, n) {
    n = n || f.name || "";
    if (!this._ok(n)) return this._show(UF);
    await this.loadBuffer(await f.arrayBuffer(), n);
  }
  async loadUrl(u) {
    if (!ws()) throw ube();
    const n = u.split("?")[0].split("#")[0].split("/").pop() || "";
    if (!this._ok(n)) return this._show(UF);
    const r = await fetch(u);
    if (!r.ok) throw new Error("fetch failed: " + r.status);
    await this.loadBuffer(await r.arrayBuffer(), n);
  }
  async loadBuffer(b, n = "") {
    if (!ws()) {
      this._show(UB);
      throw ube();
    }
    if (n && !this._ok(n) && !ok(n)) {
      this._show(UF);
      throw Object.assign(new Error(UF), { code: "unsupported_format" });
    }
    this._clr();
    this._surf();
    this._d && this._d.dispose();
    this._d = new WD({ canvas: this._c, root: this._r, name: n });
    await this._d.open(b, n);
    await this._ao();
    this._tick();
  }
  _ok(n) {
    return !n || ok(n);
  }
  async _ao() {}
  _css() {
    return ":host{display:block;position:relative;min-height:4rem}canvas,.dom{position:absolute;inset:0;width:100%;height:100%}.msg{padding:1rem;font:14px/1.4 system-ui,sans-serif}";
  }
  _body() {
    return '<div class="v"><canvas part="canvas"></canvas><div class="dom" part="dom"></div></div><div class="msg" hidden part="message"></div>';
  }
  _surf() {
    if (this._c) return;
    const s = this.shadowRoot || this.attachShadow({ mode: "open" });
    s.innerHTML = "<style>" + this._css() + "[hidden]{display:none!important}</style>" + this._body();
    this._c = s.querySelector("canvas");
    this._r = s.querySelector(".dom");
    this._m = s.querySelector(".msg");
    this._v = s.querySelector(".v") || this;
    this._s = s;
  }
  _bx() {}
  _bind() {
    this._surf();
    this._bx();
    const p = (t) => (e) => {
      if (!this._d) return;
      const r = this._c.getBoundingClientRect();
      this._d.pushEvent({
        kind: EK.Pointer,
        time_ms: performance.now(),
        x: e.clientX - r.left,
        y: e.clientY - r.top,
        type: t,
        button: e.button,
        buttons: e.buttons,
      });
      this._tick();
    };
    const t = this._v;
    t.addEventListener("pointerdown", p(0));
    t.addEventListener("pointermove", p(1));
    t.addEventListener("pointerup", p(2));
    t.addEventListener(
      "wheel",
      (e) => {
        if (!this._d) return;
        this._wp && e.preventDefault();
        const r = this._c.getBoundingClientRect();
        this._d.pushEvent({
          kind: EK.Wheel,
          time_ms: performance.now(),
          x: e.clientX - r.left,
          y: e.clientY - r.top,
          dx: e.deltaX,
          dy: e.deltaY,
        });
        this._tick();
      },
      { passive: !this._wp },
    );
    if (typeof ResizeObserver !== "undefined") {
      this._ro = new ResizeObserver(() => {
        if (!this._d) return;
        const d = window.devicePixelRatio || 1;
        const el = this._v === this ? this : this._v;
        const w = el.clientWidth;
        const h = el.clientHeight;
        this._c.width = Math.max(1, (w * d) | 0);
        this._c.height = Math.max(1, (h * d) | 0);
        this._d.pushEvent({
          kind: EK.Resize,
          time_ms: performance.now(),
          width: w,
          height: h,
          dpr: d,
        });
        this._tick();
      });
      this._ro.observe(this._v);
    }
  }
  _tick() {
    if (this._raf) return;
    this._raf = requestAnimationFrame(async () => {
      this._raf = 0;
      try {
        if (this._d) await this._d.tick();
      } catch (e) {
        this._fail(e);
      }
    });
  }
  _show(t) {
    this._surf();
    this._m.hidden = !1;
    this._m.textContent = t;
    if (this._v !== this) this._v.style.display = "none";
    else {
      this._c.style.display = "none";
      this._r.style.display = "none";
    }
  }
  _clr() {
    if (!this._m) return;
    this._m.hidden = !0;
    this._m.textContent = "";
    if (this._v !== this) this._v.style.display = "";
    else {
      this._c.style.display = "";
      this._r.style.display = "";
    }
  }
  _fail(e) {
    this._show(
      e && e.code == "unsupported_browser"
        ? UB
        : e && e.code == "unsupported_format"
          ? UF
          : (e && e.message) || String(e),
    );
  }
}

export function defineNitroXml() {
  defineTag("nitro-xml", NitroXmlElement);
}
export class NitroXmlElement extends ViewerElement {
  static get observedAttributes() {
    return ["src", "accept"];
  }
  get accept() {
    return this.getAttribute("accept") || ACCEPT;
  }
}
