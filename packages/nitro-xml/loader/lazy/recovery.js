// SPDX-License-Identifier: MIT
// Loaded only after a worker crash; normal opens retain snapshot/journal setup.
import { VIEWER_RECOVERY_FAILED_MESSAGE, EDITOR_RECOVERY_FAILED_MESSAGE } from "../formats.js";
export async function reloadOnce() {
if (this._viewerReloadAttempted) {
throw Object.assign(new Error(VIEWER_RECOVERY_FAILED_MESSAGE), { code: "viewer_recovery_exhausted" });
}
const snapshot = await this._ensureOriginalBytes();
if (!snapshot) throw Object.assign(new Error("no bytes"), { code: "viewer_recovery_failed" });
this._viewerReloadAttempted = true;
swapWorker.call(this);
await this._openBytes(new Uint8Array(snapshot), this._name, { readonly: this._ro });
return this;
}
export async function recoverEditOnce() {
if (this._editRecoveryAttempted) {
throw Object.assign(new Error(EDITOR_RECOVERY_FAILED_MESSAGE), {
code: "editor_recovery_exhausted",
});
}
try {
await this._ensureOriginalBytes();
} catch {
throw Object.assign(new Error("no bytes"), { code: "editor_recovery_failed" });
}
await this._ej();
if (!this._journal) {
throw Object.assign(new Error("no journal"), { code: "editor_recovery_failed" });
}
this._editRecoveryAttempted = true;
swapWorker.call(this);
const journal = this._journal;
const res = await journal.recover({
openFresh: async (bytes) => {
await this._openBytes(bytes, this._name, { readonly: false });
return this;
},
applyOps: async (_session, op) => {
const spec = typeof op.spec === "string" ? op.spec : JSON.stringify(op);
this._replayingJournal = true;
try {
await this._request("apply-edit", () => {
this._worker.postMessage({ type: "apply-edit", spec: spec || "" });
});
} finally {
this._replayingJournal = false;
}
},
});
if (!res.ok) {
const err =
res.error instanceof Error
? res.error
: Object.assign(new Error(String(res.error || EDITOR_RECOVERY_FAILED_MESSAGE)), {
cause: res.error,
});
err.code = res.lastGoodSave ? "editor_recovery_last_save" : err.code || "editor_recovery_failed";
err.lastGoodSave = res.lastGoodSave || null;
throw err;
}
this._editRecoveryAttempted = false;
return this;
}
function swapWorker() {
this._events.flushNow();
try {
this._worker.postMessage({ type: "dispose" });
} catch {
}
this._worker.terminate();
for (const [, p] of this._pending) p.reject(new Error("document replaced"));
this._pending.clear();
this._fatalError = null;
this._worker = this._newWorker(false);
this._wireWorker();
}

export async function recoverElement(edit) {
if (!this._d || (this._readonly === false) !== edit) return;
try {
this._clr();
await (edit ? this._d.recoverEditOnce() : this._d.reloadOnce());
await this._ao();
this._tick();
} catch (e) {
this._showViewerError(e);
}
}
