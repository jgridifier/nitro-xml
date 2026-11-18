// SPDX-License-Identifier: MIT
/**
 * Excel status-bar Sum/Average/Count gate (issue #540).
 * Runs from tests/selection/run.mjs via runXlsxStatusSumChecks.
 */
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { startServer } from './server.mjs';
import { openDemo, inEl } from './lib.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const sleep = (page, ms) => page.waitForTimeout(ms);

const geom = (page) =>
	inEl(page, 'xlsx', (el, sr) => {
		const m = (s, a, b) => [...sr.querySelectorAll(s)].map((e) => { const r = e.getBoundingClientRect(); return [r[a], r[b]]; });
		return { cols: m('.xg-hdr-col .xg-hd', 'x', 'width'), rows: m('.xg-hdr-row .xg-hd', 'y', 'height') };
	});
const at = (g, c, r) => ({ x: g.cols[c][0] + g.cols[c][1] / 2, y: g.rows[r][0] + g.rows[r][1] / 2 });

const selStatsText = (page) =>
	page.evaluate(() => {
		const s = document.querySelector('[data-sel-stats]');
		return s && !s.hidden ? s.textContent : null;
	});

const until = async (page, fn, ms = 4000) => {
	let v;
	for (let t = 0; t < ms; t += 100) {
		v = await fn();
		if (v) return v;
		await sleep(page, 100);
	}
	return v;
};

/** Prime the editor layout and return settled geometry. */
async function settledGrid(page) {
	const g0 = await geom(page);
	const p = at(g0, 5, 5);
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

const CHECKS = [
	{
		id: 'xlsx.status-bar-sum-average-count',
		// Catches: status bar not showing Sum/Average/Count for a numeric selection (issue #540).
		fn: async (page) => {
			const g = await settledGrid(page);
			// sample.xlsx: B2=4, C2=6, B3=6, C3=9, B4=8, C4=12 → Sum=45, Count=6, Average=7.5
			// Column B = index 1, Column C = index 2; Row 2 = index 1, Row 4 = index 3
			const b2 = at(g, 1, 1), c4 = at(g, 2, 3);
			await page.mouse.move(b2.x, b2.y);
			await page.mouse.down();
			await page.mouse.move(c4.x, c4.y, { steps: 4 });
			await page.mouse.up();
			await sleep(page, 200);
			const got = await until(page, () => selStatsText(page));
			if (!got) return 'status bar [data-sel-stats] did not appear after selecting B2:C4';
			if (!got.includes('Sum: 45')) return `expected Sum: 45 in "${got}"`;
			if (!got.includes('Count: 6')) return `expected Count: 6 in "${got}"`;
			if (!got.includes('Average: 7.5')) return `expected Average: 7.5 in "${got}"`;
			return null;
		},
	},
	{
		id: 'xlsx.status-bar-hidden-for-single-cell',
		// Catches: status bar stats showing for a single-cell selection (should be hidden).
		fn: async (page) => {
			const g = await settledGrid(page);
			const b2 = at(g, 1, 1);
			await page.mouse.click(b2.x, b2.y);
			await sleep(page, 400);
			const got = await selStatsText(page);
			if (got) return `status bar should be hidden for single cell but shows "${got}"`;
			return null;
		},
	},
];

export async function runXlsxStatusSumChecks(browserNames) {
	const pw = await import(pathToFileURL(join(HERE, '../../parity/visual/node_modules/playwright/index.mjs')).href);
	const server = await startServer();
	const rows = [];
	const only = process.env.XLSX_STATUS_ONLY || '';
	try {
		for (const name of browserNames) {
			const browser = await pw[name].launch();
			try {
				for (const c of CHECKS.filter((x) => !only || x.id.includes(only))) {
					const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
					const page = await context.newPage();
					if (process.env.XLSX_STATUS_DEBUG) {
						page.on('console', (m) => console.log('console:', m.text()));
						page.on('pageerror', (e) => console.log('pageerror:', String(e).slice(0, 300)));
					}
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
	const summary = `xlsx status-sum: ${rows.length - bad.length}/${rows.length} ok` + bad.map((r) => `\n  FAIL ${r.case}: ${r.detail}`).join('');
	return { ok: bad.length === 0, rows, summary };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
	const res = await runXlsxStatusSumChecks((process.env.SELECTION_BROWSERS || 'chromium').split(','));
	console.log(res.summary);
	process.exit(res.ok ? 0 : 1);
}
