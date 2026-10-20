// SPDX-License-Identifier: MIT
export const LayoutReadKind = Object.freeze({
SpanRect: 1,
ScrollHeight: 2,
ComputedStyle: 3,
NaturalWidth: 4,
});
export const LAYOUT_RESULT_LANES = 4;
/** Natural content height at `el`'s width (upstream `measureAutoFitContentHeightPx`). */
function measureScrollHeightPx(el) {
if (!el || typeof document === "undefined") return el?.scrollHeight || 0;
const doc = el.ownerDocument || document;
const width = el.offsetWidth || 0;
const clone = el.cloneNode(true);
clone.style.position = "fixed";
clone.style.visibility = "hidden";
clone.style.pointerEvents = "none";
clone.style.left = "-99999px";
clone.style.top = "0";
clone.style.right = "auto";
clone.style.bottom = "auto";
clone.style.width = `${width}px`;
clone.style.height = "auto";
clone.style.minHeight = "0";
clone.style.maxHeight = "none";
doc.body.appendChild(clone);
try {
return clone.scrollHeight || 0;
} finally {
doc.body.removeChild(clone);
}
}
export function packLayoutReqs(nodes, kinds, props) {
const n = nodes.length;
const reqs = new Array(n);
for (let i = 0; i < n; i++) {
reqs[i] = { nodeId: nodes[i], kind: kinds[i], propId: props[i] };
}
return reqs;
}
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
out[base] = measureScrollHeightPx(el);
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
const img =  (el);
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
function ptr(handle, name) {
const v = handle[name];
return typeof v === "function" ? v.call(handle) : v;
}
export function readLayoutReqsFromMemory(handle, memory) {
const n = handle.layoutCount | 0;
if (n === 0) return [];
const nodes = new Uint32Array(memory.buffer, ptr(handle, "layoutNodePtr"), n);
const kinds = new Uint8Array(memory.buffer, ptr(handle, "layoutKindPtr"), n);
const props = new Uint32Array(memory.buffer, ptr(handle, "layoutPropPtr"), n);
return packLayoutReqs(nodes, kinds, props);
}
