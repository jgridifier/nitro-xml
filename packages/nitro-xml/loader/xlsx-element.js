// SPDX-License-Identifier: MIT
/** `<nitro-xlsx>` viewer + shell (#19/#29). */

import {
  isSupportedExtension,
  UNSUPPORTED_BROWSER_MESSAGE,
  UNSUPPORTED_FORMAT_MESSAGE,
} from "./formats.js";
import { prewarmWorker, wasmSupported, unsupportedBrowserError } from "./bootstrap.js";
import { WorkerDocument } from "./worker-host.js";
import { EventKind } from "./events.js";

const TAG = "nitro-xlsx";
const XLSX_EXT = /\.(xlsx|xlsm|xltx|xltm)$/i;

function isXlsxName(name) {
  return name && XLSX_EXT.test(name);
}

export function defineNitroXlsx() {
  if (typeof customElements === "undefined") return;
  if (customElements.get(TAG)) return;
  prewarmWorker();
  customElements.define(TAG, NitroXlsxElement);
}

export class NitroXlsxElement extends HTMLElement {
  constructor() {
    super();
    this._doc = null;
    this._ro = null;
    this._raf = 0;
    this._msg = null;
    this._canvas = null;
    this._root = null;
    this._readonly = true;
  }

  static get observedAttributes() {
    return ["src", "readonly"];
  }

  attributeChangedCallback(name, _old, value) {
    if (name === "readonly") {
      this._readonly = value !== "false";
    }
  }

  connectedCallback() {
    if (!wasmSupported()) {
      this._showMessage(UNSUPPORTED_BROWSER_MESSAGE);
      return;
    }
    this._readonly = this.getAttribute("readonly") !== "false";
    this._ensureSurface();
    this._bind();
    const src = this.getAttribute("src");
    if (src) this.loadUrl(src).catch((e) => this._fail(e));
  }

  disconnectedCallback() {
    this._teardown();
  }

  get readonly() {
    return this._readonly;
  }

  /** @returns {import("./worker-host.js").WorkerDocument | null} */
  get document() {
    return this._doc;
  }

  async loadFile(file, name) {
    const n = name || file.name || "";
    if (!isXlsxName(n)) {
      this._showMessage(UNSUPPORTED_FORMAT_MESSAGE);
      return;
    }
    const buf = await file.arrayBuffer();
    await this.loadBuffer(buf, n);
  }

  async loadUrl(url) {
    if (!wasmSupported()) throw unsupportedBrowserError();
    const name = url.split("?")[0].split("#")[0].split("/").pop() || "";
    if (!isXlsxName(name)) {
      this._showMessage(UNSUPPORTED_FORMAT_MESSAGE);
      return;
    }
    const res = await fetch(url);
    if (!res.ok) throw new Error(`fetch failed: ${res.status}`);
    await this.loadBuffer(await res.arrayBuffer(), name);
  }

  async loadBuffer(buffer, name = "") {
    if (!wasmSupported()) {
      this._showMessage(UNSUPPORTED_BROWSER_MESSAGE);
      throw unsupportedBrowserError();
    }
    if (!isXlsxName(name) && name && !isSupportedExtension(name)) {
      this._showMessage(UNSUPPORTED_FORMAT_MESSAGE);
      throw Object.assign(new Error(UNSUPPORTED_FORMAT_MESSAGE), {
        code: "unsupported_format",
      });
    }
    this._clearMessage();
    this._ensureSurface();
    if (this._doc) this._doc.dispose();
    this._doc = new WorkerDocument({
      canvas: this._canvas,
      root: this._root,
      name,
    });
    await this._doc.open(buffer, name);
    await this.shellAction("ui");
    this._schedule();
  }

  _ensureSurface() {
    if (this._canvas) return;
    const shadow = this.shadowRoot || this.attachShadow({ mode: "open" });
    shadow.innerHTML =
      `<style>:host{display:flex;flex-direction:column;min-height:12rem;background:#fff;font:12px system-ui,sans-serif}` +
      `.b,.f,.t{display:flex;gap:.25rem;flex-wrap:wrap;padding:.2rem;border-bottom:1px solid #ddd;align-items:center}` +
      `.v{position:relative;flex:1;min-height:0}canvas,.dom{position:absolute;inset:0;width:100%;height:100%}` +
      `[hidden]{display:none!important}.msg{padding:1rem;font:14px/1.4 system-ui,sans-serif}</style>` +
      `<div class="b"><button data-a="zoom-out">−</button><span class="z">100%</span><button data-a="zoom-in">+</button>` +
      `<button data-a="find-open">Find</button><button data-a="copy">Copy</button><button data-a="diagnostics-toggle">Diag</button></div>` +
      `<div class="f" hidden><input class="q"/><button data-a="find-prev">↑</button><button data-a="find-next">↓</button><span class="fs"></span><button data-a="find-close">×</button></div>` +
      `<div class="t"></div><div class="v"><canvas part="canvas"></canvas><div class="dom" part="dom"></div></div>` +
      `<div class="msg" hidden part="message"></div>`;
    this._canvas = shadow.querySelector("canvas");
    this._root = shadow.querySelector(".dom");
    this._msg = shadow.querySelector(".msg");
    this._s = shadow;
  }

