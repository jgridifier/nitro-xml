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
import { EventKind as EK, KeyId } from "./events.js";

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
    // Assign only after open so ResizeObserver/events cannot race handle=null (#27).
    const d = new WD({ canvas: this._c, root: this._r, name: n });
    await d.open(b, n);
    this._d = d;
    await this._ao();
    this._tick();
  }
  _ok(n) {
    return !n || ok(n);
  }
  async _ao() {}
  _css() {
    return ":host{display:block;position:relative;min-height:4rem}.v{outline:none}canvas,.dom{position:absolute;inset:0;width:100%;height:100%}.msg{padding:1rem;font:14px/1.4 system-ui,sans-serif}";
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
    if (this._v && this._v !== this) this._v.tabIndex = 0;
  }
  _bx() {}
  _bind() {
    this._surf();
    this._bx();
    const keys = (e) => ({
      shiftKey: !!e.shiftKey,
      ctrlKey: !!e.ctrlKey,
      altKey: !!e.altKey,
      metaKey: !!e.metaKey,
    });
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
        ...keys(e),
      });
      if (t === 0 && this._v && this._v !== this) {
        this._v.focus();
        try {
          this._v.setPointerCapture(e.pointerId);
        } catch {
          /* capture is optional */
        }
      }
      this._tick();
    };
    const t = this._v;
    t.addEventListener("pointerdown", p(0));
    t.addEventListener("pointermove", p(1));
    t.addEventListener("pointerup", p(2));
    const keyEv = (type) => (e) => {
      if (!this._d) return;
      const id = KeyId[e.key];
      if (!id) return;
      const arrow = e.key.indexOf("Arrow") === 0 || e.key === "PageUp" || e.key === "PageDown";
      // Shell chrome already binds pptx arrows/page keys to nav-next/prev.
      if (arrow && this.localName === "nitro-pptx") return;
      const selectAll = (e.ctrlKey || e.metaKey) && (e.key === "a" || e.key === "A");
      if (arrow || selectAll) {
        e.preventDefault();
        e.stopPropagation();
      }
      this._d.pushEvent({
        kind: EK.Keyboard,
        time_ms: performance.now(),
        key_id: id,
        code_id: 0,
        type,
        ...keys(e),
      });
      this._tick();
    };
    t.addEventListener("keydown", keyEv(0));
    t.addEventListener("keyup", keyEv(1));
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
      try {
        if (this._d) await this._d.tick();
      } catch (e) {
        this._fail(e);
      } finally {
        this._raf = 0;
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

/** Shared Stage-6 shell chrome binder for xlsx/pptx (#29). Apply `h` + forward only. */
export class ShellViewerElement extends ViewerElement {
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
  async _ao() {
    await this.shellAction("ui");
  }
  _css() {
    return ":host{display:flex;flex-direction:column;min-height:12rem;background:#fff}.v{position:relative;flex:1;min-height:0;outline:none;background:#fff}canvas,.dom{position:absolute;inset:0;width:100%;height:100%}canvas{background:#fff}.msg{padding:1rem;font:14px/1.4 system-ui,sans-serif}";
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
