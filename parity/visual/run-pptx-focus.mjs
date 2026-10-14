// SPDX-License-Identifier: MIT
/** Focused pptx visual measurement for a fixture id list. */
import { mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { pptxVisualCorpus } from './pptx-corpus.mjs';
import { startVisualServer } from './server.mjs';
import { prepareAll } from './prepare.mjs';
import { comparePngBuffers } from './compare.mjs';
import { THRESHOLDS } from './thresholds.mjs';

const VISUAL = dirname(fileURLToPath(import.meta.url));
const OUT = join(VISUAL, 'out');
const ids = process.argv.slice(2);
if (!ids.length) {
	console.error('usage: bun run-pptx-focus.mjs <manifest-id> ...');
	process.exit(2);
}

function hostUrl(base, c, side) {
	const q = new URLSearchParams({
		side,
		id: c.id,
		fixture: `/repo/${c.fixture}`,
		width: String(c.width || 960),
		height: String(c.height || 540),
		slide: String(c.slide ?? 0),
	});
	return `${base}/pages/pptx-viewer.html?${q}`;
}

async function shot(browser, base, c, side, dpr) {
	const context = await browser.newContext({
		viewport: { width: c.width || 960, height: c.height || 540 },
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
		if (side === 'rust') {
			for (let c = 0; c < 32; c++) {
				await page.evaluate(async () => {
					const el = document.getElementById('stage')?.querySelector('nitro-pptx');
					if (el?.document) await el.document.tick();
				});
				await page.waitForTimeout(50);
			}
		}
		return await page.locator('#stage').screenshot({ type: 'png' });
	} finally {
		await context.close();
	}
}

const idSet = new Set(ids);
const cases = pptxVisualCorpus().filter((c) => {
	const manifestId = c.id.replace(/^pptx-/, '');
	return idSet.has(manifestId) || idSet.has(c.id);
});

mkdirSync(join(OUT, 'diffs'), { recursive: true });
await prepareAll();
const server = await startVisualServer();
const browser = await chromium.launch({ headless: true, args: ['--disable-lcd-text'] });
const thr = THRESHOLDS.pptx;
const rows = [];

for (const c of cases) {
	const manifestId = c.id.replace(/^pptx-/, '');
	const row = { id: manifestId, dpr1: null, dpr2: null, pass: false, err: null };
	try {
		for (const dpr of [1, 2]) {
			const ts = await shot(browser, server.url, c, 'ts', dpr);
			const rust = await shot(browser, server.url, c, 'rust', dpr);
			const cmp = comparePngBuffers(ts, rust, {
				format: 'pptx',
				threshold: thr,
				diffPath: join(OUT, 'diffs', `${c.id}-focus-dpr${dpr}.png`),
			});
			row[`dpr${dpr}`] = cmp.ratio * 100;
		}
		row.pass = row.dpr1 <= thr * 100 && row.dpr2 <= thr * 100;
	} catch (e) {
		row.err = String(e && e.message ? e.message : e).slice(0, 240);
	}
	rows.push(row);
	console.log(JSON.stringify(row));
}

await browser.close();
await server.close();
writeFileSync(join(OUT, 'pptx-focus.json'), JSON.stringify(rows, null, 2));
