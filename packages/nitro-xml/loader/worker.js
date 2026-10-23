// SPDX-License-Identifier: MIT
import {
compileWasmModule,
initCore,
unsupportedFormatError,
} from "./bootstrap.js";
import { isSupportedExtension } from "./formats.js";
import { applyFontSourceToHandle } from "./font-source.js";
let handle = null;
let glue = null;
let chain = Promise.resolve();
function unlockChrome(op) {
if (!self._pendingEditorChrome) return;
if (op === "ui" || op === "zoom-set") return;
self._pendingEditorChrome = 0;
handle.setReadOnly(false);
handle.loadEditorChrome();
}
self.onmessage = (ev) => {
const msg = ev.data || {};
chain = chain
.then(async () => {
switch (msg.type) {
case "prewarm": {
if (msg.module) {
await compileWasmModule(msg.module);
msg.module = null;
}
await initCore();
self.postMessage({ type: "ready" });
break;
}
case "open": {
if (msg.name && !isSupportedExtension(msg.name)) {
throw unsupportedFormatError();
}
const core = await initCore();
glue = core.glue;
if (handle) {
try {
handle.free();
} catch {
}
}
{
let bytes =
msg.bytes instanceof Uint8Array ? msg.bytes : new Uint8Array(msg.bytes);
msg.bytes = null;
handle = new glue.Handle(bytes);
bytes = null;
}
if (msg.fontSource !== undefined && typeof msg.fontSource !== "function") {
applyFontSourceToHandle(handle, msg.fontSource);
} else {
applyFontSourceToHandle(handle, undefined);
}
self._openFrameApplied = false;
if (msg.name) {
try {
handle.openDocument(msg.name);
} catch (e) {
try { handle.free(); } catch {}
handle = null;
throw e;
}
}
handle.setReadOnly(true);
self._pendingEditorChrome = msg.readonly === false ? 1 : 0;
self.postMessage({
type: "opened",
byteLength: handle.byteLength,
format: handle.documentFormat(),
t: handle.t !== false,
});
break;
}
case "events": {
if (!handle) throw new Error("no document");
const n = handle.pushEventBatch(msg.bytes);
self.postMessage({ type: "events-acked", count: n });
break;
}
case "take-frame": {
if (!handle) throw new Error("no document");
const { buildAndPostFrame } = await import("./lazy/frame-build.js");
await buildAndPostFrame(handle, self, { type: "frame" });
break;
}
case "mut-frame": {
if (!handle) throw new Error("no document");
{
const op = msg.op || "a";
if (op === "a" || op === "k") unlockChrome();
else unlockChrome(msg.sa || "ui");
let result;
if (op === "a") result = handle.applyEdit(msg.spec || "");
else if (op === "k")
result = handle.editorKey(
msg.key || "",
!!msg.ctrl,
!!msg.meta,
!!msg.alt,
!!msg.shift,
!!msg.mac,
);
else result = handle.shellAction(msg.sa || "ui", msg.arg || "");
const { buildAndPostFrame } = await import("./lazy/frame-build.js");
await buildAndPostFrame(handle, self, { type: "mut-frame", result });
}
break;
}
case "measure-results": {
if (!handle) throw new Error("no document");
handle.setMeasureResults(msg.widths);
self.postMessage({ type: "measure-acked", count: msg.widths.length });
break;
}
case "outline-results": {
if (!handle) throw new Error("no document");
const ow = await import("./lazy/outline-worker.js");
self.postMessage({ type: "outline-acked", count: ow.applyOutlineResults(handle, msg) });
break;
}
case "layout-results": {
if (!handle) throw new Error("no document");
handle.setLayoutResults(msg.values);
self.postMessage({ type: "layout-acked", count: msg.values.length });
break;
}
case "sa":
if (!handle) throw new Error("no document");
{
const op = msg.op || "ui";
unlockChrome(op);
self.postMessage({ type: "sa", result: handle.shellAction(op, msg.arg || "") });
}
break;
case "match-shortcut":
if (!handle) throw new Error("no document");
self.postMessage({
type: "match-shortcut",
result: handle.matchShortcut(
msg.key || "",
msg.code || "",
!!msg.ctrl,
!!msg.meta,
!!msg.alt,
!!msg.shift,
!!msg.mac,
),
});
break;
case "editor-chrome":
if (!handle) throw new Error("no document");
if (self._pendingEditorChrome) {
self._pendingEditorChrome = 0;
handle.setReadOnly(false);
}
handle.loadEditorChrome();
self.postMessage({ type: "editor-chrome", ok: true });
break;
case "editor-key":
if (!handle) throw new Error("no document");
unlockChrome();
self.postMessage({type:"editor-key",result:handle.editorKey(msg.key||"",!!msg.ctrl,!!msg.meta,!!msg.alt,!!msg.shift,!!msg.mac)});
break;
case "original-bytes": {
if (!handle) throw new Error("no document");
const bytes = new Uint8Array(handle.takePackageBytes());
self.postMessage({ type: "original-bytes", bytes }, [bytes.buffer]);
break;
}
case "save":{
if (!handle) throw new Error("no document");
const bytes=new Uint8Array(handle.saveDocument());
self.postMessage({type:"saved",bytes},[bytes.buffer]);
break;
}
case "apply-edit":
if (!handle) throw new Error("no document");
unlockChrome();
self.postMessage({type:"apply-edit",result:handle.applyEdit(msg.spec||"")});
break;
case "dispose": {
self._pendingEditorChrome = 0;
if (handle) {
try {
handle.free();
} catch {
}
handle = null;
}
glue = null;
self.postMessage({ type: "disposed" });
break;
}
default:
self.postMessage({ type: "error", message: `unknown message ${msg.type}` });
}
})
.catch((e) => {
self.postMessage({
type: "error",
code: e && e.code,
message: (e && e.message) || String(e),
});
});
};
