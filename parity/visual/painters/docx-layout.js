// SPDX-License-Identifier: MIT
/**
 * Shared DOM painter for docx LayoutResult (#23 / #45).
 * Same painter for TS- and Rust-sourced IR so pixel diffs isolate layout only.
 */

function px(n) {
	const v = Number(n);
	return Number.isFinite(v) ? v : 0;
}

function paintParagraph(parent, p) {
	if (!p || typeof p !== 'object') return;
	const box = document.createElement('div');
	box.className = 'dve-print-block';
	box.style.position = 'absolute';
	box.style.left = '0';
	box.style.top = `${px(p.yPx)}px`;
	box.style.width = '100%';
	box.style.height = `${px(p.heightPx)}px`;
	for (const line of p.lines || []) {
		const ln = document.createElement('div');
		ln.className = 'dve-print-line';
		ln.style.position = 'absolute';
		ln.style.left = '0';
		ln.style.top = `${px(line.yPx)}px`;
		ln.style.height = `${px(line.heightPx)}px`;
		ln.style.width = '100%';
		ln.style.whiteSpace = 'pre';
		for (const f of line.fragments || []) {
			if (!f) continue;
			if (f.object) {
				const pic = document.createElement('div');
				pic.className = 'dve-print-picture';
				pic.style.position = 'absolute';
				pic.style.left = `${px(f.xPx)}px`;
				pic.style.top = `${px(f.topPx)}px`;
				pic.style.width = `${px(f.object.widthPx ?? f.widthPx)}px`;
				pic.style.height = `${px(f.object.heightPx ?? line.heightPx)}px`;
				pic.style.background = '#dde3ea';
				ln.appendChild(pic);
				continue;
			}
			if (!f.text && !px(f.widthPx)) continue;
			const sp = document.createElement('span');
			sp.style.position = 'absolute';
			sp.style.left = `${px(f.xPx)}px`;
			sp.style.whiteSpace = 'pre';
			const size = px(f.fontSizePt) || 11;
			const family = f.fontFamily || 'Calibri, sans-serif';
			sp.style.font = `${f.bold ? '700 ' : ''}${f.italic ? 'italic ' : ''}${size}pt ${family}`;
			if (f.color) sp.style.color = String(f.color).startsWith('#') ? f.color : `#${f.color}`;
			if (f.underline) sp.style.textDecoration = 'underline';
			sp.textContent = f.text || '';
			ln.appendChild(sp);
		}
		box.appendChild(ln);
	}
	parent.appendChild(box);
}

function paintTable(parent, table) {
	const box = document.createElement('div');
	box.className = 'dve-print-block dve-print-table';
	box.style.position = 'absolute';
	box.style.left = table.xPx ? `${px(table.xPx)}px` : '0';
	box.style.top = `${px(table.yPx)}px`;
	box.style.height = `${px(table.heightPx)}px`;
	for (const row of table.rows || []) {
		const rowEl = document.createElement('div');
		rowEl.className = 'dve-print-row';
		rowEl.style.position = 'absolute';
		rowEl.style.left = '0';
		rowEl.style.top = `${px(row.yPx)}px`;
		rowEl.style.height = `${px(row.heightPx)}px`;
		rowEl.style.width = '100%';
		const cells = row.cells || [];
		const geom = row.geometry || [];
		cells.forEach((paras, i) => {
			const g = geom[i] || {};
			const cell = document.createElement('div');
			cell.className = 'dve-print-cell';
			cell.style.position = 'absolute';
			cell.style.left = `${px(g.xPx)}px`;
			cell.style.width = `${px(g.widthPx) || 40}px`;
			cell.style.top = '0';
			cell.style.height = '100%';
			cell.style.overflow = 'hidden';
			for (const para of paras || []) paintParagraph(cell, para);
			rowEl.appendChild(cell);
		});
		box.appendChild(rowEl);
	}
	parent.appendChild(box);
}

function paintBlock(parent, block) {
	if (!block || typeof block !== 'object') return;
	if (block.kind === 'table') paintTable(parent, block);
	else paintParagraph(parent, block);
}

/**
 * @param {HTMLElement} stage
 * @param {{ pages?: object[] }} ir
 */
export function paintDocxLayout(stage, ir) {
	stage.replaceChildren();
	const root = document.createElement('div');
	root.className = 'dve-print-pages';
	root.style.background = '#fff';
	for (const page of ir.pages || []) {
		const el = document.createElement('div');
		el.className = 'dve-print-page';
		el.style.position = 'relative';
		el.style.background = '#fff';
		el.style.width = `${px(page.widthPx)}px`;
		el.style.height = `${px(page.heightPx)}px`;
		el.style.overflow = 'hidden';
		const contentH = px(page.heightPx) - px(page.marginTopPx) - px(page.marginBottomPx);
		for (const col of page.columns || []) {
			const c = document.createElement('div');
			c.className = 'dve-print-column';
			c.style.position = 'absolute';
			c.style.left = `${px(page.marginLeftPx) + px(col.xPx)}px`;
			c.style.top = `${px(page.marginTopPx)}px`;
			c.style.width = `${px(col.widthPx)}px`;
			c.style.height = `${contentH}px`;
			for (const block of col.blocks || []) paintBlock(c, block);
			el.appendChild(c);
		}
		const notes = page.footnotes || [];
		if (notes.length) {
			const band = document.createElement('div');
			band.className = 'dve-print-footnotes';
			band.style.position = 'absolute';
			band.style.left = `${px(page.marginLeftPx)}px`;
			band.style.width = `${px(page.widthPx) - px(page.marginLeftPx) - px(page.marginRightPx)}px`;
			for (const note of notes) {
				const n = document.createElement('div');
				n.style.position = 'absolute';
				n.style.top = `${px(note.yPx)}px`;
				n.style.left = '0';
				n.style.right = '0';
				for (const para of note.paragraphs || []) paintParagraph(n, para);
				band.appendChild(n);
			}
			el.appendChild(band);
		}
		for (const fl of page.floats || []) {
			const f = document.createElement('div');
			f.className = 'dve-print-float';
			f.style.position = 'absolute';
			f.style.left = `${px(fl.xPx)}px`;
			f.style.top = `${px(fl.yPx)}px`;
			f.style.width = `${px(fl.widthPx)}px`;
			f.style.height = `${px(fl.heightPx)}px`;
			f.style.background = '#dde3ea';
			el.appendChild(f);
		}
		root.appendChild(el);
	}
	stage.appendChild(root);
}
