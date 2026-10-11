// SPDX-License-Identifier: MIT
/**
 * Main-thread client for a dedicated document worker.
 * Transfers the file ArrayBuffer in; applies frame command buffers locally;
 * runs measureText once per frame and posts widths back as one batch.
 */

import { takePrewarmedWorker, prewarmWorker } from "./bootstrap.js";
import { applyCommandBuffer } from "./apply.js";
import { createEventCoalescer } from "./events.js";
import { isSupportedExtension } from "./formats.js";
import { unsupportedFormatError } from "./bootstrap.js";

export class WorkerDocument {
  /**
   * @param {{ canvas?: HTMLCanvasElement|null, root?: Element|null, name?: string, workerUrl?: string|URL }} [opts]
   */
  constructor(opts = {}) {
    this._canvas = opts.canvas || null;
    this._root = opts.root || null;
    this._name = opts.name || "";
    this._nodes = new Map();
    this._images = new Map();
    this._pending = new Map();
    this._req = 0;
    this._worker =
      takePrewarmedWorker() ||
      prewarmWorker(opts.workerUrl) ||
      new Worker(new URL("./worker.js", import.meta.url), {
        type: "module",
        name: "nitro-xml-doc",
      });
    this._worker.onmessage = (ev) => this._onMessage(ev.data);
    this._events = createEventCoalescer((bytes) => {
      this._worker.postMessage({ type: "events", bytes }, [bytes.buffer]);
    });
    this._measureCanvas = null;
  }

  /**
   * @param {ArrayBuffer} buffer transferred (caller must not use after)
   * @param {string} [name]
   */
  async open(buffer, name) {
    const n = name || this._name;
    if (n && !isSupportedExtension(n)) throw unsupportedFormatError();
    this._name = n;
    return this._request("opened", () => {
      this._worker.postMessage({ type: "open", bytes: buffer, name: n }, [buffer]);
    });
  }

  /** @param {import('./events.js').LoaderEvent} ev */
  pushEvent(ev) {
    this._events.push(ev);
  }

  /**
   * Pull one frame from the worker, measure in one batch, apply commands, reply widths.
   */
  async tick() {
    this._events.flushNow();
    const frame = await this._request("frame", () => {
      this._worker.postMessage({ type: "take-frame" });
    });
    const { commands, measureReqs, measureCount, strings } = frame;
    if (measureCount > 0 && measureReqs) {
      const widths = new Float64Array(measureCount);
      const ctx = this._measureCtx();
      if (ctx) {
        for (let i = 0; i < measureCount; i++) {
          const font = (strings && strings[measureReqs.fonts[i]]) || "sans-serif";
          const text = (strings && strings[measureReqs.texts[i]]) || "";
          ctx.font = `${measureReqs.sizes[i]}px ${font}`;
          widths[i] = ctx.measureText(text).width;
        }
      }
      await this._request("measure-acked", () => {
        this._worker.postMessage({ type: "measure-results", widths }, [widths.buffer]);
      });
    }
    const drawCtx =
      this._canvas && this._canvas.getContext ? this._canvas.getContext("2d") : null;
    const resolve = (id) => (strings && strings[id]) || "";
    const result = applyCommandBuffer(commands, {
      canvas: drawCtx,
      root: this._root,
      resolve,
      images: this._images,
      nodes: this._nodes,
    });
    return { ...result, measured: measureCount || 0 };
  }

  dispose() {
    this._events.flushNow();
    try {
      this._worker.postMessage({ type: "dispose" });
    } catch {
      /* ignore */
    }
    this._worker.terminate();
  }

  _measureCtx() {
    if (this._canvas && this._canvas.getContext) return this._canvas.getContext("2d");
    if (typeof document === "undefined") return null;
    if (!this._measureCanvas) {
      this._measureCanvas = document.createElement("canvas");
      this._measureCanvas.width = 1;
      this._measureCanvas.height = 1;
    }
    return this._measureCanvas.getContext("2d");
  }

  _onMessage(msg) {
    if (!msg || !msg.type) return;
    if (msg.type === "error") {
      for (const [, p] of this._pending) {
        p.reject(Object.assign(new Error(msg.message), { code: msg.code }));
      }
      this._pending.clear();
      return;
    }
    const waiters = [...this._pending.entries()].filter(([, p]) => p.expect === msg.type);
    if (waiters.length) {
      const [id, p] = waiters[0];
      this._pending.delete(id);
      p.resolve(msg);
    }
  }

  _request(expect, send) {
    const id = ++this._req;
    return new Promise((resolve, reject) => {
      this._pending.set(id, { expect, resolve, reject });
      send();
    });
  }
}
