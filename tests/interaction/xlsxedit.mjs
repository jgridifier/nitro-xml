// SPDX-License-Identifier: MIT
/**
 * Excel editor interaction gate (run from tests/selection/run.mjs, so it rides the `selection-parity` job).
 * Real mouse, keyboard and ribbon-button events against the full xlsx demo. Each check names the bug it catches.
 */
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { startServer } from './server.mjs';
import { openDemo, inEl, uiState } from './lib.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const sleep = (page, ms) => page.waitForTimeout(ms);
const geom = (page) =>
	inEl(page, 'xlsx', (el, sr) => {
		const m = (s, a, b) => [...sr.querySelectorAll(s)].map((e) => { const r = e.getBoundingClientRect(); return [r[a], r[b]]; });
		return { cols: m('.xg-hdr-col .xg-hd', 'x', 'width'), rows: m('.xg-hdr-row .xg-hd', 'y', 'height') };
	});
const at = (g, c, r) => ({ x: g.cols[c][0] + g.cols[c][1] / 2, y: g.rows[r][0] + g.rows[r][1] / 2 });
const selection = async (page) => (await uiState(page, 'xlsx')).selection;
const text = (page, p) =>
	inEl(page, 'xlsx', (el, sr, q) => sr.elementsFromPoint(q.x, q.y).filter((e) => e.classList.contains('xg-c') || e.classList.contains('xg-tx')).map((e) => e.textContent).join('|'), p);
const until = async (page, fn, ms = 3000) => {
	let v;
	for (let t = 0; t < ms; t += 100) {
		v = await fn();
		if (v) return v;
		await sleep(page, 100);
	}
	return v;
};

/** The first keystroke mounts the formula bar and shifts the grid; prime it, then return the settled geometry. */
async function settledGrid(page) {
	const g0 = await geom(page);
	const p = at(g0, 12, 10);
	await page.mouse.click(p.x, p.y);
	await page.keyboard.press('ArrowDown');
	let last = null, same = 0;
	for (let i = 0; i < 40 && same < 4; i++) {
		await sleep(page, 150);
		const y = (await geom(page)).rows[0]?.[0];
		same = last === y ? same + 1 : 0;
		last = y;
	}
	return geom(page);
}
const ribbon = (page, name) => page.getByRole('button', { name, exact: true }).first();

