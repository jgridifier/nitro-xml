// SPDX-License-Identifier: MIT
export async function attach(el) {
  await (await import("./s.js")).attach(el);
  if (!el._readonly) await (await import("./input-proxy.js")).attach(el);
  await (await import("./host-ribbon.js")).attachRibbon(el);
}
