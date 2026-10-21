// SPDX-License-Identifier: MIT
/**
 * Outline-aware WorkerDocument tick body.
 * Loaded on first tick via dynamic import (cached); glyph-outline.js loads only
 * when outlineCount > 0.
 */
import { applyCommandBuffer } from "../apply.js";
import { fillMeasureWidths } from "../measure.js";
import { applyChromePatch } from "./chrome-patch.js";

export async function tickBody(doc) {
  if (!doc.constructor._nxPark) {
    const { patch } = await import("./wp.js");
    patch(doc.constructor);
  }
  doc._events.flushNow();
  const layoutReads = await doc._flushPendingLayout();
  const frame = await doc._request("frame", () => {
    doc._worker.postMessage({ type: "take-frame" });
  });
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
  if (doc._chromeEl && (frame.h || frame.hf)) {
    applyChromePatch(doc._chromeEl, { h: frame.h, hf: frame.hf });
  } else if (frame.h && doc.onChrome) doc.onChrome(frame.h);
  // An empty frame means "layout not ready yet" (e.g. right after an edit reopens the view), so keep
  // the last good frame instead of clearing the page; the re-tick below paints the real one.
  const keep = commands.byteLength === 0 && doc._root && doc._root.firstChild;
  const result = keep ? { ops: 0 } : applyCommandBuffer(commands, {
    canvas: drawCtx,
    root: doc._root,
    resolve,
    images: doc._images,
    nodes: doc._nodes,
  });
  doc._queueLayout(layoutReqs, layoutCount || 0);
  // Measured widths only land in the next frame; bounded so unmeasurable text cannot spin.
  doc._rt = measureCount > 0 ? (doc._rt | 0) + 1 : 0;
  if (doc._rt > 0 && doc._rt <= 6) void doc.tick().catch(() => {});
  return {
    ...result,
    measured: measureCount || 0,
    outlined: outlineCount || 0,
    layoutReads,
  };
}
