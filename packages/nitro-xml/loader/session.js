// SPDX-License-Identifier: MIT
/**
 * Document session: handle wrap, FinalizationRegistry, per-frame tick
 * (measure batch → command apply → event flush).
 */

import { initCore, getWasmMemory, unsupportedFormatError } from "./bootstrap.js";
import { isSupportedExtension } from "./formats.js";
import { applyCommandBuffer } from "./apply.js";
import { createEventCoalescer } from "./events.js";
import { flushMeasureBatch, readMeasureReqsFromMemory } from "./measure.js";

const registry =
  typeof FinalizationRegistry !== "undefined"
    ? new FinalizationRegistry((free) => {
        try {
          free();
        } catch {
          /* already freed */
        }
      })
    : null;

/**
 * Open a document from bytes. Extension is a hint (allow-list); engine content-type
 * checks land with #13.
 *
 * @param {ArrayBuffer | Uint8Array} bytes
 * @param {{ name?: string, canvas?: HTMLCanvasElement, root?: Element }} [opts]
 */
export async function openDocument(bytes, opts = {}) {
  if (opts.name && !isSupportedExtension(opts.name)) {
    throw unsupportedFormatError();
  }
  const { glue, memory } = await initCore();
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  const handle = new glue.Handle(u8);
  return new DocumentSession(handle, {
    memory: memory || getWasmMemory(),
    canvas: opts.canvas || null,
    root: opts.root || null,
    name: opts.name || "",
  });
}

export class DocumentSession {
  /**
   * @param {any} handle wasm Handle
   * @param {{ memory: WebAssembly.Memory|null, canvas: HTMLCanvasElement|null, root: Element|null, name: string }} opts
   */
  constructor(handle, opts) {
    this._handle = handle;
    this._memory = opts.memory;
    this._canvas = opts.canvas;
    this._root = opts.root;
    this._name = opts.name;
    this._nodes = new Map();
    this._images = new Map();
    this._disposed = false;
    this._measureCtx = null;

    const h = handle;
    this._events = createEventCoalescer((batch) => {
      if (this._disposed) return;
      h.pushEventBatch(batch);
    });

    if (registry) {
      const free = () => {
        try {
          h.free();
        } catch {
          /* ignore */
        }
      };
      this._registryToken = {};
      registry.register(this, free, this._registryToken);
    }
  }

  get handle() {
    return this._handle;
  }

  get name() {
    return this._name;
  }

  /** @param {import('./events.js').LoaderEvent} ev */
  pushEvent(ev) {
    this._events.push(ev);
  }

  /**
   * One frame: batched measure read-back, command-buffer apply, event flush.
   * @param {{ canvas?: HTMLCanvasElement|null, root?: Element|null }} [opts]
   */
  tick(opts = {}) {
    if (this._disposed) return { measured: 0, ops: 0 };
    if (opts.canvas !== undefined) this._canvas = opts.canvas;
    if (opts.root !== undefined) this._root = opts.root;

    const ctx = this._ensureMeasureCtx();
    const handle = this._measureHandle(ctx);
    const measured = flushMeasureBatch(handle, ctx);

    const commands = this._handle.takeFrameCommands();
    const drawCtx = this._canvas && this._canvas.getContext
      ? this._canvas.getContext("2d")
      : null;
    const { ops, error } = applyCommandBuffer(commands, {
      canvas: drawCtx,
      root: this._root,
      resolve: (id) => this._handle.resolve(id),
      images: this._images,
      nodes: this._nodes,
    });

    this._events.flushNow();
    return { measured, ops, error };
  }

  dispose() {
    if (this._disposed) return;
    this._disposed = true;
    this._events.flushNow();
    if (registry && this._registryToken) registry.unregister(this._registryToken);
    try {
      this._handle.free();
    } catch {
      /* ignore */
    }
  }

  _ensureMeasureCtx() {
    if (this._canvas && this._canvas.getContext) {
      return this._canvas.getContext("2d");
    }
    if (typeof document === "undefined") return null;
    if (!this._measureCtx) {
      const c = document.createElement("canvas");
      c.width = 1;
      c.height = 1;
      this._measureCtx = c.getContext("2d");
    }
    return this._measureCtx;
  }

  _measureHandle(ctx) {
    const h = this._handle;
    const memory = this._memory;
    return {
      get measureCount() {
        return h.measureCount;
      },
      resolve: (id) => h.resolve(id),
      setMeasureResults: (widths) => h.setMeasureResults(widths),
      takeMeasureReqs: () => {
        if (!memory) return [];
        return readMeasureReqsFromMemory(
          {
            measureCount: h.measureCount,
            measureFontPtr: h.measureFontPtr,
            measureSizePtr: h.measureSizePtr,
            measureTextPtr: h.measureTextPtr,
          },
          memory,
        );
      },
      // ctx kept for API symmetry; flushMeasureBatch receives it separately
      _ctx: ctx,
    };
  }
}
