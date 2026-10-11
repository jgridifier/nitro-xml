// SPDX-License-Identifier: MIT
/**
 * Dedicated document worker: owns wasm Handle + model. DOM/canvas apply stays on
 * the main thread (docs/adr/0002-renderer-port.md). ArrayBuffer is transferred in (zero-copy).
 */
import {
  initCore,
  getWasmMemory,
  unsupportedFormatError,
} from "./bootstrap.js";
import { isSupportedExtension } from "./formats.js";

/** @type {any} */
let handle = null;
/** @type {any} */
let glue = null;

self.onmessage = async (ev) => {
  const msg = ev.data || {};
  try {
    switch (msg.type) {
      case "prewarm": {
        await initCore();
        self.postMessage({ type: "ready" });
        break;
      }
      case "open": {
        if (msg.name && !isSupportedExtension(msg.name)) {
          throw unsupportedFormatError();
        }
        const core = await initCore();
        glue = core.glue;
        const bytes =
          msg.bytes instanceof Uint8Array ? msg.bytes : new Uint8Array(msg.bytes);
        if (handle) {
          try {
            handle.free();
          } catch {
            /* ignore */
          }
        }
        handle = new glue.Handle(bytes);
        self.postMessage({ type: "opened", byteLength: handle.byteLength });
        break;
      }
      case "events": {
        if (!handle) throw new Error("no document");
        const n = handle.pushEventBatch(msg.bytes);
        handle.drainEvents();
        self.postMessage({ type: "events-acked", count: n });
        break;
      }
      case "take-frame": {
        if (!handle) throw new Error("no document");
        const measureCount = handle.measureCount;
        let measureReqs = null;
        const mem = getWasmMemory();
        if (measureCount > 0 && mem) {
          measureReqs = {
            fonts: new Uint32Array(mem.buffer, handle.measureFontPtr, measureCount).slice(),
            sizes: new Float32Array(mem.buffer, handle.measureSizePtr, measureCount).slice(),
            texts: new Uint32Array(mem.buffer, handle.measureTextPtr, measureCount).slice(),
          };
        }
        // Resolve font/text strings for the main-thread measure pass (one batch).
        let strings = null;
        if (measureReqs) {
          strings = {};
          for (let i = 0; i < measureCount; i++) {
            const f = measureReqs.fonts[i];
            const t = measureReqs.texts[i];
            if (!(f in strings)) strings[f] = handle.resolve(f) || "";
            if (!(t in strings)) strings[t] = handle.resolve(t) || "";
          }
        }
        const commands = handle.takeFrameCommands();
        const transfer = [commands.buffer];
        if (measureReqs) {
          transfer.push(
            measureReqs.fonts.buffer,
            measureReqs.sizes.buffer,
            measureReqs.texts.buffer,
          );
        }
        self.postMessage(
          { type: "frame", commands, measureReqs, measureCount, strings },
          transfer,
        );
        break;
      }
      case "measure-results": {
        if (!handle) throw new Error("no document");
        handle.setMeasureResults(msg.widths);
        self.postMessage({ type: "measure-acked", count: msg.widths.length });
        break;
      }
      case "dispose": {
        if (handle) {
          try {
            handle.free();
          } catch {
            /* ignore */
          }
          handle = null;
        }
        glue = null;
        self.postMessage({ type: "disposed" });
        break;
      }
      default:
        self.postMessage({ type: "error", message: `unknown message ${msg.type}` });
    }
  } catch (e) {
    self.postMessage({
      type: "error",
      code: e && e.code,
      message: (e && e.message) || String(e),
    });
  }
};
