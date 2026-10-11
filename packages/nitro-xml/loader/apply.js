// SPDX-License-Identifier: MIT
/** Per-frame command-buffer applier: one pass, no per-element WASM→JS calls. */

export const Op = Object.freeze({
  FillRect: 1,
  StrokeRect: 2,
  DrawImage: 3,
  DrawText: 4,
  CreateElement: 5,
  SetAttr: 6,
  AppendChild: 7,
});

/**
 * @typedef {{
 *   canvas?: CanvasRenderingContext2D | null,
 *   root?: Element | null,
 *   resolve: (id: number) => string | undefined,
 *   images?: Map<number, CanvasImageSource>,
 *   nodes?: Map<number, Element>,
 * }} ApplyTarget
 */

/**
 * Apply a seed command buffer in a single pass.
 * Returns `{ops, error?}` — on truncated/unknown opcodes, stops without throwing
 * so a bad frame cannot take down the host.
 *
 * @param {Uint8Array} bytes
 * @param {ApplyTarget} target
 */
export function applyCommandBuffer(bytes, target) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const nodes = target.nodes || (target.nodes = new Map());
  const images = target.images || (target.images = new Map());
  const ctx = target.canvas || null;
  const root = target.root || null;
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
          const el = document.createElement(tagName);
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
          if (key != null) el.setAttribute(key, val ?? "");
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
      default:
        return { ops, error: `unknown opcode ${tag}` };
    }
  }
  return { ops };
}

/** @param {number} argb 0xAARRGGBB */
export function cssColor(argb) {
  const a = ((argb >>> 24) & 0xff) / 255;
  const r = (argb >>> 16) & 0xff;
  const g = (argb >>> 8) & 0xff;
  const b = argb & 0xff;
  return a >= 1 ? `rgb(${r}, ${g}, ${b})` : `rgba(${r}, ${g}, ${b}, ${a})`;
}

/** Encode helpers for tests / demos (mirrors Rust CommandBuffer writers). */
export function encodeFillRect(x, y, w, h, color) {
  const u8 = new Uint8Array(21);
  const v = new DataView(u8.buffer);
  u8[0] = Op.FillRect;
  v.setInt32(1, x, true);
  v.setInt32(5, y, true);
  v.setInt32(9, w, true);
  v.setInt32(13, h, true);
  v.setUint32(17, color, true);
  return u8;
}

export function encodeStrokeRect(x, y, w, h, color, line) {
  const u8 = new Uint8Array(25);
  const v = new DataView(u8.buffer);
  u8[0] = Op.StrokeRect;
  v.setInt32(1, x, true);
  v.setInt32(5, y, true);
  v.setInt32(9, w, true);
  v.setInt32(13, h, true);
  v.setUint32(17, color, true);
  v.setFloat32(21, line, true);
  return u8;
}
