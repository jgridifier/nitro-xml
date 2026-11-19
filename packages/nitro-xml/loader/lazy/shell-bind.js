// SPDX-License-Identifier: MIT
// Existing shell keyboard/chrome handlers, loaded before the first document opens.
export function bindShell() {
const g = (o, a) => this.shellAction(o, a || "");
const ch = this._ch;
ch.onclick = (e) => {
const b = e.target.closest && e.target.closest("[data-a]");
b && g(b.dataset.a, b.dataset.g);
};
ch.oninput = (e) => {
e.target.classList.contains("q") && g("find-query", e.target.value);
};
const mac=typeof navigator!="undefined"&&/mac|iphone|ipad|ipod/i.test((navigator.userAgentData&&navigator.userAgentData.platform)||navigator.platform||"");
this.onkeydown = (e) => {
if (!this._d) return;
const q = e.target.classList.contains("q");
const m = e.ctrlKey || e.metaKey;
let k = (m ? "m" : "") + e.key;
if (q && e.key == "Enter") k = e.shiftKey ? "qSEnter" : "qEnter";
else if (q && e.key == "Escape") k = "qEscape";
const hkRoot = (ch.querySelector && ch.querySelector("[data-hk]")) || ch.firstElementChild;
for (const p of ((hkRoot && hkRoot.dataset.hk) || "").split("|")) {
const i = p.indexOf(":");
if (p.slice(0, i) === k) {
if (k == "Escape" && this._d.editorKey) return this._d.editorKey(e, mac).then((x) => (x && x.handled ? paintEdit(x) : g("find-close")));
e.preventDefault();
return g(p.slice(i + 1));
}
}
const inFind = (e.composedPath ? e.composedPath() : []).some(
(n) => n.classList && n.classList.contains("q"),
);
const paintEdit = (edit) => {
if (!edit || !edit.handled) return;
e.preventDefault();
if (edit.h != null && this._ch) this._ch.innerHTML = edit.h;
this._tick();
};
if (!this._readonly && (e.ctrlKey || e.metaKey || e.altKey)) {
void this._d.matchShortcut(e, mac).then((hit) => {
if (hit && (hit.command || hit.action)) {
e.preventDefault();
if (hit.command) g("cmd", hit.command);
else if (hit.action === "help") g("diagnostics-toggle");
return;
}
if (!inFind && this._d.editorKey) return this._d.editorKey(e, mac).then(paintEdit);
});
return;
}
if (!this._readonly && !inFind && this._d.editorKey) {
void this._d.editorKey(e, mac).then(paintEdit);
}
};
}
