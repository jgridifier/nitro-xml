// SPDX-License-Identifier: MIT
/**
 * Key-path hygiene gate (run from tests/selection/run.mjs, so it rides the `selection-parity` job).
 * Real mouse + keyboard events with no human delay between them. Each check names the bug it catches.
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

const CHECKS = [
	{
		id: 'keypath.click-then-key-order',
		// Catches: a key pressed right after a click acting on the old selection, or sitting unprocessed because the tick request was dropped while another tick was in flight.
		fn: async (page) => {
			const g = await geom(page);
			for (let i = 0; i < 6; i++) {
				await page.mouse.click(at(g, 2, 2).x, at(g, 2, 2).y);
				await sleep(page, 300);
				const p = at(g, 5, 8);
				await page.mouse.click(p.x, p.y);
				await page.keyboard.press('ArrowDown');
				await sleep(page, 500);
				const s = await selection(page);
				if (s !== 'F10') return `run ${i}: click F9 then immediate ArrowDown -> ${s}`;
			}
			return null;
		},
	},
	{
		id: 'keypath.shift-arrow-burst',
		// Catches: back-to-back Shift+Arrow presses dropping steps (ticks coalesced away while one was in flight).
		fn: async (page) => {
			const g = await geom(page);
			const p = at(g, 1, 1);
			await page.mouse.click(p.x, p.y);
			await sleep(page, 300);
			for (let i = 0; i < 4; i++) await page.keyboard.press('Shift+ArrowDown');
			await sleep(page, 700);
			const s = await selection(page);
			return s === 'B2:B6' ? null : `4x Shift+ArrowDown from B2 -> ${s} (expected B2:B6)`;
		},
	},
	{
		id: 'keypath.click-then-type',
		// Catches: typed text landing in the previously selected cell when typing starts right after a click.
		fn: async (page) => {
			const g = await geom(page);
			const a = at(g, 3, 3);
			await page.mouse.click(a.x, a.y);
			await sleep(page, 300);
			const b = at(g, 6, 6);
			await page.mouse.click(b.x, b.y);
			await page.keyboard.type('kpz');
			await page.keyboard.press('Enter');
			await sleep(page, 700);
			const txt = await inEl(page, 'xlsx', (el, sr) => sr.querySelector('.dom').textContent);
			const sel = await selection(page);
			return /kpz/.test(txt) && sel === 'G8' ? null : `typed after click: dom has kpz=${/kpz/.test(txt)}, selection=${sel} (expected G8)`;
		},
	},
	{
		id: 'keypath.no-layout-shift',
		// Catches: the formula bar mounting late and moving the grid 29 px under a click already in flight.
		fn: async (page) => {
			const ys = [(await geom(page)).rows[0][0]];
			const g = await geom(page);
			const p = at(g, 3, 3);
			await page.mouse.click(p.x, p.y);
			ys.push((await geom(page)).rows[0][0]);
			await page.keyboard.press('ArrowDown');
			await sleep(page, 400);
			ys.push((await geom(page)).rows[0][0]);
			await page.keyboard.type('q');
			await sleep(page, 400);
			ys.push((await geom(page)).rows[0][0]);
			return ys.every((y) => Math.abs(y - ys[0]) < 1) ? null : `grid y moved: ${ys.map((y) => y.toFixed(0)).join(' -> ')}`;
		},
	},
];

export async function runKeyPathChecks(browserNames) {
	const pw = await import(pathToFileURL(join(HERE, '../../parity/visual/node_modules/playwright/index.mjs')).href);
	const server = await startServer();
	const rows = [];
	try {
		for (const name of browserNames) {
			const browser = await pw[name].launch();
			try {
				for (const c of CHECKS) {
					const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
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
	const summary = `key path: ${rows.length - bad.length}/${rows.length} ok` + bad.map((r) => `\n  FAIL ${r.case}: ${r.detail}`).join('');
	return { ok: bad.length === 0, rows, summary };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
	const res = await runKeyPathChecks((process.env.SELECTION_BROWSERS || 'chromium,webkit,firefox').split(','));
	console.log(res.summary);
	process.exit(res.ok ? 0 : 1);
}
