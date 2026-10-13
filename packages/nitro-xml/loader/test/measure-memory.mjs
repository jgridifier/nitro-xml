// SPDX-License-Identifier: MIT
/** Test-only: poke wasm measure SoA via raw pointers (prefer takeMeasureReqs). */

function ptr(handle, name) {
  const v = handle[name];
  return typeof v === "function" ? v.call(handle) : v;
}

export function readMeasureReqsFromMemory(handle, memory) {
  const n = handle.measureCount | 0;
  if (n === 0) return [];
  const u32 = new Uint32Array(memory.buffer, ptr(handle, "measureFontPtr"), n);
  const f32 = new Float32Array(memory.buffer, ptr(handle, "measureSizePtr"), n);
  const t32 = new Uint32Array(memory.buffer, ptr(handle, "measureTextPtr"), n);
  const out = new Array(n);
  for (let i = 0; i < n; i++) out[i] = { fontId: u32[i], sizePx: f32[i], textId: t32[i] };
  return out;
}
