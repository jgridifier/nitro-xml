// SPDX-License-Identifier: MIT
/** Patch `.chrome` from a full `h` or formula-only `hf` edit payload. */

export function applyChromePatch(ch, patch) {
  if (!ch || !patch) return;
  const hf = patch.hf;
  if (hf != null) {
    const x = ch.querySelector(".xfb");
    if (x) x.outerHTML = hf;
    else ch.insertAdjacentHTML("afterbegin", hf);
    return;
  }
  const h = patch.h;
  if (h != null) ch.innerHTML = h;
}
