// SPDX-License-Identifier: MIT
/**
 * Shared worker frame export (lazy — off eager brotli budget).
 * Used by take-frame and mut-frame (edit+paint in one round-trip).
 */

/** Strip chromeHtml/ui from mut-frame apply results — delivered via `h` instead. */
function lightMutResult(result) {
  if (typeof result !== "string") return result;
  try {
    const o = JSON.parse(result || "{}");
    return JSON.stringify({
      save: !!o.save,
      ribbonAction: o.ribbonAction ?? null,
      handled: o.handled,
      k: o.k,
    });
  } catch {
    return result;
  }
}

export async function buildAndPostFrame(handle, workerSelf, extra = {}) {
  if (handle.t !== false || workerSelf._openFrameApplied) handle.prepareFrame();
  else workerSelf._openFrameApplied = true;
  let chromeHtml =
    typeof handle.takeChromeIfDirty === "function"
      ? handle.takeChromeIfDirty() || null
      : null;
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
  if (workerSelf._pendingEditorChrome === 1) {
    workerSelf._pendingEditorChrome = 2;
  } else if (workerSelf._pendingEditorChrome === 2) {
    workerSelf._pendingEditorChrome = 0;
    await new Promise((r) => setTimeout(r, 0));
    try {
      handle.setReadOnly(false);
      handle.loadEditorChrome();
    } catch {}
  }
}
