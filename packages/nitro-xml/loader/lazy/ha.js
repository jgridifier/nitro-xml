// SPDX-License-Identifier: MIT
import { attach as pendingInput } from "./pending-input.js";
export async function attach(el) {
  if (!el._readonly) {
    pendingInput(el);
    if (el.localName === "nitro-docx") (await import("./docx-caret.js")).attach(el);
  }
  await (await import("./s.js")).attach(el);
  if (!el._readonly) await (await import("./input-proxy.js")).attach(el);
}
