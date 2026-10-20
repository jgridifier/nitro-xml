// SPDX-License-Identifier: MIT
/**
 * Host-side paint for a frame already returned by mut-frame / take-frame.
 * Mirrors worker-host-tick paint half so edit paths skip a second round-trip.
 */
import { applyCommandBuffer } from "../apply.js";
import { fillMeasureWidths } from "../measure.js";

export async function paintFrame(doc, frame) {
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
  const drawCtx =
    doc._canvas && doc._canvas.getContext ? doc._canvas.getContext("2d") : null;
  const resolve = (id) => (strings && strings[id]) || "";
  if (frame.h && doc.onChrome) doc.onChrome(frame.h);
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
  return {
    ...result,
    measured: measureCount || 0,
    outlined: outlineCount || 0,
    layoutReads: 0,
  };
}