const CHECKS = [
	{
		id: 'xlsx.ribbon-insert-delete-rows-columns',
		// Catches: Insert/Delete rows and columns missing from the Excel ribbon (or dead), or acting on one line when several are selected.
		fn: async (page) => {
			const g = await settledGrid(page);
			const a3 = at(g, 0, 2), a4 = at(g, 0, 3), a5 = at(g, 0, 4), c3 = at(g, 2, 2), d3 = at(g, 3, 2);
			const [t3, t4] = [await text(page, a3), await text(page, a4)];
			if (!t3 || !t4 || t3 === t4) return `fixture cells not distinct: "${t3}" "${t4}"`;
			// Select A3:A4 (two rows), insert -> two blank rows, old A3 lands on A5.
			await page.mouse.move(a3.x, a3.y);
			await page.mouse.down();
			await page.mouse.move(a4.x, a4.y, { steps: 4 });
			await page.mouse.up();
			await sleep(page, 200);
			await ribbon(page, 'Insert sheet rows above the selection').click();
			if (!(await until(page, async () => (await text(page, a5)) === t3))) return `after Insert rows A5="${await text(page, a5)}", want "${t3}"`;
			if (await text(page, a3)) return `A3 not blank after inserting two rows: "${await text(page, a3)}"`;
			await ribbon(page, 'Delete the selected sheet rows').click();
			if (!(await until(page, async () => (await text(page, a3)) === t3))) return `after Delete rows A3="${await text(page, a3)}", want "${t3}"`;
			// Columns: select C3:D3, insert two columns, old C3 lands in E3.
			const e3 = at(g, 4, 2);
			const [tc, td] = [await text(page, c3), await text(page, d3)];
			await page.mouse.move(c3.x, c3.y);
			await page.mouse.down();
			await page.mouse.move(d3.x, d3.y, { steps: 4 });
			await page.mouse.up();
			await sleep(page, 200);
			await ribbon(page, 'Insert sheet columns left of the selection').click();
			if (!(await until(page, async () => (await text(page, e3)) === tc))) return `after Insert columns E3="${await text(page, e3)}", want "${tc}"`;
			await ribbon(page, 'Delete the selected sheet columns').click();
			if (!(await until(page, async () => (await text(page, c3)) === tc && (await text(page, d3)) === td))) return `after Delete columns C3="${await text(page, c3)}" D3="${await text(page, d3)}", want "${tc}" "${td}"`;
			return null;
		},
	},
	{
		id: 'xlsx.tab-navigation',
		// Catches: Tab not moving the active cell, WebKit focus leaving the grid on Tab (every later key dead), Enter after a Tab run not returning to the start column, Ctrl+Home doing nothing.
		fn: async (page) => {
			const g = await settledGrid(page);
			const p = at(g, 1, 1);
			await page.mouse.click(p.x, p.y);
			await sleep(page, 200);
			const seq = [];
			for (const k of ['Tab', 'Tab', 'Shift+Tab', 'ArrowDown', 'ArrowRight']) {
				const prev = await selection(page);
				await page.keyboard.press(k);
				await until(page, async () => (await selection(page)) !== prev, 1500);
				seq.push(await selection(page));
			}
			const want = ['C2', 'D2', 'C2', 'C3', 'D3'];
			if (seq.join() !== want.join()) return `got ${seq.join(' ')}; want ${want.join(' ')}`;
			// Type across a row with Tab, then Enter returns to the first column of the run.
			const q = at(g, 6, 19);
			await page.mouse.click(q.x, q.y);
			await sleep(page, 200);
			await page.keyboard.type('a'); await page.keyboard.press('Tab');
			await page.keyboard.type('b'); await page.keyboard.press('Tab');
			await page.keyboard.type('c'); await page.keyboard.press('Enter');
			if (!(await until(page, async () => (await selection(page)) === 'G21'))) return `after a,Tab,b,Tab,c,Enter selection=${await selection(page)}, want G21`;
			await sleep(page, 300);
			// Row 21 may have scrolled into view; read the cell back from the current geometry.
			const g2 = await geom(page);
			const rowIdx = await inEl(page, 'xlsx', (el, sr) => [...sr.querySelectorAll('.xg-hdr-row .xg-hd')].findIndex((e) => e.textContent.trim() === '20'));
			const h20 = rowIdx < 0 ? null : await text(page, at(g2, 7, rowIdx));
			if (h20 !== 'b') return `H20 should hold "b", has ${JSON.stringify(h20)}`;
			await page.keyboard.press('Control+Home');
			if (!(await until(page, async () => (await selection(page)) === 'A1'))) return `Ctrl+Home selection=${await selection(page)}`;
			return null;
		},
	},
];

export async function runXlsxEditChecks(browserNames) {
	const pw = await import(pathToFileURL(join(HERE, '../../parity/visual/node_modules/playwright/index.mjs')).href);
	const server = await startServer();
	const rows = [];
	const only = process.env.XLSXEDIT_ONLY || '';
	try {
		for (const name of browserNames) {
			const browser = await pw[name].launch();
			try {
				for (const c of CHECKS.filter((x) => x.id.includes(only))) {
					const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
					if (name === 'chromium') await context.grantPermissions(['clipboard-read', 'clipboard-write']);
					const page = await context.newPage();
					let msg;
					try {
						await openDemo(page, server.url, 'xlsx');
						msg = await c.fn(page);
					} catch (e) {
						msg = `error: ${String(e.message || e).split('\n')[0]}`;
					}
					rows.push({ case: `${c.id}@${name}`, ok: !msg, detail: msg || '' });
					await context.close();
				}
			} finally {
				await browser.close();
			}
		}
	} finally {
		server.close();
	}
	const bad = rows.filter((r) => !r.ok);
	const summary = `xlsx edit: ${rows.length - bad.length}/${rows.length} ok` + bad.map((r) => `\n  FAIL ${r.case}: ${r.detail}`).join('');
	return { ok: bad.length === 0, rows, summary };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
	const res = await runXlsxEditChecks((process.env.SELECTION_BROWSERS || 'chromium,webkit,firefox').split(','));
	console.log(res.summary);
	process.exit(res.ok ? 0 : 1);
}
