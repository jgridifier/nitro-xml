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

function caseThreshold(c) {
	return c.threshold ?? THRESHOLDS[c.format];
}

function hostUrl(base, c, side) {
	if (c.kind === 'xlsx-viewer') {
		const q = new URLSearchParams({
			side,
			id: c.id,
			fixture: `/repo/${c.fixture}`,
			width: String(c.width || 800),
			height: String(c.height || 480),
		});
		return `${base}/pages/xlsx-viewer.html?${q}`;
	}
	if (c.kind === 'pptx-viewer') {
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
	if (c.kind === 'docx-viewer') {
		const q = new URLSearchParams({
			side,
			id: c.id,
			fixture: `/repo/${c.fixture}`,
			width: String(c.width || 800),
			height: String(c.height || 600),
		});
		return `${base}/pages/docx-viewer.html?${q}`;
	}
	const q = new URLSearchParams({
		kind: c.kind,
		side,
		id: c.id,
		width: String(c.width || 480),
		height: String(c.height || 320),
	});
	return `${base}/pages/host.html?${q}`;
}

function readyTimeoutMs(c) {
	if (c.kind === 'xlsx-viewer' || c.kind === 'pptx-viewer' || c.kind === 'docx-viewer') {
		return 90_000;
	}
	return 30_000;
}

function recordRow(rows, c, row) {
	const informational = Boolean(c.informational);
	const hardFail = !row.ok && !informational;
	rows.push({ ...row, informational, hardFail });
	return hardFail ? 1 : 0;
}

async function waitReady(page, timeoutMs = 30_000) {
	await page.waitForFunction(() => {
		const el = document.getElementById('stage');
		return el && (el.dataset.ready === '1' || el.dataset.ready === 'err');
	}, null, { timeout: timeoutMs });
	const ready = await page.evaluate(() => document.getElementById('stage')?.dataset.ready);
	if (ready !== '1') {
		const title = await page.title();
		const body = await page.evaluate(() => document.getElementById('stage')?.textContent?.slice(0, 400));
		throw new Error(`host not ready: ${title}${body ? ` — ${body}` : ''}`);
	}
}

async function screenshotSide(browser, base, c, side, dpr) {
	const context = await browser.newContext({
		viewport: { width: c.width || 480, height: c.height || 320 },
		deviceScaleFactor: dpr,
	});
	try {
		const page = await context.newPage();
		await page.goto(hostUrl(base, c, side), { waitUntil: 'networkidle' });
		await waitReady(page, readyTimeoutMs(c));
		return await page.locator('#stage').screenshot({ type: 'png' });
	} finally {
		await context.close();
	}
}

/** Record a host/compare failure without aborting the gate (informational cases). */
function recordHostError(rows, c, mode, dpr, err) {
	const reason = String(err && err.message ? err.message : err).slice(0, 240);
	return recordRow(rows, c, {
		case: c.id,
		mode,
		dpr,
		ok: false,
		ratio: 1,
		mismatched: 1,
		total: 1,
		threshold: caseThreshold(c),
		reason,
	});
}

async function printPdf(browser, base, c, side) {
	const context = await browser.newContext({
		viewport: { width: c.width || 480, height: c.height || 320 },
		deviceScaleFactor: 1,
	});
	try {
		const page = await context.newPage();
		await page.goto(hostUrl(base, c, side), { waitUntil: 'networkidle' });
		await waitReady(page, readyTimeoutMs(c));
		await page.emulateMedia({ media: 'print' });
		return await page.pdf({
			width: `${c.width || 480}px`,
			height: `${c.height || 320}px`,
			printBackground: true,
			margin: { top: '0', right: '0', bottom: '0', left: '0' },
		});
	} finally {
		await context.close();
	}
}

async function rasterizePdf(browser, base, pdfRelUrl, dpr = 1) {
	const context = await browser.newContext({
		viewport: { width: 1200, height: 900 },
		deviceScaleFactor: dpr,
	});
	try {
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
		return await page.locator('#c').screenshot({ type: 'png' });
	} finally {
		await context.close();
	}
}

/**
 * @returns {Promise<{ ok: boolean, rows: object[], summary: string }>}
 */
export async function runVisualGate() {
	mkdirSync(SHOTS, { recursive: true });
	mkdirSync(DIFFS, { recursive: true });
	mkdirSync(PDFS, { recursive: true });
	mkdirSync(OUT, { recursive: true });

	/** @type {object[]} */
	const rows = [];
	let failed = 0;
	let runError = null;
	let browser = null;
	let server = null;

	const writeSummary = () => {
		const deferred = VISUAL_CASES.filter((c) => !c.ready).map((c) => `${c.id}: ${c.todo}`);
		const hardOk = rows.filter((r) => r.ok || r.informational).length;
		const pptxTable = buildPptxFixtureTable(rows);
		const summaryLines = [
			runError
				? `visual: FAIL (runner error: ${runError})`
				: `visual: ${failed === 0 ? 'PASS' : 'FAIL'} (${hardOk}/${rows.length} comparisons; ${rows.filter((r) => r.informational && !r.ok).length} informational)`,
			...rows.map((r) => {
				const tag = r.ok ? 'OK' : r.informational ? 'INFO' : 'FAIL';
				return `  ${tag} ${r.case} ${r.mode} dpr=${r.dpr} mismatch=${(r.ratio * 100).toFixed(4)}% (limit ${(r.threshold * 100).toFixed(2)}%)${r.reason ? ` ${r.reason}` : ''}${r.informational && !r.ok ? ' [informational]' : ''}`;
			}),
			...(pptxTable.length
				? [
						'',
						'pptx per-fixture pixel table (ts-vs-rust; hard ≤0.1% / else informational):',
						'fixture\tdpr1%\tdpr2%\tdpr1_ok\tdpr2_ok\tnote',
						...pptxTable,
					]
				: []),
			...(deferred.length ? ['deferred:', ...deferred.map((d) => `  - ${d}`)] : []),
			`diffs: ${DIFFS}`,
		];
		const summary = summaryLines.join('\n');
		writeFileSync(join(OUT, 'summary.txt'), summary + '\n');
		writeFileSync(
			join(OUT, 'results.json'),
			JSON.stringify(
				{ failed, rows, deferred, pptxTable, error: runError ? String(runError) : null },
				null,
				2,
			),
		);
		console.log(summary);
	};

	try {
		await prepareAll();
		server = await startVisualServer();
		// Match TS and Rust glyph AA at DPR2 (same method as #171 docx soft
		// corpus: Linux Chromium otherwise keeps ClearType on one surface while
		// the other goes grayscale). Applied to both sides.
		browser = await chromium.launch({ headless: true, args: ['--disable-lcd-text'] });

		// Harness self-check: TS-vs-TS at DPR 1 for every ready case (acceptance: green before Rust).
		for (const c of readyCases()) {
			try {
				const a = await screenshotSide(browser, server.url, c, 'ts', 1);
				const b = await screenshotSide(browser, server.url, c, 'ts', 1);
				writeFileSync(join(SHOTS, `${c.id}-ts-dpr1-a.png`), a);
				writeFileSync(join(SHOTS, `${c.id}-ts-dpr1-b.png`), b);
				const thr = caseThreshold(c);
				const cmp = comparePngBuffers(a, b, {
					format: c.format,
					threshold: thr,
					diffPath: join(DIFFS, `${c.id}-ts-vs-ts-dpr1.png`),
				});
				failed += recordRow(rows, c, {
					case: c.id,
					mode: 'ts-vs-ts',
					dpr: 1,
					ok: cmp.ok,
					ratio: cmp.ratio,
					mismatched: cmp.mismatched,
					total: cmp.total,
					threshold: thr,
				});
			} catch (err) {
				if (c.informational) failed += recordHostError(rows, c, 'ts-vs-ts', 1, err);
				else throw err;
			}
		}

		for (const c of readyCases()) {
			if (!c.rust) continue;
			for (const dpr of [1, 2]) {
				try {
					const ts = await screenshotSide(browser, server.url, c, 'ts', dpr);
					const rust = await screenshotSide(browser, server.url, c, 'rust', dpr);
					writeFileSync(join(SHOTS, `${c.id}-ts-dpr${dpr}.png`), ts);
					writeFileSync(join(SHOTS, `${c.id}-rust-dpr${dpr}.png`), rust);
					const thr = caseThreshold(c);
					const cmp = comparePngBuffers(ts, rust, {
						format: c.format,
						threshold: thr,
						diffPath: join(DIFFS, `${c.id}-ts-vs-rust-dpr${dpr}.png`),
					});
					failed += recordRow(rows, c, {
						case: c.id,
						mode: 'ts-vs-rust',
						dpr,
						ok: cmp.ok,
						ratio: cmp.ratio,
						mismatched: cmp.mismatched,
						total: cmp.total,
						threshold: thr,
						reason: cmp.reason,
					});
				} catch (err) {
					if (c.informational) failed += recordHostError(rows, c, 'ts-vs-rust', dpr, err);
					else throw err;
				}
			}

			if (c.skipPrint) continue;

			// Print-to-PDF: emit PDFs, rasterize page 1, pixelmatch.
			try {
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
				const thr = caseThreshold(c);
				const cmp = comparePngBuffers(tsRaster, rustRaster, {
					format: c.format,
					threshold: thr,
					diffPath: join(DIFFS, `${c.id}-ts-vs-rust-print.png`),
				});
				failed += recordRow(rows, c, {
					case: c.id,
					mode: 'ts-vs-rust-print-pdf',
					dpr: 1,
					ok: cmp.ok,
					ratio: cmp.ratio,
					mismatched: cmp.mismatched,
					total: cmp.total,
					threshold: thr,
					reason: cmp.reason,
				});
			} catch (err) {
				// Informational host timeouts must not abort remaining cases (poi-bug65649).
				if (c.informational) failed += recordHostError(rows, c, 'ts-vs-rust-print-pdf', 1, err);
				else throw err;
			}
		}
	} catch (e) {
		runError = e;
	} finally {
		if (browser) await browser.close();
		if (server) await server.close();
		writeSummary();
	}
	return { ok: failed === 0 && !runError, rows, summary: '' };
}

/** One TSV row per pptx fixture with DPR1/DPR2 mismatch percents for the artifact. */
function buildPptxFixtureTable(rows) {
	/** @type {Map<string, { d1?: object, d2?: object }>} */
	const byCase = new Map();
	for (const r of rows) {
		if (!String(r.case).startsWith('pptx-')) continue;
		if (r.mode !== 'ts-vs-rust') continue;
		let slot = byCase.get(r.case);
		if (!slot) {
			slot = {};
			byCase.set(r.case, slot);
		}
		if (r.dpr === 1) slot.d1 = r;
		else if (r.dpr === 2) slot.d2 = r;
	}
	const ids = [...byCase.keys()].sort();
	return ids.map((id) => {
		const { d1, d2 } = byCase.get(id);
		const pct = (r) => (r ? (r.ratio * 100).toFixed(4) : '');
		const ok = (r) => (r ? (r.ok ? 'OK' : 'INFO') : '');
		const note = [d1?.reason, d2?.reason].filter(Boolean).join('; ').replace(/\t/g, ' ');
		return `${id}\t${pct(d1)}\t${pct(d2)}\t${ok(d1)}\t${ok(d2)}\t${note}`;
	});
}

if (import.meta.main) {
	const { ok } = await runVisualGate();
	process.exit(ok ? 0 : 1);
}
