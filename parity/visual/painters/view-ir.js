// SPDX-License-Identifier: MIT
/**
 * Deterministic canvas painter for xlsx CellItem view-IR (#45).
 * Same painter for TS- and Rust-sourced IR so pixel diffs isolate IR/layout only.
 */

/**
 * @param {HTMLCanvasElement} canvas
 * @param {{ sheets?: { items: object[] }[] }} ir
 * @param {{ width: number, height: number, scrollX?: number, scrollY?: number }} viewport
 */
export function paintViewIr(canvas, ir, viewport) {
	const dpr = window.devicePixelRatio || 1;
	const w = viewport.width;
	const h = viewport.height;
	canvas.style.width = `${w}px`;
	canvas.style.height = `${h}px`;
	canvas.width = Math.round(w * dpr);
	canvas.height = Math.round(h * dpr);
	const ctx = canvas.getContext('2d');
	if (!ctx) throw new Error('2d context');
	ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
	ctx.fillStyle = '#ffffff';
	ctx.fillRect(0, 0, w, h);

	const sheet = ir.sheets?.[0];
	if (!sheet) return;
	const scrollX = viewport.scrollX || 0;
	const scrollY = viewport.scrollY || 0;

	for (const item of sheet.items || []) {
		const x = item.x - scrollX;
		const y = item.y - scrollY;
		const iw = item.w;
		const ih = item.h;
		if (x + iw < 0 || y + ih < 0 || x > w || y > h) continue;
		const view = item.view || {};
		if (view.fill) {
			ctx.fillStyle = view.fill;
			ctx.fillRect(x, y, iw, ih);
		}
		const text = view.text || item.text || '';
		if (!text) continue;
		const font = view.font || {};
		const size = font.sizePx || 11;
		const weight = font.bold ? 'bold' : 'normal';
		const style = font.italic ? 'italic' : 'normal';
		const family = font.family || 'Arial';
		ctx.font = `${style} ${weight} ${size}px "${family}", Arial, sans-serif`;
		ctx.fillStyle = font.color || '#000000';
		ctx.textBaseline = 'alphabetic';
		const tx = x + (item.textLeft || 0) + 2;
		const ty = y + ih - 3;
		ctx.save();
		ctx.beginPath();
		ctx.rect(x, y, iw, ih);
		ctx.clip();
		ctx.fillText(text, tx, ty);
		ctx.restore();
	}
}