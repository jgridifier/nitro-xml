// SPDX-License-Identifier: MIT
export async function attach(el) {
  await (await import("./s.js")).attach(el);
  if (el.localName === "nitro-xlsx") await (await import("./xlsx-selection.js")).attach(el);
  if (!el._readonly) await (await import("./input-proxy.js")).attach(el);
}
