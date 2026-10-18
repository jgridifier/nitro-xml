// SPDX-License-Identifier: MIT
import { initCore, getWasmMemory, unsupportedFormatError } from "./bootstrap.js";
import { isSupportedExtension } from "./formats.js";
import { applyCommandBuffer } from "./apply.js";
import { createEventCoalescer } from "./events.js";
import { flushMeasureBatch } from "./measure.js";
import { flushOutlineBatch } from "./glyph-outline.js";
import { applyFontSourceToHandle } from "./font-source.js";
import { flushLayoutReadBatch, packLayoutReqs } from "./layout-readback.js";
const registry =
typeof FinalizationRegistry !== "undefined"
? new FinalizationRegistry((free) => {
try {
free();
} catch {
}
})
: null;
export async function openDocument(bytes, opts = {}) {
if (opts.name && !isSupportedExtension(opts.name)) {
throw unsupportedFormatError();
}
const { glue, memory } = await initCore();
const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
const handle = new glue.Handle(u8);
applyFontSourceToHandle(handle, opts.fontSource);
if (opts.name) {
try {
handle.openDocument(opts.name);
} catch {
}
}
return new DocumentSession(handle, {
memory: memory || getWasmMemory(),
canvas: opts.canvas || null,
root: opts.root || null,
name: opts.name || "",
});
}
export class DocumentSession {
constructor(handle, opts) {
this._handle = handle;
this._memory = opts.memory;
this._canvas = opts.canvas;
this._root = opts.root;
this._name = opts.name;
this._nodes = new Map();
this._images = new Map();
this._disposed = false;
this._measureCtx = null;
this._pendingLayoutReqs = null;
const h = handle;
this._events = createEventCoalescer((batch) => {
if (this._disposed) return;
h.pushEventBatch(batch);
});
if (registry) {
const free = () => {
try {
h.free();
} catch {
}
};
this._registryToken = {};
registry.register(this, free, this._registryToken);
}
}
get handle() {
return this._handle;
}
get name() {
return this._name;
}
/** Pending canvas measure probes (for settle loops). */
get measureCount() {
return this._disposed ? 0 : this._handle.measureCount | 0;
}
/** Pending glyph-outline probes (for settle loops). */
get outlineCount() {
return this._disposed ? 0 : this._handle.outlineCount | 0;
}
pushEvent(ev) {
this._events.push(ev);
}
shellAction(op, arg = "") {
return this._disposed ? null : JSON.parse(this._handle.shellAction(op || "ui", arg || ""));
}
tick(opts = {}) {
  if (this._disposed) return { measured: 0, outlined: 0, ops: 0, layoutReads: 0 };
  if (opts.canvas !== undefined) this._canvas = opts.canvas;
  if (opts.root !== undefined) this._root = opts.root;
  const layoutReads = this._flushPendingLayout();
  const ctx = this._ensureMeasureCtx();
  const handle = this._measureHandle(ctx);
  const measured = flushMeasureBatch(handle, ctx);
  const outlined = flushOutlineBatch(this._handle, ctx);
  this._handle.prepareFrame();
  const commands = this._handle.takeFrameCommands();
  const drawCtx = this._canvas && this._canvas.getContext
    ? this._canvas.getContext("2d")
    : null;
  const { ops, error } = applyCommandBuffer(commands, {
    canvas: drawCtx,
    root: this._root,
    resolve: (id) => this._handle.resolve(id),
    images: this._images,
    nodes: this._nodes,
  });
  this._takeLayoutAfterApply();
  this._events.flushNow();
  return { measured, outlined, ops, layoutReads, error };
}
dispose() {
if (this._disposed) return;
this._disposed = true;
this._events.flushNow();
if (registry && this._registryToken) registry.unregister(this._registryToken);
try {
this._handle.free();
} catch {
}
}
_flushPendingLayout() {
const reqs = this._pendingLayoutReqs;
this._pendingLayoutReqs = null;
if (!reqs || reqs.length === 0) return 0;
return flushLayoutReadBatch(
{
layoutCount: reqs.length,
resolve: (id) => this._handle.resolve(id),
takeLayoutReqs: () => reqs,
setLayoutResults: (values) => this._handle.setLayoutResults(values),
},
this._nodes,
);
}
_takeLayoutAfterApply() {
const h = this._handle;
if ((h.layoutCount | 0) === 0) {
this._pendingLayoutReqs = null;
return;
}
const taken = h.takeLayoutReqs();
this._pendingLayoutReqs = packLayoutReqs(taken[0], taken[1], taken[2]);
}
_ensureMeasureCtx() {
if (typeof document === "undefined") {
return this._canvas && this._canvas.getContext ? this._canvas.getContext("2d") : null;
}
if (!this._measureCtx) {
const c = document.createElement("canvas");
c.width = 1;
c.height = 1;
this._measureCtx = c.getContext("2d");
}
return this._measureCtx;
}
_measureHandle(ctx) {
const h = this._handle;
return {
get measureCount() {
return h.measureCount;
},
get outlineCount() {
return h.outlineCount;
},
resolve: (id) => h.resolve(id),
setMeasureResults: (widths) => h.setMeasureResults(widths),
takeMeasureReqs: () => h.takeMeasureReqs(),
takeOutlineReqs: () => h.takeOutlineReqs(),
setOutlineResults: (contours) => h.setOutlineResults(contours),
_ctx: ctx,
};
}
}
