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
if (handle.t !== false || self._openFrameApplied) handle.prepareFrame();
else self._openFrameApplied = true;
let chromeHtml = typeof handle.takeChromeIfDirty==="function" ? handle.takeChromeIfDirty()||null : null;
let measureReqs = null;
let measureCount = 0;
if ((handle.measureCount | 0) > 0) {
const t = handle.takeMeasureReqs();
measureReqs = { fonts: t[0], sizes: t[1], texts: t[2] };
measureCount = measureReqs.fonts.length;
}
let outlineReqs = null, outlineCount = 0, outlineMod = null;
if ((handle.outlineCount | 0) > 0) {
outlineMod = await import("./lazy/outline-worker.js");
({ outlineReqs, outlineCount } = outlineMod.takeOutlineFrame(handle));
}
const exported = handle.exportStrings();
const strings = {};
for (let i = 0; i < exported.length; i++) {
strings[i] = exported[i];
}
let layoutReqs = null;
let layoutCount = 0;
if ((handle.layoutCount | 0) > 0) {
const taken = handle.takeLayoutReqs();
layoutReqs = { nodes: taken[0], kinds: taken[1], props: taken[2] };
layoutCount = layoutReqs.nodes.length;
for (let i = 0; i < layoutCount; i++) {
const p = layoutReqs.props[i];
if (p && strings[p] === undefined) strings[p] = handle.resolve(p) || "";
}
}
const commands = handle.takeFrameCommands();
const transfer = [commands.buffer];
if (measureReqs) {
transfer.push(
measureReqs.fonts.buffer,
measureReqs.sizes.buffer,
measureReqs.texts.buffer,
);
}
if (outlineReqs) outlineMod.transferOutlineBuffers(outlineReqs, transfer);
if (layoutReqs) {
transfer.push(
layoutReqs.nodes.buffer,
layoutReqs.kinds.buffer,
layoutReqs.props.buffer,
);
}
self.postMessage(
{
type: "frame",
h: chromeHtml,
commands,
measureReqs,
measureCount,
outlineReqs,
outlineCount,
layoutReqs,
layoutCount,
strings,
t: handle.t !== false,
},
transfer,
);
if (self._pendingEditorChrome === 1) {
self._pendingEditorChrome = 2;
} else if (self._pendingEditorChrome === 2) {
self._pendingEditorChrome = 0;
await new Promise((r) => setTimeout(r, 0));
try {
handle.setReadOnly(false);
handle.loadEditorChrome();
} catch {
}
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
if (self._pendingEditorChrome && op !== "ui" && op !== "zoom-set") {
self._pendingEditorChrome = 0;
handle.setReadOnly(false);
handle.loadEditorChrome();
}
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
if (self._pendingEditorChrome) {
self._pendingEditorChrome = 0;
handle.setReadOnly(false);
handle.loadEditorChrome();
}
self.postMessage({type:"editor-key",result:handle.editorKey(msg.key||"",!!msg.ctrl,!!msg.meta,!!msg.alt,!!msg.shift,!!msg.mac)});
break;
case "save":{
if (!handle) throw new Error("no document");
const bytes=new Uint8Array(handle.saveDocument());
self.postMessage({type:"saved",bytes},[bytes.buffer]);
break;
}
case "apply-edit":
if (!handle) throw new Error("no document");
if (self._pendingEditorChrome) {
self._pendingEditorChrome = 0;
handle.setReadOnly(false);
handle.loadEditorChrome();
}
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
