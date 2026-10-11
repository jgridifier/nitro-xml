// SPDX-License-Identifier: MIT
/** Document worker: owns wasm Handle; DOM/canvas apply stays on the main thread. */
import {
  initCore,
  getWasmMemory,
  unsupportedFormatError,
} from "./bootstrap.js";
import { isSupportedExtension } from "./formats.js";
import { applyFontSourceToHandle } from "./font-source.js";

let handle = null;
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
        // Serializable fontSource only (string | false | "off"); resolvers use DocumentSession.
        if (msg.fontSource !== undefined && typeof msg.fontSource !== "function") {
          applyFontSourceToHandle(handle, msg.fontSource);
        } else {
          applyFontSourceToHandle(handle, undefined);
        }
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
        let layoutReqs = null;
        let layoutCount = 0;
        if ((handle.layoutCount | 0) > 0) {
          const taken = handle.takeLayoutReqs();
          layoutReqs = { nodes: taken[0], kinds: taken[1], props: taken[2] };
          layoutCount = layoutReqs.nodes.length;
          if (!strings) strings = {};
          for (let i = 0; i < layoutCount; i++) {
            const p = layoutReqs.props[i];
            if (p && !(p in strings)) strings[p] = handle.resolve(p) || "";
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
        if (layoutReqs) {
          transfer.push(
            layoutReqs.nodes.buffer,
            layoutReqs.kinds.buffer,
            layoutReqs.props.buffer,
          );
        }
        self.postMessage(
          {
            type: "frame",
            commands,
            measureReqs,
            measureCount,
            layoutReqs,
            layoutCount,
            strings,
          },
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
      case "layout-results": {
        if (!handle) throw new Error("no document");
        handle.setLayoutResults(msg.values);
        self.postMessage({ type: "layout-acked", count: msg.values.length });
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
