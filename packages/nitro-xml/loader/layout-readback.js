// SPDX-License-Identifier: MIT
/**
 * Post-apply layout read-back (Head of Product follow-up on #33).
 * Requests are keyed by command-buffer node id and answered in one batch on the
 * next frame — never one WASM→JS call per element.
 *
 * Kinds (see docs/boundary.md, loader README, ADR 0002):
 *   SpanRect       — getBoundingClientRect (docx canvas-measurer.ts span probe)
 *   ScrollHeight   — element.scrollHeight (pptx autofit)
 *   ComputedStyle  — parseFloat(getComputedStyle(el)[prop]) (autofit font/line)
 *   NaturalWidth   — HTMLImageElement.naturalWidth/Height (P5 image-native-size)
 */

export const LayoutReadKind = Object.freeze({
  SpanRect: 1,
  ScrollHeight: 2,
  ComputedStyle: 3,
  NaturalWidth: 4,
});

/** Fixed result slot: four f64 values per request (unused lanes are 0). */
export const LAYOUT_RESULT_LANES = 4;

/**
 * @typedef {{
 *   nodeId: number,
 *   kind: number,
 *   propId?: number,
 * }} LayoutReadReq
 *
 * @typedef {{
 *   layoutCount: number,
 *   resolve: (id: number) => string | undefined | null,
 *   setLayoutResults: (values: Float64Array) => void,
 *   takeLayoutReqs?: () => LayoutReadReq[],
 * }} LayoutReadHandle
 */

/**
 * Read layout for all pending requests against the loader's node map.
 * Writes a packed Float64Array of length `layoutCount * 4` in one call.
 *
 * @param {LayoutReadHandle} handle
 * @param {Map<number, Element>} nodes
 * @returns {number} number of requests answered (0 if none)
 */
export function flushLayoutReadBatch(handle, nodes) {
  const n = handle.layoutCount | 0;
  if (n === 0) return 0;
  const reqs =
    typeof handle.takeLayoutReqs === "function" ? handle.takeLayoutReqs() : [];
  const out = new Float64Array(n * LAYOUT_RESULT_LANES);
  if (reqs.length !== n) {
    handle.setLayoutResults(out);
    return n;
  }
  for (let i = 0; i < n; i++) {
    const r = reqs[i];
    const base = i * LAYOUT_RESULT_LANES;
    const el = nodes && nodes.get(r.nodeId);
    if (!el) continue;
    switch (r.kind) {
      case LayoutReadKind.SpanRect: {
        if (typeof el.getBoundingClientRect === "function") {
          const rect = el.getBoundingClientRect();
          out[base] = rect.x;
          out[base + 1] = rect.y;
          out[base + 2] = rect.width;
          out[base + 3] = rect.height;
        }
        break;
      }
      case LayoutReadKind.ScrollHeight: {
        out[base] = el.scrollHeight || 0;
        break;
      }
      case LayoutReadKind.ComputedStyle: {
        if (typeof getComputedStyle === "function") {
          const prop = handle.resolve(r.propId || 0) || "";
          if (prop) {
            const raw = getComputedStyle(el).getPropertyValue(prop);
            const num = Number.parseFloat(raw);
            out[base] = Number.isFinite(num) ? num : Number.NaN;
          }
        }
        break;
      }
      case LayoutReadKind.NaturalWidth: {
        const img = /** @type {HTMLImageElement} */ (el);
        out[base] = img.naturalWidth || 0;
        out[base + 1] = img.naturalHeight || 0;
        break;
      }
      default:
        break;
    }
  }
  handle.setLayoutResults(out);
  return n;
}

/**
 * Build a request list from wasm SoA pointers.
 *
 * @param {{
 *   layoutCount: number,
 *   layoutNodePtr: number,
 *   layoutKindPtr: number,
 *   layoutPropPtr: number,
 * }} handle
 * @param {WebAssembly.Memory} memory
 * @returns {LayoutReadReq[]}
 */
export function readLayoutReqsFromMemory(handle, memory) {
  const n = handle.layoutCount | 0;
  if (n === 0) return [];
  const nodes = new Uint32Array(memory.buffer, handle.layoutNodePtr, n);
  const kinds = new Uint8Array(memory.buffer, handle.layoutKindPtr, n);
  const props = new Uint32Array(memory.buffer, handle.layoutPropPtr, n);
  const out = new Array(n);
  for (let i = 0; i < n; i++) {
    out[i] = { nodeId: nodes[i], kind: kinds[i], propId: props[i] };
  }
  return out;
}
