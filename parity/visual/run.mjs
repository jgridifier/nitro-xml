// SPDX-License-Identifier: MIT
/**
 * Visual gate runner (#45): Playwright screenshots at DPR 1 and 2, print-to-PDF,
 * pixelmatch TS oracle vs Rust (and TS-vs-TS harness self-check).
 *
 * Minimal headless host/runner taken from #6's scope — no perf metrics or timing.
 */

import { mkdirSync, writeFileSync, existsSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { readyCases, VISUAL_CASES } from './registry.mjs';
import { startVisualServer } from './server.mjs';
import { prepareAll } from './prepare.mjs';
import { comparePngBuffers } from './compare.mjs';
import { THRESHOLDS } from './thresholds.mjs';

const VISUAL = dirname(fileURLToPath(import.meta.url));
const OUT = join(VISUAL, 'out');
const SHOTS = join(OUT, 'shots');
const DIFFS = join(OUT, 'diffs');
const PDFS = join(OUT, 'pdfs');

function hostUrl(base, c, side) {
	const q = new URLSearchParams({
		kind: c.kind,
		side,
		id: c.id,
		width: String(c.width || 480),
		height: String(c.height || 320),
	});
	return `${base}/pages/host.html?${q}`;
}

async function waitReady(page) {
	await page.waitForFunction(() => {
		const el = document.getElementById('stage');
		return el && (el.dataset.ready === '1' || el.dataset.ready === 'err');
	}, null, { timeout: 30_000 });
	const ready = await page.evaluate(() => document.getElementById('stage')?.dataset.ready);
	if (ready !== '1') {
		const title = await page.title();
		throw new Error(`host not ready: ${title}`);
	}
}

async function screenshotSide(browser, base, c, side, dpr) {
	const context = await browser.newContext({
		viewport: { width: c.width || 480, height: c.height || 320 },
		deviceScaleFactor: dpr,
	});
	const page = await context.newPage();
	await page.goto(hostUrl(base, c, side), { waitUntil: 'networkidle' });
	await waitReady(page);
	const buf = await page.locator('#stage').screenshot({ type: 'png' });
	await context.close();
	return buf;
}

async function printPdf(browser, base, c, side) {
	const context = await browser.newContext({
		viewport: { width: c.width || 480, height: c.height || 320 },
		deviceScaleFactor: 1,
	});
	const page = await context.newPage();
	await page.goto(hostUrl(base, c, side), { waitUntil: 'networkidle' });
	await waitReady(page);
	await page.emulateMedia({ media: 'print' });
	const pdf = await page.pdf({
		width: `${c.width || 480}px`,
		height: `${c.height || 320}px`,
		printBackground: true,
		margin: { top: '0', right: '0', bottom: '0', left: '0' },
	});
	await context.close();
	return pdf;
}

async function rasterizePdf(browser, base, pdfRelUrl, dpr = 1) {
	const context = await browser.newContext({
		viewport: { width: 1200, height: 900 },
		deviceScaleFactor: dpr,
	});
	const page = await context.newPage();
	await page.goto(`${base}/pages/pdf-raster.html?src=${encodeURIComponent(pdfRelUrl)}`, {
		waitUntil: 'networkidle',
	});
	await page.waitForFunction(() => {
		const c = document.getElementById('c');
		return c && (c.dataset.ready === '1' || c.dataset.ready === 'err');
	}, null, { timeout: 60_000 });
	const ready = await page.evaluate(() => document.getElementById('c')?.dataset.ready);
	if (ready !== '1') {
		throw new Error(`pdf raster failed: ${await page.title()}`);
	}
	const buf = await page.locator('#c').screenshot({ type: 'png' });
	await context.close();
	return buf;
}

/**
 * @returns {Promise<{ ok: boolean, rows: object[], summary: string }>}
 */
export async function runVisualGate() {
	mkdirSync(SHOTS, { recursive: true });
	mkdirSync(DIFFS, { recursive: true });
	mkdirSync(PDFS, { recursive: true });

	await prepareAll();
	const server = await startVisualServer();
	const browser = await chromium.launch({ headless: true });
	/** @type {object[]} */
	const rows = [];
	let failed = 0;

	try {
		// Harness self-check: TS-vs-TS at DPR 1 for every ready case (acceptance: green before Rust).
		for (const c of readyCases()) {
			const a = await screenshotSide(browser, server.url, c, 'ts', 1);
			const b = await screenshotSide(browser, server.url, c, 'ts', 1);
			writeFileSync(join(SHOTS, `${c.id}-ts-dpr1-a.png`), a);
			writeFileSync(join(SHOTS, `${c.id}-ts-dpr1-b.png`), b);
			const cmp = comparePngBuffers(a, b, {
				format: c.format,
				diffPath: join(DIFFS, `${c.id}-ts-vs-ts-dpr1.png`),
			});
			const row = {
				case: c.id,
				mode: 'ts-vs-ts',
				dpr: 1,
				ok: cmp.ok,
				ratio: cmp.ratio,
				mismatched: cmp.mismatched,
				total: cmp.total,
				threshold: THRESHOLDS[c.format],
			};
			rows.push(row);
			if (!cmp.ok) failed++;
		}

		for (const c of readyCases()) {
			if (!c.rust) continue;
			for (const dpr of [1, 2]) {
				const ts = await screenshotSide(browser, server.url, c, 'ts', dpr);
				const rust = await screenshotSide(browser, server.url, c, 'rust', dpr);
				writeFileSync(join(SHOTS, `${c.id}-ts-dpr${dpr}.png`), ts);
				writeFileSync(join(SHOTS, `${c.id}-rust-dpr${dpr}.png`), rust);
				const cmp = comparePngBuffers(ts, rust, {
					format: c.format,
					diffPath: join(DIFFS, `${c.id}-ts-vs-rust-dpr${dpr}.png`),
				});
				const row = {
					case: c.id,
					mode: 'ts-vs-rust',
					dpr,
					ok: cmp.ok,
					ratio: cmp.ratio,
					mismatched: cmp.mismatched,
					total: cmp.total,
					threshold: THRESHOLDS[c.format],
					reason: cmp.reason,
				};
				rows.push(row);
				if (!cmp.ok) failed++;
			}

			// Print-to-PDF: emit PDFs, rasterize page 1, pixelmatch.
			const tsPdf = await printPdf(browser, server.url, c, 'ts');
			const rustPdf = await printPdf(browser, server.url, c, 'rust');
			const tsPdfPath = join(PDFS, `${c.id}-ts.pdf`);
			const rustPdfPath = join(PDFS, `${c.id}-rust.pdf`);
			writeFileSync(tsPdfPath, tsPdf);
			writeFileSync(rustPdfPath, rustPdf);
			const tsRaster = await rasterizePdf(browser, server.url, `/out/pdfs/${c.id}-ts.pdf`);
			const rustRaster = await rasterizePdf(browser, server.url, `/out/pdfs/${c.id}-rust.pdf`);
			writeFileSync(join(SHOTS, `${c.id}-ts-print.png`), tsRaster);
			writeFileSync(join(SHOTS, `${c.id}-rust-print.png`), rustRaster);
			const cmp = comparePngBuffers(tsRaster, rustRaster, {
				format: c.format,
				diffPath: join(DIFFS, `${c.id}-ts-vs-rust-print.png`),
			});
			const row = {
				case: c.id,
				mode: 'ts-vs-rust-print-pdf',
				dpr: 1,
				ok: cmp.ok,
				ratio: cmp.ratio,
				mismatched: cmp.mismatched,
				total: cmp.total,
				threshold: THRESHOLDS[c.format],
				reason: cmp.reason,
			};
			rows.push(row);
			if (!cmp.ok) failed++;
		}
	} finally {
		await browser.close();
		await server.close();
	}

	const deferred = VISUAL_CASES.filter((c) => !c.ready).map((c) => `${c.id}: ${c.todo}`);
	const summaryLines = [
		`visual: ${failed === 0 ? 'PASS' : 'FAIL'} (${rows.filter((r) => r.ok).length}/${rows.length} comparisons)`,
		...rows.map(
			(r) =>
				`  ${r.ok ? 'OK' : 'FAIL'} ${r.case} ${r.mode} dpr=${r.dpr} mismatch=${(r.ratio * 100).toFixed(4)}% (limit ${(r.threshold * 100).toFixed(2)}%)${r.reason ? ` ${r.reason}` : ''}`,
		),
		...(deferred.length ? ['deferred:', ...deferred.map((d) => `  - ${d}`)] : []),
		`diffs: ${DIFFS}`,
	];
	const summary = summaryLines.join('\n');
	writeFileSync(join(OUT, 'summary.txt'), summary + '\n');
	writeFileSync(join(OUT, 'results.json'), JSON.stringify({ failed, rows, deferred }, null, 2));
	console.log(summary);
	return { ok: failed === 0, rows, summary };
}

if (import.meta.main) {
	const { ok } = await runVisualGate();
	process.exit(ok ? 0 : 1);
}
