// SPDX-License-Identifier: MIT
/** `<nitro-xlsx>` viewer with Stage-6 shell chrome (#19 + #29). */

import {
  isSupportedExtension,
  UNSUPPORTED_BROWSER_MESSAGE,
  UNSUPPORTED_FORMAT_MESSAGE,
} from "./formats.js";
import { prewarmWorker, wasmSupported, unsupportedBrowserError } from "./bootstrap.js";
import { WorkerDocument } from "./worker-host.js";
import { EventKind } from "./events.js";
import { mountShellChrome, bindShellUi, followHyperlink } from "./shell-chrome.js";

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
    this._chrome = null;
    this._readonly = true;
  }

  static get observedAttributes() {
    return ["src", "readonly"];
  }

  attributeChangedCallback(name, _old, value) {
    if (name === "readonly") this._readonly = value !== "false";
  }

  connectedCallback() {
    if (!wasmSupported()) {
      this._ensureChrome();
      this._showMessage(UNSUPPORTED_BROWSER_MESSAGE);
      return;
    }
    this._readonly = this.getAttribute("readonly") !== "false";
    this._ensureChrome();
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

  get document() {
    return this._doc;
  }

  async loadFile(file, name) {
    const n = name || file.name || "";
    if (!isXlsxName(n)) {
      this._showMessage(UNSUPPORTED_FORMAT_MESSAGE);
      return;
    }
    await this.loadBuffer(await file.arrayBuffer(), n);
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
    this._ensureChrome();
    if (this._doc) this._doc.dispose();
    this._doc = new WorkerDocument({
      canvas: this._chrome.canvas,
      root: this._chrome.root,
      name,
    });
    await this._doc.open(buffer, name);
    await this._refreshShell();
    this._schedule();
  }

  /** Host/test hook: run a shell action through the worker. */
  async shellAction(op, arg = "") {
    if (!this._doc) return null;
    const result = await this._doc.shellAction(op, arg);
    this._applyShellResult(result);
    this._schedule();
    return result;
  }

  _ensureChrome() {
    if (this._chrome) return;
    const shadow = this.shadowRoot || this.attachShadow({ mode: "open" });
    this._chrome = mountShellChrome(shadow);
  }

  _bind() {
    const c = this._chrome;
    if (!c) return;
    c.bar?.addEventListener("click", (e) => this._onChromeClick(e));
    c.tabs?.addEventListener("click", (e) => this._onChromeClick(e));
    c.find?.addEventListener("click", (e) => this._onChromeClick(e));
    c.findInput?.addEventListener("input", () => {
      this.shellAction("find-query", c.findInput.value || "").catch(() => {});
    });
    c.findInput?.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        this.shellAction(e.shiftKey ? "find-prev" : "find-next").catch(() => {});
      } else if (e.key === "Escape") {
        e.preventDefault();
        this.shellAction("find-close").catch(() => {});
      }
    });
    this.addEventListener("keydown", (e) => this._onKey(e));
    const onPtr = (type) => (ev) => {
      if (!this._doc || !c.canvas) return;
      const r = c.canvas.getBoundingClientRect();
      this._doc.pushEvent({
        kind: EventKind.Pointer,
        time_ms: performance.now(),
        x: ev.clientX - r.left,
        y: ev.clientY - r.top,
        type,
        button: ev.button,
        buttons: ev.buttons,
      });
      this._schedule();
    };
    c.viewport?.addEventListener("pointerdown", onPtr(0));
    c.viewport?.addEventListener("pointermove", onPtr(1));
    c.viewport?.addEventListener("pointerup", onPtr(2));
    c.viewport?.addEventListener(
      "wheel",
      (e) => {
        if (!this._doc) return;
        e.preventDefault();
        const r = c.canvas.getBoundingClientRect();
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
        if (!this._doc || !c.canvas) return;
        const dpr = window.devicePixelRatio || 1;
        const w = c.viewport.clientWidth;
        const h = c.viewport.clientHeight;
        c.canvas.width = Math.max(1, (w * dpr) | 0);
        c.canvas.height = Math.max(1, (h * dpr) | 0);
        this._doc.pushEvent({
          kind: EventKind.Resize,
          time_ms: performance.now(),
          width: w,
          height: h,
          dpr,
        });
        this._schedule();
      });
      this._ro.observe(c.viewport);
    }
  }

  _onChromeClick(e) {
    const btn = e.target?.closest?.("[data-act]");
    if (!btn) return;
    const op = btn.getAttribute("data-act");
    const arg = btn.getAttribute("data-arg") || "";
    this.shellAction(op, arg).catch(() => {});
  }

  _onKey(e) {
    if (!this._doc) return;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && (e.key === "f" || e.key === "F")) {
      e.preventDefault();
      this.shellAction("find-open").then(() => this._chrome?.findInput?.focus());
    } else if (mod && (e.key === "c" || e.key === "C") && !e.shiftKey) {
      // Let copy through shell when selection managed; still allow native if input focused.
      if (e.target === this._chrome?.findInput) return;
      e.preventDefault();
      this.shellAction("copy").catch(() => {});
    } else if (mod && (e.key === "=" || e.key === "+")) {
      e.preventDefault();
      this.shellAction("zoom-in").catch(() => {});
    } else if (mod && e.key === "-") {
      e.preventDefault();
      this.shellAction("zoom-out").catch(() => {});
    } else if (e.key === "Escape") {
      this.shellAction("find-close").catch(() => {});
    }
  }

  async _refreshShell() {
    if (!this._doc) return;
    const ui = await this._doc.shellUi();
    bindShellUi(this._chrome, ui);
  }

  _applyShellResult(result) {
    if (!result) return;
    if (result.ui) bindShellUi(this._chrome, result.ui);
    if (result.copyText != null && result.copyText !== undefined) {
      const text = result.copyText || "";
      if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
        navigator.clipboard.writeText(text).catch(() => {});
      }
      this.dispatchEvent(
        new CustomEvent("nitro-copy", { detail: { text }, bubbles: true }),
      );
    }
    if (result.pendingHyperlink) {
      const followed = followHyperlink(result.pendingHyperlink);
      this.dispatchEvent(
        new CustomEvent("nitro-hyperlink", {
          detail: { ...result.pendingHyperlink, followed },
          bubbles: true,
        }),
      );
    }
    this.dispatchEvent(
      new CustomEvent("nitro-shell", { detail: result.ui || null, bubbles: true }),
    );
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
    this._ensureChrome();
    if (this._chrome.msg) {
      this._chrome.msg.hidden = false;
      this._chrome.msg.textContent = text;
    }
    if (this._chrome.viewport) this._chrome.viewport.style.display = "none";
  }

  _clearMessage() {
    if (this._chrome?.msg) {
      this._chrome.msg.hidden = true;
      this._chrome.msg.textContent = "";
    }
    if (this._chrome?.viewport) this._chrome.viewport.style.display = "";
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
