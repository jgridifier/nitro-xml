// SPDX-License-Identifier: MIT
/** One-off DPR1/DPR2 mismatch % for xlsx edit chrome (issue #150). */
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { startVisualServer } from './server.mjs';
import { prepareAll } from './prepare.mjs';
import { comparePngBuffers } from './compare.mjs';
import { THRESHOLDS } from './thresholds.mjs';

const VISUAL = dirname(fileURLToPath(import.meta.url));
const FIXTURE = 'fixtures/real/xlsx/poi-45544.xlsx';
const WIDTH = 960;
const HEIGHT = 132;

function hostUrl(base, side) {
	const q = new URLSearchParams({
		side,
		id: 'xlsx-edit-chrome-poi-45544',
		fixture: `/repo/${FIXTURE}`,
		width: String(WIDTH),
		height: String(HEIGHT),
	});
	return `${base}/pages/xlsx-edit-chrome.html?${q}`;
}

async function shot(browser, base, side, dpr) {
	const context = await browser.newContext({
		viewport: { width: WIDTH, height: HEIGHT },
		deviceScaleFactor: dpr,
	});
	try {
		const page = await context.newPage();
		await page.goto(hostUrl(base, side), { waitUntil: 'networkidle', timeout: 180_000 });
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
				document.getElementById('capture')?.textContent?.slice(0, 400),
			);
			throw new Error(`host not ready: ${body || ''}`);
		}
		return await page.locator('#capture').screenshot({ type: 'png' });
	} finally {
		await context.close();
	}
}

await prepareAll();
const server = await startVisualServer();
const browser = await chromium.launch({ headless: true, args: ['--disable-lcd-text'] });
try {
	for (const dpr of [1, 2]) {
		const ts = await shot(browser, server.url, 'ts', dpr);
		const rust = await shot(browser, server.url, 'rust', dpr);
		const cmp = comparePngBuffers(ts, rust, {
			format: 'xlsx',
			threshold: THRESHOLDS.xlsx,
			diffPath: join(VISUAL, 'out', `xlsx-edit-chrome-measure-dpr${dpr}.png`),
		});
		console.log(
			`dpr${dpr}: ${(cmp.ratio * 100).toFixed(4)}% (${cmp.mismatched}/${cmp.total}) ok=${cmp.ok}`,
		);
	}
} finally {
	await browser.close();
	await server.close();
}
