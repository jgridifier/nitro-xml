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
