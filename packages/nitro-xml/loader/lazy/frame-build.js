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
  // Editable open: finish soft-share ensure_edit *before* posting the first
  // frame so waitFirstPaint cannot return and start timed edits while the
  // worker is still materialising (that race put ~1s back into sample0).
  if (workerSelf._pendingEditorChrome === 1) {
    workerSelf._pendingEditorChrome = 0;
    await new Promise((r) => setTimeout(r, 0));
    try {
      handle.setReadOnly(false);
      handle.loadEditorChrome();
    } catch {}
  }
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
  let measureReqs = null;
  let measureCount = 0;
  if ((handle.measureCount | 0) > 0) {
    const t = handle.takeMeasureReqs();
    measureReqs = { fonts: t[0], sizes: t[1], texts: t[2] };
    measureCount = measureReqs.fonts.length;
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
  const type = extra.type || "frame";
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
}
