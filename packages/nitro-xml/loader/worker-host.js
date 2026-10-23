// SPDX-License-Identifier: MIT
import { takePrewarmedWorker, prewarmWorker, unsupportedFormatError } from "./bootstrap.js";
import { createEventCoalescer } from "./events.js";
import {
  isSupportedExtension,
  VIEWER_RECOVERY_FAILED_MESSAGE,
  EDITOR_RECOVERY_FAILED_MESSAGE,
} from "./formats.js";
import { getDefaultFontSource } from "./font-source.js";
import { flushLayoutReadBatch, packLayoutReqs } from "./layout-readback.js";
export class WorkerDocument {
constructor(opts = {}) {
this._canvas = opts.canvas || null;
this._root = opts.root || null;
this._name = opts.name || "";
this._fontSource = opts.fontSource;
this._workerUrl = opts.workerUrl;
this._nodes = new Map();
this._images = new Map();
this._pending = new Map();
this._req = 0;
this._originalBytes = null;
this._ro = true;
this._viewerReloadAttempted = false;
this._editRecoveryAttempted = false;
this._journal = null;
this._replayingJournal = false;
this._worker = this._newWorker(true);
this._wireWorker();
this._events = createEventCoalescer((bytes) => {
this._worker.postMessage({ type: "events", bytes }, [bytes.buffer]);
});
this._measureCanvas = null;
this._pendingLayoutReqs = null;
this._strings = null;
this._measureCount = 0;
this._outlineCount = 0;
this._t = true;
}
get measureCount() {
return this._measureCount | 0;
}
get outlineCount() {
return this._outlineCount | 0;
}
get sheetTailComplete() {
return this._t !== false;
}
_newWorker(prewarm) {
const url = this._workerUrl || new URL("./worker.js", import.meta.url);
return (
(prewarm && takePrewarmedWorker()) ||
(prewarm && prewarmWorker(this._workerUrl)) ||
new Worker(url, { type: "module", name: "nitro-xml-doc" })
);
}
_wireWorker() {
this._worker.onmessage = (ev) => this._onMessage(ev.data);
this._worker.onerror = () => {
const err = Object.assign(new Error("Worker crashed"), { code: "worker_crashed" });
for (const [, p] of this._pending) p.reject(err);
this._pending.clear();
};
}
async open(buffer, name, opts = {}) {
const n = name || this._name;
if (n && !isSupportedExtension(n)) throw unsupportedFormatError();
this._name = n;
const u8 = buffer instanceof Uint8Array ? buffer : new Uint8Array(buffer);
this._originalBytes = new Uint8Array(u8);
this._ro = opts.readonly !== false;
this._viewerReloadAttempted = false;
this._editRecoveryAttempted = false;
this._journal = null;
return this._openBytes(u8, n, opts);
}
async _ej() {
if (this._journal || this._ro || !this._originalBytes) return this._journal;
const { createJournal } = await import("./lazy/journal.js");
return (this._journal = createJournal(this._originalBytes));
}
async reloadOnce() {
if (this._viewerReloadAttempted) {
throw Object.assign(new Error(VIEWER_RECOVERY_FAILED_MESSAGE), { code: "viewer_recovery_exhausted" });
}
if (!this._originalBytes) throw Object.assign(new Error("no bytes"), { code: "viewer_recovery_failed" });
this._viewerReloadAttempted = true;
this._swapWorker();
await this._openBytes(new Uint8Array(this._originalBytes), this._name, { readonly: this._ro });
return this;
}
async recoverEditOnce() {
if (this._editRecoveryAttempted) {
throw Object.assign(new Error(EDITOR_RECOVERY_FAILED_MESSAGE), {
code: "editor_recovery_exhausted",
});
}
await this._ej();
if (!this._journal) {
throw Object.assign(new Error("no journal"), { code: "editor_recovery_failed" });
}
this._editRecoveryAttempted = true;
this._swapWorker();
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
_swapWorker() {
this._events.flushNow();
try {
this._worker.postMessage({ type: "dispose" });
} catch {
}
this._worker.terminate();
this._pending.clear();
this._worker = this._newWorker(false);
this._wireWorker();
}
async _openBytes(buffer, name, opts = {}) {
const n = name || this._name;
const fontSource =
this._fontSource !== undefined ? this._fontSource : getDefaultFontSource();
const serializable =
typeof fontSource === "function" ? undefined : fontSource;
const readonly = opts.readonly !== false;
const u8 = buffer instanceof Uint8Array ? buffer : new Uint8Array(buffer);
const opened = await this._request("opened", () => {
this._worker.postMessage(
{ type: "open", bytes: u8, name: n, fontSource: serializable, readonly },
[u8.buffer],
);
});
this._t = opened.t !== false;
return opened;
}
async loadEditorChrome() {
return this._request("editor-chrome", () => {
this._worker.postMessage({ type: "editor-chrome" });
});
}
async editorKey(ev, mac) {
// Flush queued pointer events before the key so click-then-immediate-key
// acts on the new selection (audit theme 8 / S3-1).
this._events.flushNow();
const msg = await this._request("editor-key", () => {
this._worker.postMessage({
type: "editor-key",
key: ev.key || "",
ctrl: !!ev.ctrlKey,
meta: !!ev.metaKey,
alt: !!ev.altKey,
shift: !!ev.shiftKey,
mac: !!mac,
});
});
try { return JSON.parse(msg.result || "{}"); } catch { return {}; }
}
async save() {
const bytes = (await this._request("saved", () => {
this._worker.postMessage({ type: "save" });
})).bytes;
if (this._journal && bytes) this._journal.markSave(bytes);
return bytes;
}
async applyEdit(spec) {
await this._ej();
const result = (await this._request("apply-edit", () => {
this._worker.postMessage({ type: "apply-edit", spec: spec || "" });
})).result;
if (this._journal && !this._replayingJournal && spec) {
this._journal.append({ spec: String(spec) });
}
return result;
}
async matchShortcut(ev, mac) {
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
pushEvent(ev) {
this._events.push(ev);
}
async shellAction(op, arg = "") {
const msg = await this._request("sa", () => this._worker.postMessage({ type: "sa", op, arg: arg || "" }));
try { return JSON.parse(msg.result); } catch { return { ui: null }; }
}
async tick() {
const prev = this._tickLock || Promise.resolve();
let release;
this._tickLock = new Promise((r) => { release = r; });
await prev;
try {
const { tickBody } = await import("./lazy/worker-host-tick.js");
return await tickBody(this);
} finally {
release();
}
}
dispose() {
this._events.flushNow();
try{this._worker.postMessage({type:"dispose"});}catch{}
takePrewarmedWorker(this._worker);this._worker=null;
}
async _flushPendingLayout() {
const reqs = this._pendingLayoutReqs;
this._pendingLayoutReqs = null;
if (!reqs || reqs.length === 0) return 0;
const strings = this._strings;
const valuesHolder = { values: null };
const n = flushLayoutReadBatch(
{
layoutCount: reqs.length,
resolve: (id) => (strings && strings[id]) || "",
takeLayoutReqs: () => reqs,
setLayoutResults: (values) => {
valuesHolder.values = values;
},
},
this._nodes,
);
if (valuesHolder.values) {
const values = valuesHolder.values;
await this._request("layout-acked", () => {
this._worker.postMessage({ type: "layout-results", values }, [values.buffer]);
});
}
return n;
}
_queueLayout(layoutReqs, layoutCount) {
if (!layoutCount || !layoutReqs) {
this._pendingLayoutReqs = null;
return;
}
this._pendingLayoutReqs = packLayoutReqs(
layoutReqs.nodes,
layoutReqs.kinds,
layoutReqs.props,
);
}
_measureCtx() {
if (typeof document === "undefined")
return this._canvas && this._canvas.getContext ? this._canvas.getContext("2d") : null;
if (!this._measureCanvas) {
this._measureCanvas = document.createElement("canvas");
this._measureCanvas.width = 1;
this._measureCanvas.height = 1;
}
return this._measureCanvas.getContext("2d");
}
_onMessage(msg) {
if (!msg || !msg.type) return;
if (msg.type === "error") {
for (const [, p] of this._pending) {
p.reject(Object.assign(new Error(msg.message), { code: msg.code }));
}
this._pending.clear();
return;
}
const waiters = [...this._pending.entries()].filter(([, p]) => p.expect === msg.type);
if (waiters.length) {
const [id, p] = waiters[0];
this._pending.delete(id);
p.resolve(msg);
}
}
_request(expect, send) {
const id = ++this._req;
return new Promise((resolve, reject) => {
this._pending.set(id, { expect, resolve, reject });
send();
});
}
}
