// SPDX-License-Identifier: MIT
export const EVENT_VERSION = 1;
export const EventKind = Object.freeze({
Input: 1,
Pointer: 2,
Wheel: 3,
Keyboard: 4,
Composition: 5,
Resize: 6,
});
export function encodeEventBatch(events) {
let size = 4;
for (const ev of events) size += eventByteLength(ev);
const buf = new ArrayBuffer(size);
const view = new DataView(buf);
const u8 = new Uint8Array(buf);
view.setUint16(0, EVENT_VERSION, true);
view.setUint16(2, events.length, true);
let o = 4;
for (const ev of events) {
u8[o++] = ev.kind;
u8[o++] = 0;
view.setUint16(o, 0, true);
o += 2;
view.setFloat64(o, ev.time_ms, true);
o += 8;
switch (ev.kind) {
case EventKind.Input:
view.setUint32(o, ev.text_id >>> 0, true);
o += 4;
break;
case EventKind.Pointer:
view.setFloat32(o, ev.x, true);
o += 4;
view.setFloat32(o, ev.y, true);
o += 4;
u8[o++] = ev.type & 0xff;
u8[o++] = ev.button & 0xff;
view.setUint16(o, ev.buttons & 0xffff, true);
o += 2;
break;
case EventKind.Wheel:
view.setFloat32(o, ev.x, true);
o += 4;
view.setFloat32(o, ev.y, true);
o += 4;
view.setFloat32(o, ev.dx, true);
o += 4;
view.setFloat32(o, ev.dy, true);
o += 4;
break;
case EventKind.Keyboard:
view.setUint32(o, ev.key_id >>> 0, true);
o += 4;
view.setUint32(o, ev.code_id >>> 0, true);
o += 4;
u8[o++] = ev.type & 0xff;
u8[o++] = ev.mods & 0xff;
view.setUint16(o, 0, true);
o += 2;
break;
case EventKind.Composition:
u8[o++] = ev.type & 0xff;
u8[o++] = 0;
view.setUint16(o, 0, true);
o += 2;
view.setUint32(o, ev.text_id >>> 0, true);
o += 4;
break;
case EventKind.Resize:
view.setFloat32(o, ev.width, true);
o += 4;
view.setFloat32(o, ev.height, true);
o += 4;
view.setFloat32(o, ev.dpr, true);
o += 4;
break;
default:
throw new Error(`unknown event kind ${ev.kind}`);
}
}
return u8;
}
function eventByteLength(ev) {
switch (ev.kind) {
case EventKind.Input:
return 12 + 4;
case EventKind.Pointer:
return 12 + 12;
case EventKind.Wheel:
return 12 + 16;
case EventKind.Keyboard:
return 12 + 12;
case EventKind.Composition:
return 12 + 8;
case EventKind.Resize:
return 12 + 12;
default:
throw new Error(`unknown event kind ${ev.kind}`);
}
}
export function createEventCoalescer(flush) {
let pending = [];
let raf = 0;
function schedule() {
if (raf || typeof requestAnimationFrame !== "function") return;
raf = requestAnimationFrame(() => {
raf = 0;
if (!pending.length) return;
const batch = pending;
pending = [];
flush(encodeEventBatch(batch));
});
}
return {
push(ev) {
if (ev.kind === EventKind.Resize) {
pending = pending.filter((e) => e.kind !== EventKind.Resize);
pending.push(ev);
} else if (ev.kind === EventKind.Pointer && ev.type === 1 ) {
const i = pending.findIndex((e) => e.kind === EventKind.Pointer && e.type === 1);
if (i >= 0) pending[i] = ev;
else pending.push(ev);
} else {
pending.push(ev);
}
if (typeof requestAnimationFrame === "function") schedule();
else {
const batch = pending;
pending = [];
flush(encodeEventBatch(batch));
}
},
flushNow() {
if (raf && typeof cancelAnimationFrame === "function") cancelAnimationFrame(raf);
raf = 0;
if (!pending.length) return null;
const batch = pending;
pending = [];
const bytes = encodeEventBatch(batch);
flush(bytes);
return bytes;
},
get length() {
return pending.length;
},
};
}
