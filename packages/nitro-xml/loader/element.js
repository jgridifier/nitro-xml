// SPDX-License-Identifier: MIT
/** `<nitro-xml>` custom element: wasm bootstrap, format gate, worker frame loop. */

import {
  ACCEPT,
  UNSUPPORTED_BROWSER_MESSAGE,
  UNSUPPORTED_FORMAT_MESSAGE,
  isSupportedExtension,
} from "./formats.js";
import { prewarmWorker, wasmSupported, unsupportedBrowserError } from "./bootstrap.js";
import { WorkerDocument } from "./worker-host.js";
import { EventKind } from "./events.js";

const TAG = "nitro-xml";

export function defineNitroXml() {
  if (typeof customElements === "undefined") return;
  if (customElements.get(TAG)) return;
  prewarmWorker();
  customElements.define(TAG, NitroXmlElement);
}

export class NitroXmlElement extends HTMLElement {
  constructor() {
    super();
    this._doc = null;
    this._ro = null;
    this._raf = 0;
    this._msg = null;
    this._canvas = null;
    this._root = null;
  }

  static get observedAttributes() {
    return ["src", "accept"];
  }

  connectedCallback() {
    if (!wasmSupported()) {
      this._showMessage(UNSUPPORTED_BROWSER_MESSAGE);
      return;
    }
    this._ensureSurface();
    this._bind();
    const src = this.getAttribute("src");
    if (src) this.loadUrl(src).catch((e) => this._fail(e));
  }

  disconnectedCallback() {
    this._teardown();
  }

  get accept() {
    return this.getAttribute("accept") || ACCEPT;
  }

  async loadFile(file, name) {
    const n = name || file.name || "";
    if (n && !isSupportedExtension(n)) {
      this._showMessage(UNSUPPORTED_FORMAT_MESSAGE);
      return;
    }
    const buf = await file.arrayBuffer();
    await this.loadBuffer(buf, n);
  }

  async loadUrl(url) {
    if (!wasmSupported()) throw unsupportedBrowserError();
    const name = url.split("?")[0].split("#")[0].split("/").pop() || "";
    if (name && !isSupportedExtension(name)) {
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
    if (name && !isSupportedExtension(name)) {
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
    this._schedule();
  }

  _ensureSurface() {
    if (this._canvas) return;
    const shadow = this.shadowRoot || this.attachShadow({ mode: "open" });
    shadow.innerHTML =
      `<style>:host{display:block;position:relative;min-height:4rem}` +
      `canvas, .dom{position:absolute;inset:0;width:100%;height:100%}` +
      `.msg{padding:1rem;font:14px/1.4 system-ui,sans-serif}</style>` +
      `<canvas part="canvas"></canvas><div class="dom" part="dom"></div>` +
      `<div class="msg" hidden part="message"></div>`;
    this._canvas = shadow.querySelector("canvas");
    this._root = shadow.querySelector(".dom");
    this._msg = shadow.querySelector(".msg");
  }

  _bind() {
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
      { passive: true },
    );
    if (typeof ResizeObserver !== "undefined") {
      this._ro = new ResizeObserver(() => {
        if (!this._doc || !this._canvas) return;
        const dpr = window.devicePixelRatio || 1;
        const w = this.clientWidth;
        const h = this.clientHeight;
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
      this._ro.observe(this);
    }
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
    if (this._canvas) this._canvas.style.display = "none";
    if (this._root) this._root.style.display = "none";
  }

  _clearMessage() {
    if (this._msg) {
      this._msg.hidden = true;
      this._msg.textContent = "";
    }
    if (this._canvas) this._canvas.style.display = "";
    if (this._root) this._root.style.display = "";
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
