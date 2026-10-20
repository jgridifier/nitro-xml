// SPDX-License-Identifier: MIT
import { scheduleMetafileAttr } from "./metafile-resolve.js";

export const Op = Object.freeze({
FillRect: 1,
StrokeRect: 2,
DrawImage: 3,
DrawText: 4,
CreateElement: 5,
SetAttr: 6,
AppendChild: 7,
SetText: 8,
SetStyle: 9,
SetInnerMarkup: 10,
PlaceBox: 11,
});
export function applyCommandBuffer(bytes, target) {
const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
const nodes = target.nodes || (target.nodes = new Map());
const images = target.images || (target.images = new Map());
const ctx = target.canvas || null;
const root = target.root || null;
if (root) { root.replaceChildren(); nodes.clear(); }
let i = 0;
let ops = 0;
const need = (n) => i + n <= bytes.byteLength;
while (i < bytes.byteLength) {
const tag = bytes[i++];
switch (tag) {
case Op.FillRect: {
if (!need(20)) return { ops, error: "truncated FillRect" };
const x = view.getInt32(i, true);
const y = view.getInt32(i + 4, true);
const w = view.getInt32(i + 8, true);
const h = view.getInt32(i + 12, true);
const color = view.getUint32(i + 16, true);
i += 20;
if (ctx) {
ctx.fillStyle = cssColor(color);
ctx.fillRect(x, y, w, h);
}
ops++;
break;
}
case Op.StrokeRect: {
if (!need(24)) return { ops, error: "truncated StrokeRect" };
const x = view.getInt32(i, true);
const y = view.getInt32(i + 4, true);
const w = view.getInt32(i + 8, true);
const h = view.getInt32(i + 12, true);
const color = view.getUint32(i + 16, true);
const line = view.getFloat32(i + 20, true);
i += 24;
if (ctx) {
ctx.strokeStyle = cssColor(color);
ctx.lineWidth = line;
ctx.strokeRect(x, y, w, h);
}
ops++;
break;
}
case Op.DrawImage: {
if (!need(20)) return { ops, error: "truncated DrawImage" };
const imgId = view.getUint32(i, true);
const x = view.getInt32(i + 4, true);
const y = view.getInt32(i + 8, true);
const w = view.getInt32(i + 12, true);
const h = view.getInt32(i + 16, true);
i += 20;
const img = images.get(imgId);
if (ctx && img) ctx.drawImage(img, x, y, w, h);
ops++;
break;
}
case Op.DrawText: {
if (!need(24)) return { ops, error: "truncated DrawText" };
const x = view.getInt32(i, true);
const y = view.getInt32(i + 4, true);
const fontId = view.getUint32(i + 8, true);
const size = view.getFloat32(i + 12, true);
const textId = view.getUint32(i + 16, true);
const color = view.getUint32(i + 20, true);
i += 24;
if (ctx) {
const font = target.resolve(fontId) || "sans-serif";
const text = target.resolve(textId) || "";
ctx.font = `${size}px ${font}`;
ctx.fillStyle = cssColor(color);
ctx.fillText(text, x, y);
}
ops++;
break;
}
case Op.CreateElement: {
if (!need(12)) return { ops, error: "truncated CreateElement" };
const id = view.getUint32(i, true);
const tagId = view.getUint32(i + 4, true);
const parentId = view.getUint32(i + 8, true);
i += 12;
if (typeof document !== "undefined") {
const tagName = target.resolve(tagId) || "div";
const el = tagName==="svg"?document.createElementNS("http://www.w3.org/2000/svg",tagName):document.createElement(tagName);
nodes.set(id, el);
const parent = parentId === 0 ? root : nodes.get(parentId);
if (parent) parent.appendChild(el);
}
ops++;
break;
}
case Op.SetAttr: {
if (!need(12)) return { ops, error: "truncated SetAttr" };
const id = view.getUint32(i, true);
const keyId = view.getUint32(i + 4, true);
const valId = view.getUint32(i + 8, true);
i += 12;
const el = nodes.get(id);
if (el) {
const key = target.resolve(keyId);
const val = target.resolve(valId);
if (key != null) {
el.setAttribute(key, val ?? "");
if (key === "src") scheduleMetafileAttr(el, val ?? "");
}
}
ops++;
break;
}
case Op.AppendChild: {
if (!need(8)) return { ops, error: "truncated AppendChild" };
const parentId = view.getUint32(i, true);
const childId = view.getUint32(i + 4, true);
i += 8;
const parent = parentId === 0 ? root : nodes.get(parentId);
const child = nodes.get(childId);
if (parent && child) parent.appendChild(child);
ops++;
break;
}
case Op.SetText: {
if (!need(8)) return { ops, error: "truncated SetText" };
const id = view.getUint32(i, true);
const textId = view.getUint32(i + 4, true);
i += 8;
const el = nodes.get(id);
if (el) {
const text = target.resolve(textId) || "";
el.textContent = text;
}
ops++;
break;
}
case Op.SetStyle: {
if (!need(12)) return { ops, error: "truncated SetStyle" };
const id = view.getUint32(i, true);
const propId = view.getUint32(i + 4, true);
const valId = view.getUint32(i + 8, true);
i += 12;
const el = nodes.get(id);
if (el) {
const prop = target.resolve(propId);
const val = target.resolve(valId);
if (prop != null) el.style[prop] = val ?? "";
}
ops++;
break;
}
case Op.SetInnerMarkup: {
if (!need(8)) return { ops, error: "truncated SetInnerMarkup" };
const parentId = view.getUint32(i, true);
const markupId = view.getUint32(i + 4, true);
i += 8;
const parent = parentId === 0 ? root : nodes.get(parentId);
const markup = target.resolve(markupId);
if (parent && markup && typeof document !== "undefined") {
const tpl = document.createElementNS("http://www.w3.org/2000/svg", "svg");
tpl.innerHTML = markup;
while (tpl.firstChild) parent.appendChild(tpl.firstChild);
}
ops++;
break;
}
case Op.PlaceBox: {
if (!need(20)) return { ops, error: "truncated PlaceBox" };
const id = view.getUint32(i, true);
const x = view.getInt32(i + 4, true);
const y = view.getInt32(i + 8, true);
const w = view.getInt32(i + 12, true);
const h = view.getInt32(i + 16, true);
i += 20;
const el = nodes.get(id);
if (el) {
const s = el.style;
s.left = x + "px";
s.top = y + "px";
s.width = w + "px";
s.height = h + "px";
}
ops++;
break;
}
default:
return { ops, error: `unknown opcode ${tag}` };
}
}
return { ops };
}
export function cssColor(argb) {
const a = ((argb >>> 24) & 0xff) / 255;
const r = (argb >>> 16) & 0xff;
const g = (argb >>> 8) & 0xff;
const b = argb & 0xff;
return a >= 1 ? `rgb(${r}, ${g}, ${b})` : `rgba(${r}, ${g}, ${b}, ${a})`;
}
