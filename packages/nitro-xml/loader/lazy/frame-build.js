// SPDX-License-Identifier: MIT
/**
 * Shared worker frame export (lazy — off eager brotli budget).
 * Used by take-frame and mut-frame (edit+paint in one round-trip).
 */

/**
 * Shrink mut-frame apply JSON so the host does not retain a second copy of the
 * chrome HTML alongside the command buffer (xlsx S heap regression). Keep the
 * short keys the element shell path needs: copy text (`c`), hyperlink (`p`),
 * save / ribbon / key handled. Nested `ui` / `h` are dropped — chrome HTML is
 * delivered once via the frame's `h` field (see promote below).
 */
export function lightMutResult(result) {
  if (typeof result !== "string") return result;
  try {
    const o = JSON.parse(result || "{}");
    return JSON.stringify({
      save: !!o.save,
      ribbonAction: o.ribbonAction ?? null,
      handled: o.handled,
      k: o.k,
      c: o.c ?? o.copyText ?? null,
      p: o.p ?? o.pendingHyperlink ?? null,
    });
  } catch {
    return result;
  }
}

/** shellAction embeds chrome in result.h/hf without setting dirty_ui — lift into frame fields. */
export function chromeFromResult(result) {
  if (typeof result !== "string") return null;
  try {
    const o = JSON.parse(result || "{}");
    const out = {};
    if (typeof o.h === "string" && o.h) out.h = o.h;
    if (typeof o.hf === "string" && o.hf) out.hf = o.hf;
    return out.h || out.hf ? out : null;
  } catch {
    return null;
  }
}

export async function buildAndPostFrame(handle, workerSelf, extra = {}) {
  const pendingChrome = workerSelf._pendingEditorChrome === 1;
  if (handle.t !== false || workerSelf._openFrameApplied) handle.prepareFrame();
  else workerSelf._openFrameApplied = true;
  let chromeHtml =
    typeof handle.takeChromeIfDirty === "function"
      ? handle.takeChromeIfDirty() || null
      : null;
  let chromeFormula =
    typeof handle.takeFormulaChromeIfDirty === "function"
      ? handle.takeFormulaChromeIfDirty() || null
      : null;
  if (!chromeHtml && !chromeFormula && (extra.type || "frame") === "mut-frame") {
    const lifted = chromeFromResult(extra.result);
    if (lifted) {
      chromeHtml = lifted.h || null;
      chromeFormula = lifted.hf || null;
    }
  }
  // xlsx mut-frame already paints a complete sticky grid (`_nxPainted`); the host
  // never re-ticks to apply CanvasTextMeasure results, so posting measures here
  // only adds a dead worker round-trip on every novel cell string. Drop pending
  // so the queue does not grow; a later take-frame can re-request on scroll/resize.
  let measureReqs = null;
  let measureCount = 0;
  const frameType = extra.type || "frame";
  const skipXlsxMutMeasures =
    frameType === "mut-frame" &&
    typeof handle.documentFormat === "function" &&
    handle.documentFormat() === "xlsx";
  if ((handle.measureCount | 0) > 0) {
    const t = handle.takeMeasureReqs();
    if (!skipXlsxMutMeasures) {
      measureReqs = { fonts: t[0], sizes: t[1], texts: t[2] };
      measureCount = measureReqs.fonts.length;
    }
  }
  let outlineReqs = null,
    outlineCount = 0,
    outlineMod = null;
  if ((handle.outlineCount | 0) > 0) {
    outlineMod = await import("./outline-worker.js");
    ({ outlineReqs, outlineCount } = outlineMod.takeOutlineFrame(handle));
  }
  const exported = handle.exportStrings();
  const strings = {};
  for (let i = 0; i < exported.length; i++) strings[i] = exported[i];
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
  if (outlineReqs) outlineMod.transferOutlineBuffers(outlineReqs, transfer);
  if (layoutReqs) {
    transfer.push(
      layoutReqs.nodes.buffer,
      layoutReqs.kinds.buffer,
      layoutReqs.props.buffer,
    );
  }
  const type = frameType;
  const result =
    type === "mut-frame" ? lightMutResult(extra.result) : extra.result;
  workerSelf.postMessage(
    {
      type,
      result,
      h: chromeHtml,
      hf: chromeFormula,
      commands,
      measureReqs,
      measureCount,
      outlineReqs,
      outlineCount,
      layoutReqs,
      layoutCount,
      strings,
      t: handle.t !== false,
    },
    transfer,
  );
  // Paint first so embed waitFirstPaint excludes soft-share; then unlock + signal
  // edit-ready so timed edits never race ensure_edit materialise.
  if (pendingChrome) {
    workerSelf._pendingEditorChrome = 0;
    await new Promise((r) => setTimeout(r, 0));
    try {
      handle.setReadOnly(false);
      handle.loadEditorChrome();
    } catch {}
    workerSelf.postMessage({ type: "er" });
  }
}
