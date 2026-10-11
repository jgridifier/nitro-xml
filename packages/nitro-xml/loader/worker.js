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
        if (msg.name) {
          try {
            handle.openDocument(msg.name);
          } catch {
            /* non-viewer formats: inspect-only until their painters land */
          }
        }
        self.postMessage({
          type: "opened",
          byteLength: handle.byteLength,
          format: handle.documentFormat(),
        });
        break;
      }
      case "events": {
        if (!handle) throw new Error("no document");
        const n = handle.pushEventBatch(msg.bytes);
        self.postMessage({ type: "events-acked", count: n });
        break;
      }
      case "take-frame": {
        if (!handle) throw new Error("no document");
        handle.prepareFrame();
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
        // exportStrings() returns a JS Array (wasm-bindgen js_sys::Array), not a Map.
        const exported = handle.exportStrings();
        const strings = {};
        for (let i = 0; i < exported.length; i++) {
          strings[i] = exported[i];
        }
        let layoutReqs = null;
        let layoutCount = 0;
        if ((handle.layoutCount | 0) > 0) {
          const taken = handle.takeLayoutReqs();
          layoutReqs = { nodes: taken[0], kinds: taken[1], props: taken[2] };
          layoutCount = layoutReqs.nodes.length;
          for (let i = 0; i < layoutCount; i++) {
            const p = layoutReqs.props[i];
            if (p && strings[p] === undefined) strings[p] = handle.resolve(p) || "";
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
      case "shell-action":
        if (!handle) throw new Error("no document");
        self.postMessage({ type: "shell-action", result: handle.shellAction(msg.op || "ui", msg.arg || "") });
        break;
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
