// SPDX-License-Identifier: MIT
/**
 * Worker-side OutlineBatch take/apply helpers.
 * Dynamic-imported from worker.js only when outlineCount > 0 (or outline-results).
 */

export function takeOutlineFrame(handle) {
  const t = handle.takeOutlineReqs();
  return {
    outlineReqs: { fonts: t[0], chars: t[1], sizes: t[2] },
    outlineCount: t[0].length,
  };
}

export function transferOutlineBuffers(outlineReqs, transfer) {
  transfer.push(
    outlineReqs.fonts.buffer,
    outlineReqs.chars.buffer,
    outlineReqs.sizes.buffer,
  );
}

export function applyOutlineResults(handle, msg) {
  const packed = msg.packed || new Float64Array(0);
  handle.setOutlineResults(packed);
  return msg.count | 0;
}
