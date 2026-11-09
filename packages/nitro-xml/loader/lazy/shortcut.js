// SPDX-License-Identifier: MIT
export async function matchShortcut(ev, mac) {
const msg = await this._request("match-shortcut", () => {
this._worker.postMessage({
type: "match-shortcut",
key: ev.key || "",
code: ev.code || "",
ctrl: !!ev.ctrlKey,
meta: !!ev.metaKey,
alt: !!ev.altKey,
shift: !!ev.shiftKey,
mac: !!mac,
});
});
try {
return JSON.parse(msg.result || "{}");
} catch {
return {};
}
}
