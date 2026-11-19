// SPDX-License-Identifier: MIT
export function attach(el) {
  import("./s.js").then((m) => m.attach(el)).catch(() => {});
  if (!el._readonly) import("./input-proxy.js").then((m) => m.attach(el)).catch(() => {});
}
