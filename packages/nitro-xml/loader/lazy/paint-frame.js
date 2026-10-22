// SPDX-License-Identifier: MIT
/**
 * Host-side paint for a frame already returned by mut-frame / take-frame.
 * Mirrors worker-host-tick paint half so edit paths skip a second round-trip.
 */
import { applyCommandBuffer } from "../apply.js";
import { fillMeasureWidths } from "../measure.js";
import { applyChromePatch } from "./chrome-patch.js";

/** Drop transferable / large fields so the mut-frame message is not retained after paint. */
function releaseFrame(frame) {
  if (!frame) return;
  frame.commands = null;
  frame.measureReqs = null;
  frame.outlineReqs = null;
  frame.layoutReqs = null;
  frame.strings = null;
  frame.h = null;
  frame.hf = null;
  frame.result = null;
}

export async function paintFrame(doc, frame) {
  try {
    const {
      commands,
      measureReqs,
      measureCount,
      outlineReqs,
      outlineCount,
      layoutReqs,
      layoutCount,
      strings,
    } = frame;
    doc._strings = strings || doc._strings;
    doc._measureCount = measureCount | 0;
    doc._outlineCount = outlineCount | 0;
    doc._t = frame.t !== false;
    const drawCtx =
      doc._canvas && doc._canvas.getContext ? doc._canvas.getContext("2d") : null;
    const resolve = (id) => (strings && strings[id]) || "";
    if (doc._chromeEl && (frame.h || frame.hf)) {
      applyChromePatch(doc._chromeEl, { h: frame.h, hf: frame.hf });
    } else if (frame.h && doc.onChrome) doc.onChrome(frame.h);
    // Empty commands mean the layout is not ready (measures still in flight). Wiping the
    // root here blanks the page and hides the caret; keep the last good frame.
    const keep = commands.byteLength === 0 && doc._root && doc._root.firstChild;
    const result = keep
      ? { ops: 0 }
      : applyCommandBuffer(commands, {
          canvas: drawCtx,
          root: doc._root,
          resolve,
          images: doc._images,
          nodes: doc._nodes,
        });
    doc._queueLayout(layoutReqs, layoutCount || 0);
    if (measureCount > 0 && measureReqs) {
      const ctx = doc._measureCtx();
      const widths = fillMeasureWidths(
        ctx,
        measureCount,
        (i) => (strings && strings[measureReqs.fonts[i]]) || "sans-serif",
        (i) => measureReqs.sizes[i],
        (i) => (strings && strings[measureReqs.texts[i]]) || "",
      );
      await doc._request("measure-acked", () => {
        doc._worker.postMessage({ type: "measure-results", widths }, [widths.buffer]);
      });
    }
    if (outlineCount > 0 && outlineReqs) {
      const { answerWorkerOutlines } = await import("./glyph-outline.js");
      await answerWorkerOutlines(
        doc._measureCtx(),
        outlineReqs,
        outlineCount,
        strings,
        (n, packed) =>
          doc._request("outline-acked", () => {
            doc._worker.postMessage({ type: "outline-results", count: n, packed }, [
              packed.buffer,
            ]);
          }),
      );
    }
    doc._rt = measureCount > 0 ? (doc._rt | 0) + 1 : 0;
    if (doc._rt > 0 && doc._rt <= 6) void doc.tick().catch(() => {});
    return {
      ...result,
      measured: measureCount | 0,
      outlined: outlineCount | 0,
      layoutReads: 0,
    };
  } finally {
    // Drop large message fields so mut-frame payloads are not retained on small
    // sheets (JS heap peak was dominated by the kept frame + chrome HTML).
    releaseFrame(frame);
  }
}
