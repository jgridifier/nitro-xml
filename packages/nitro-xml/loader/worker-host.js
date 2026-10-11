// SPDX-License-Identifier: MIT
/**
 * Main-thread client for a dedicated document worker.
 * Transfers the file ArrayBuffer in; applies frame command buffers locally;
 * runs measureText once per frame and posts widths back as one batch;
 * answers prior-frame layout read-backs in one batch (never per-element).
 */

import { takePrewarmedWorker, prewarmWorker } from "./bootstrap.js";
import { applyCommandBuffer } from "./apply.js";
import { createEventCoalescer } from "./events.js";
import { isSupportedExtension } from "./formats.js";
import { unsupportedFormatError } from "./bootstrap.js";
import { getDefaultFontSource } from "./font-source.js";
import { flushLayoutReadBatch } from "./layout-readback.js";

export class WorkerDocument {
  /**
   * @param {{
   *   canvas?: HTMLCanvasElement|null,
   *   root?: Element|null,
   *   name?: string,
   *   workerUrl?: string|URL,
   *   fontSource?: string | false,
   * }} [opts]
   */
  constructor(opts = {}) {
    this._canvas = opts.canvas || null;
    this._root = opts.root || null;
    this._name = opts.name || "";
    this._fontSource = opts.fontSource;
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
    /** @type {{ nodeId: number, kind: number, propId?: number }[] | null} */
    this._pendingLayoutReqs = null;
    this._strings = null;
  }

  /**
   * @param {ArrayBuffer} buffer transferred (caller must not use after)
   * @param {string} [name]
   */
  async open(buffer, name) {
    const n = name || this._name;
    if (n && !isSupportedExtension(n)) throw unsupportedFormatError();
    this._name = n;
    const fontSource =
      this._fontSource !== undefined ? this._fontSource : getDefaultFontSource();
    // Functions cannot be structured-cloned into the worker; use the
    // main-thread DocumentSession path for resolver callbacks.
    const serializable =
      typeof fontSource === "function" ? undefined : fontSource;
    return this._request("opened", () => {
      this._worker.postMessage(
        { type: "open", bytes: buffer, name: n, fontSource: serializable },
        [buffer],
      );
    });
  }

  /** @param {import('./events.js').LoaderEvent} ev */
  pushEvent(ev) {
    this._events.push(ev);
  }

  /**
   * Pull one frame from the worker, answer prior layout reads, measure, apply,
   * queue this frame's layout requests for the next tick.
   */
  async tick() {
    this._events.flushNow();
    const layoutReads = await this._flushPendingLayout();
    const frame = await this._request("frame", () => {
      this._worker.postMessage({ type: "take-frame" });
    });
    const {
      commands,
      measureReqs,
      measureCount,
      layoutReqs,
      layoutCount,
      strings,
    } = frame;
    this._strings = strings || this._strings;
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
    this._queueLayout(layoutReqs, layoutCount || 0);
    return { ...result, measured: measureCount || 0, layoutReads };
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

  async _flushPendingLayout() {
    const reqs = this._pendingLayoutReqs;
    this._pendingLayoutReqs = null;
    if (!reqs || reqs.length === 0) return 0;
    const strings = this._strings;
    const valuesHolder = { values: null };
    const n = flushLayoutReadBatch(
      {
        layoutCount: reqs.length,
        resolve: (id) => (strings && strings[id]) || "",
        takeLayoutReqs: () => reqs,
        setLayoutResults: (values) => {
          valuesHolder.values = values;
        },
      },
      this._nodes,
    );
    if (valuesHolder.values) {
      const values = valuesHolder.values;
      await this._request("layout-acked", () => {
        this._worker.postMessage({ type: "layout-results", values }, [values.buffer]);
      });
    }
    return n;
  }

  _queueLayout(layoutReqs, layoutCount) {
    if (!layoutCount || !layoutReqs) {
      this._pendingLayoutReqs = null;
      return;
    }
    const reqs = new Array(layoutCount);
    for (let i = 0; i < layoutCount; i++) {
      reqs[i] = {
        nodeId: layoutReqs.nodes[i],
        kind: layoutReqs.kinds[i],
        propId: layoutReqs.props[i],
      };
    }
    this._pendingLayoutReqs = reqs;
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
