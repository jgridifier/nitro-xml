// SPDX-License-Identifier: MIT
import { attachHyperlinks } from "./hyperlink.js";
import { attachTestIds } from "./testids.js";
export function attach(el) {
  attachHyperlinks(el);
  el._testIdsOff?.();
  el._testIdsOff = attachTestIds(el.shadowRoot, el.localName.slice(6));
  import("./s.js").then((m) => m.attach(el)).catch(() => {});
  if (!el._readonly) import("./input-proxy.js").then((m) => m.attach(el)).catch(() => {});
}
