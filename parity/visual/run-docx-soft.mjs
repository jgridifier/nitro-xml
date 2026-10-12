// SPDX-License-Identifier: MIT
/**
 * Focused docx-viewer visual pass for soft corpus fixtures (#23 follow-up).
 * Usage: node parity/visual/run-docx-soft.mjs [substring...]
 * Default: all currently informational real-docx cases (+ optional ids).
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { readyCases } from './registry.mjs';
import { startVisualServer } from './server.mjs';
import { prepareAll } from './prepare.mjs';
import { comparePngBuffers } from './compare.mjs';

const VISUAL = dirname(fileURLToPath(import.meta.url));
const OUT = join(VISUAL, 'out');
const FILTERS = process.argv.slice(2);

const SOFT_DEFAULT = [
	'poi-45690',
	'poi-60158',
	'poi-61745',
	'poi-bug54849',
	'poi-wordwithattachments',
	'poi-bug65649',
	'poi-chartex',
	'poi-delins',
	'poi-drawing',
	'poi-endnotes',
	'poi-form-footnotes',
	'blk-paras-and-tables',
];

function hostUrl(base, c, side) {
	const q = new URLSearchParams({
		side,
		id: c.id,
		fixture: `/repo/${c.fixture}`,
		width: String(c.width || 800),
		height: String(c.height || 600),
	});
	return `${base}/pages/docx-viewer.html?${q}`;
}

async function shot(browser, base, c, side, dpr) {
	const context = await browser.newContext({
		viewport: { width: c.width || 800, height: c.height || 600 },
		deviceScaleFactor: dpr,
	});
	try {
		const page = await context.newPage();
		await page.goto(hostUrl(base, c, side), { waitUntil: 'networkidle', timeout: 180_000 });
		await page.waitForFunction(
			() => {
				const el = document.getElementById('stage');
				return el && (el.dataset.ready === '1' || el.dataset.ready === 'err');
			},
			null,
			{ timeout: 180_000 },
		);
		const ready = await page.evaluate(() => document.getElementById('stage')?.dataset.ready);
		if (ready !== '1') {
			const body = await page.evaluate(() =>
				document.getElementById('stage')?.textContent?.slice(0, 400),
			);
			throw new Error(`host not ready: ${body || page.title()}`);
		}
		return await page.locator('#stage').screenshot({ type: 'png' });
	} finally {
		await context.close();
	}
}

async function printRaster(browser, base, c, side) {
	const context = await browser.newContext({
		viewport: { width: c.width || 800, height: c.height || 600 },
		deviceScaleFactor: 1,
	});
	try {
		const page = await context.newPage();
		await page.goto(hostUrl(base, c, side), { waitUntil: 'networkidle', timeout: 180_000 });
		await page.waitForFunction(
			() => {
				const el = document.getElementById('stage');
				return el && (el.dataset.ready === '1' || el.dataset.ready === 'err');
			},
			null,
			{ timeout: 180_000 },
		);
		const ready = await page.evaluate(() => document.getElementById('stage')?.dataset.ready);
		if (ready !== '1') throw new Error('print host not ready');
		await page.emulateMedia({ media: 'print' });
		const pdf = await page.pdf({
			width: `${c.width || 800}px`,
			height: `${c.height || 600}px`,
			printBackground: true,
			margin: { top: '0', right: '0', bottom: '0', left: '0' },
		});
		const pdfRel = `/out/pdfs/soft-${c.id}-${side}.pdf`;
		writeFileSync(join(OUT, 'pdfs', `soft-${c.id}-${side}.pdf`), pdf);
		await context.close();

		const ctx2 = await browser.newContext({
			viewport: { width: 1200, height: 900 },
			deviceScaleFactor: 1,
		});
		try {
			const p2 = await ctx2.newPage();
			await p2.goto(`${base}/pages/pdf-raster.html?src=${encodeURIComponent(pdfRel)}`, {
				waitUntil: 'networkidle',
			});
			await p2.waitForFunction(
				() => {
					const el = document.getElementById('c');
					return el && (el.dataset.ready === '1' || el.dataset.ready === 'err');
				},
				null,
				{ timeout: 60_000 },
			);
			return await p2.locator('#c').screenshot({ type: 'png' });
		} finally {
			await ctx2.close();
		}
	} catch (e) {
		await context.close().catch(() => {});
		throw e;
	}
}

const needles = FILTERS.length ? FILTERS : SOFT_DEFAULT;
const cases = readyCases().filter(
	(c) => c.kind === 'docx-viewer' && needles.some((n) => c.id.includes(n) || (c.fixture || '').includes(n)),
);

mkdirSync(join(OUT, 'pdfs'), { recursive: true });
mkdirSync(join(OUT, 'diffs'), { recursive: true });
await prepareAll();
const server = await startVisualServer();
const browser = await chromium.launch({ headless: true, args: ['--disable-lcd-text'] });
const rows = [];

for (const c of cases) {
	const thr = c.threshold ?? 0.001;
	const row = { id: c.id, fixture: c.fixture, dpr1: null, dpr2: null, print: null, pass: false, err: null };
	try {
		for (const dpr of [1, 2]) {
			const ts = await shot(browser, server.url, c, 'ts', dpr);
			const rust = await shot(browser, server.url, c, 'rust', dpr);
			const cmp = comparePngBuffers(ts, rust, {
				format: 'docx',
				threshold: thr,
				diffPath: join(OUT, 'diffs', `${c.id}-soft-dpr${dpr}.png`),
			});
			row[`dpr${dpr}`] = cmp.ratio * 100;
		}
		const tsP = await printRaster(browser, server.url, c, 'ts');
		const rustP = await printRaster(browser, server.url, c, 'rust');
		const cmpP = comparePngBuffers(tsP, rustP, {
			format: 'docx',
			threshold: thr,
			diffPath: join(OUT, 'diffs', `${c.id}-soft-print.png`),
		});
		row.print = cmpP.ratio * 100;
		row.pass = [row.dpr1, row.dpr2, row.print].every((v) => v != null && v <= thr * 100);
	} catch (e) {
		row.err = String(e && e.message ? e.message : e).slice(0, 240);
	}
	rows.push(row);
	console.log(JSON.stringify(row));
}

await browser.close();
await server.close();

const measured = rows.filter((r) => !r.err);
const modeStats = (key) => {
	const vals = measured.map((r) => r[key]).filter((v) => v != null).sort((a, b) => a - b);
	if (!vals.length) return { n: 0, pass: 0, median: null, worst: null, worstId: null };
	const mid = vals[Math.floor(vals.length / 2)];
	const worst = vals[vals.length - 1];
	const worstId = measured.find((r) => r[key] === worst)?.id ?? null;
	return {
		n: vals.length,
		pass: vals.filter((v) => v <= 0.1).length,
		median: mid,
		worst,
		worstId,
	};
};

const summary = {
	cases: rows.length,
	errors: rows.filter((r) => r.err).map((r) => r.id),
	dpr1: modeStats('dpr1'),
	dpr2: modeStats('dpr2'),
	print: modeStats('print'),
	allModesPass: rows.filter((r) => r.pass).map((r) => r.id),
	rows,
};
writeFileSync(join(OUT, 'docx-soft-corpus.json'), JSON.stringify(summary, null, 2));
console.log('--- summary ---');
console.log(JSON.stringify({ dpr1: summary.dpr1, dpr2: summary.dpr2, print: summary.print, allModesPass: summary.allModesPass, errors: summary.errors }, null, 2));
