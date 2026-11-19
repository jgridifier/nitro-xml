// SPDX-License-Identifier: MIT
import { checkInputSize, isSupportedExtension as ok, UNSUPPORTED_FORMAT_MESSAGE as UF } from "../formats.js";
import { wasmSupported as ws, unsupportedBrowserError as ube } from "../bootstrap.js";
export async function loadFile(el, f, n) {
  try {
    checkInputSize(f.size);
    n = n || f.name || "";
    if (!el._ok(n)) throw Object.assign(new Error(UF), { code: "unsupported_format" });
    await el.loadBuffer(await f.arrayBuffer(), n);
  } catch (e) { el._fail(e); throw e; }
}
export async function loadUrl(el, u) {
  try {
    if (!ws()) throw ube();
    const n = u.split("?")[0].split("#")[0].split("/").pop() || "";
    if (!el._ok(n)) throw Object.assign(new Error(UF), { code: "unsupported_format" });
    const r = await fetch(u);
    if (!r.ok) throw new Error("fetch failed: " + r.status);
    checkInputSize(Number(r.headers.get("content-length")));
    await el.loadBuffer(await r.arrayBuffer(), n);
  } catch (e) { el._fail(e); throw e; }
}