  _bind() {
    this._ensureSurface();
    this._s.addEventListener("click", (e) => {
      const b = e.target.closest?.("[data-a]");
      if (b) this.shellAction(b.getAttribute("data-a"), b.getAttribute("data-g") || "");
    });
    this._s.querySelector(".q").addEventListener("input", (e) => this.shellAction("find-query", e.target.value || ""));
    this.addEventListener("keydown", (e) => {
      if (!this._doc) return;
      const m = e.ctrlKey || e.metaKey, k = e.key;
      if (m && k === "f") { e.preventDefault(); this.shellAction("find-open"); }
      else if (m && k === "c" && e.target.className !== "q") { e.preventDefault(); this.shellAction("copy"); }
      else if (m && (k === "=" || k === "+")) { e.preventDefault(); this.shellAction("zoom-in"); }
      else if (m && k === "-") { e.preventDefault(); this.shellAction("zoom-out"); }
      else if (k === "Escape") this.shellAction("find-close");
    });
    const onPtr = (type) => (e) => {
      if (!this._doc) return;
      const r = this._canvas.getBoundingClientRect();
      this._doc.pushEvent({
        kind: EventKind.Pointer,
        time_ms: performance.now(),
        x: e.clientX - r.left,
        y: e.clientY - r.top,
        type,
        button: e.button,
        buttons: e.buttons,
      });
      this._schedule();
    };
    this.addEventListener("pointerdown", onPtr(0));
    this.addEventListener("pointermove", onPtr(1));
    this.addEventListener("pointerup", onPtr(2));
    this.addEventListener(
      "wheel",
      (e) => {
        if (!this._doc) return;
        e.preventDefault();
        const r = this._canvas.getBoundingClientRect();
        this._doc.pushEvent({
          kind: EventKind.Wheel,
          time_ms: performance.now(),
          x: e.clientX - r.left,
          y: e.clientY - r.top,
          dx: e.deltaX,
          dy: e.deltaY,
        });
        this._schedule();
      },
      { passive: false },
    );
    if (typeof ResizeObserver !== "undefined") {
      this._ro = new ResizeObserver(() => {
        if (!this._doc || !this._canvas) return;
        const dpr = window.devicePixelRatio || 1;
        const v = this._s.querySelector(".v");
        const w = v.clientWidth;
        const h = v.clientHeight;
        this._canvas.width = Math.max(1, (w * dpr) | 0);
        this._canvas.height = Math.max(1, (h * dpr) | 0);
        this._doc.pushEvent({
          kind: EventKind.Resize,
          time_ms: performance.now(),
          width: w,
          height: h,
          dpr,
        });
        this._schedule();
      });
      this._ro.observe(this._s.querySelector(".v"));
    }
  }


  async shellAction(op, arg = "") {
    if (!this._doc) return null;
    const r = await this._doc.shellAction(op, arg), s = this._s, ui = r?.ui;
    if (s && ui) {
      s.querySelector(".z").textContent = Math.round(ui.zoom || 100) + "%";
      s.querySelector(".f").hidden = !ui.find?.open;
      const q = s.querySelector(".q");
      if (document.activeElement !== q) q.value = ui.find?.query || "";
      s.querySelector(".fs").textContent = ui.find?.status || "";
      s.querySelector(".t").innerHTML = (ui.tabs || [])
        .map((t) => `<button data-a="sheet" data-g="${t.index}"${t.active ? " aria-current=true" : ""}>${t.name}</button>`)
        .join("");
    }
    if (r?.copyText != null) {
      navigator.clipboard?.writeText?.(r.copyText || "");
      this.dispatchEvent(new CustomEvent("nitro-copy", { detail: r.copyText || "", bubbles: true }));
    }
    if (r?.pendingHyperlink) {
      const l = r.pendingHyperlink;
      if (l.open && l.target) try { window.open(l.target, "_blank", "noopener,noreferrer"); } catch {}
      this.dispatchEvent(new CustomEvent("nitro-hyperlink", { detail: l, bubbles: true }));
    }
    this._schedule();
    return r;
  }

  _schedule() {
    if (this._raf) return;
    this._raf = requestAnimationFrame(async () => {
      this._raf = 0;
      if (!this._doc) return;
      try {
        await this._doc.tick();
      } catch (e) {
        this._fail(e);
      }
    });
  }

  _showMessage(text) {
    this._ensureSurface();
    if (this._msg) {
      this._msg.hidden = false;
      this._msg.textContent = text;
    }
    const v = this._s?.querySelector(".v"); if (v) v.style.display = "none";
  }

  _clearMessage() {
    if (this._msg) {
      this._msg.hidden = true;
      this._msg.textContent = "";
    }
    const v = this._s?.querySelector(".v"); if (v) v.style.display = "";
  }

  _fail(e) {
    if (e && e.code === "unsupported_browser") {
      this._showMessage(UNSUPPORTED_BROWSER_MESSAGE);
      return;
    }
    if (e && e.code === "unsupported_format") {
      this._showMessage(UNSUPPORTED_FORMAT_MESSAGE);
      return;
    }
    this._showMessage((e && e.message) || String(e));
  }

  _teardown() {
    if (this._raf) cancelAnimationFrame(this._raf);
    this._raf = 0;
    if (this._ro) this._ro.disconnect();
    if (this._doc) this._doc.dispose();
    this._doc = null;
  }
}
