// SPDX-License-Identifier: MIT
// Unsaved-changes tracking for an editable viewer element. The engine has no "modified" flag, so
// the frame compares the serialised workbook against a baseline (bytes at open, then at each save).
// Unedited documents serialise to their original bytes, so the compare is exact and cheap.
// The probe asks the worker for a save directly (not `WorkerDocument.save`) so it never touches the
// crash-recovery journal's last-good-save blob.
const fnv = (u8) => {
  let h = 2166136261;
  for (let i = 0; i < u8.length; i++) h = Math.imul(h ^ u8[i], 16777619) >>> 0;
  return h + ":" + u8.length;
};

async function peek(el) {
  const d = el._d;
  if (!d || !d._worker || !d._request) return null;
  try {
    const msg = await d._request("saved", () => d._worker.postMessage({ type: "save" }));
    return msg && msg.bytes ? fnv(new Uint8Array(msg.bytes)) : null;
  } catch {
    return null;
  }
}

/** @returns {{ rebase(): Promise<void>, force(): void, touch(): void, saved(bytes: Uint8Array): void, destroy(): void }} */
export function trackDirty(el, onChange) {
  let base = null;
  let dirty = false;
  let timer = 0;
  let live = true;
  let forced = false;
  const set = (v) => {
    if (v !== dirty) onChange((dirty = v));
  };
  async function check() {
    const now = await peek(el);
    if (!live || forced || now == null || base == null) return;
    set(now !== base);
  }
  const touch = () => {
    clearTimeout(timer);
    timer = setTimeout(check, 350);
  };
  // Typed edits go through the element's key handler, so listen for key and clipboard activity too.
  const evs = ["keyup", "input", "paste", "cut", "pointerup"];
  for (const e of evs) el.addEventListener(e, touch);
  return {
    async rebase() {
      forced = false;
      base = await peek(el);
      set(false);
    },
    force() {
      forced = true;
      set(true);
    },
    touch,
    saved(bytes) {
      forced = false;
      base = fnv(new Uint8Array(bytes));
      set(false);
    },
    destroy() {
      live = false;
      clearTimeout(timer);
      for (const e of evs) el.removeEventListener(e, touch);
    },
  };
}
