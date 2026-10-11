// SPDX-License-Identifier: MIT
/**
 * Live-viewer selection/caret geometry (#43): Playwright drives the same
 * select-ref (and later pointer/keyboard) actions on upstream TS and
 * `<nitro-xlsx>` / `<nitro-pptx>`, then compares selection, outline/caret
 * rectangles (±1 CSS px), and copied text.
 */

import { mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { startVisualServer } from '../../parity/visual/server.mjs';
import { readyCases } from './registry.mjs';
import { normalizeCopy } from './shell.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const VISUAL = join(ROOT, 'parity/visual');
const OUT = join(ROOT, 'tests/selection/out');
const CARET_TOLERANCE_PX = 1;

async function loadChromium() {
	const mod = await import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);
	return mod.chromium;
}

function viewerUrl(base, c, side) {
	const page = c.format === 'pptx' ? 'pptx-viewer.html' : 'xlsx-viewer.html';
	const q = new URLSearchParams({
		side,
		id: c.id,
		fixture: `/repo/${c.fixture}`,
		width: String(c.width || 800),
		height: String(c.height || 480),
	});
	return `${base}/pages/${page}?${q}`;
}

async function waitReady(page, timeoutMs = 45_000) {
	await page.waitForFunction(() => {
		const el = document.getElementById('stage');
		return el && (el.dataset.ready === '1' || el.dataset.ready === 'err');
	}, null, { timeout: timeoutMs });
	const ready = await page.evaluate(() => document.getElementById('stage')?.dataset.ready);
	if (ready !== '1') {
		const body = await page.evaluate(() =>
			document.getElementById('stage')?.textContent?.slice(0, 500),
		);
		throw new Error(`viewer not ready: ${body || ''}`);
	}
}

function rectsWithin(a, b, tol) {
	if (!a || !b) return a == null && b == null;
	return (
		Math.abs(a.x - b.x) <= tol &&
		Math.abs(a.y - b.y) <= tol &&
		Math.abs(a.w - b.w) <= tol &&
		Math.abs(a.h - b.h) <= tol
	);
}

async function runActionsOnPage(page, side, actions) {
	return page.evaluate(
		async ({ side, actions }) => {
			const stage = document.getElementById('stage');
			const rustEl = stage?.querySelector('nitro-xlsx, nitro-pptx');
			const tsEl = stage?.querySelector('xlsx-editor');

			const waitFrames = (n = 2) =>
				new Promise((resolve) => {
					let left = n;
					const step = () => {
						left -= 1;
						if (left <= 0) resolve();
						else requestAnimationFrame(step);
					};
					requestAnimationFrame(step);
				});

			const readRects = (root) => {
				const outline = root?.querySelector('.xg-outline:not([hidden])');
				const active = root?.querySelector('.xg-active:not([hidden])');
				const box = (el) => {
					if (!el) return null;
					const r = el.getBoundingClientRect();
					return { x: r.x, y: r.y, w: r.width, h: r.height };
				};
				return {
					highlight: box(outline),
					caret: box(active) || box(outline),
				};
			};

			let selection = '';
			let copy = '';
			let highlight = null;
			let caret = null;

			if (side === 'rust') {
				const el = rustEl;
				if (!el?.document) throw new Error('no nitro document');
				for (const a of actions) {
					if (a.op === 'select-ref') {
						const r = await el.shellAction('select', a.ref || 'A1');
						selection = r?.ui?.selection ?? a.ref;
						for (let i = 0; i < 4; i++) {
							await el.document.tick();
							await waitFrames(1);
						}
					} else if (a.op === 'copy') {
						const r = await el.shellAction('copy', '');
						copy = r?.c ?? r?.copyText ?? '';
						selection = r?.ui?.selection ?? selection;
					} else {
						throw new Error(`live rust action not wired: ${a.op}`);
					}
				}
				const root = el.shadowRoot;
				const boxes = readRects(root);
				highlight = boxes.highlight;
				caret = boxes.caret;
			} else {
				const el = tsEl;
				if (!el) throw new Error('no xlsx-editor');
				for (const a of actions) {
					if (a.op === 'select-ref') {
						el.select(a.ref || 'A1');
						selection = el.getSelection();
						await waitFrames(3);
					} else if (a.op === 'copy') {
						// Prefer public selection; TSV from cell text via shell oracle in shell.mjs.
						selection = el.getSelection();
						copy = ''; // filled by Node-side TS oracle when needed
						await waitFrames(1);
					} else {
						throw new Error(`live ts action not wired: ${a.op}`);
					}
				}
				const root = el.shadowRoot || el;
				const boxes = readRects(root);
				highlight = boxes.highlight;
				caret = boxes.caret;
			}

			return { selection, copy, highlight, caret };
		},
		{ side, actions },
	);
}

/**
 * @returns {Promise<{ ok: boolean, rows: object[], summary: string }>}
 */
export async function runLiveChecks() {
	const cases = readyCases().filter((c) => c.live);
	const rows = [];
	if (cases.length === 0) {
		return { ok: true, rows, summary: 'selection live: PASS (0 cases)' };
	}

	mkdirSync(OUT, { recursive: true });
	const chromium = await loadChromium();
	const server = await startVisualServer({ root: VISUAL });
	const browser = await chromium.launch({ headless: true });
	let failed = 0;

	try {
		for (const c of cases) {
			const want = new Set(c.compare || ['selection', 'highlight', 'caret']);
			const ctx = await browser.newContext({
				viewport: { width: c.width || 800, height: c.height || 480 },
				deviceScaleFactor: 1,
			});
			try {
				const tsPage = await ctx.newPage();
				const rustPage = await ctx.newPage();
				await tsPage.goto(viewerUrl(server.url, c, 'ts'), { waitUntil: 'domcontentloaded' });
				await rustPage.goto(viewerUrl(server.url, c, 'rust'), {
					waitUntil: 'domcontentloaded',
				});
				await waitReady(tsPage);
				await waitReady(rustPage);

				const ts = await runActionsOnPage(tsPage, 'ts', c.actions || []);
				const rust = await runActionsOnPage(rustPage, 'rust', c.actions || []);

				const selOk = !want.has('selection') || rust.selection === ts.selection;
				const hlOk =
					!want.has('highlight') ||
					rectsWithin(rust.highlight, ts.highlight, CARET_TOLERANCE_PX);
				const caretOk =
					!want.has('caret') || rectsWithin(rust.caret, ts.caret, CARET_TOLERANCE_PX);
				const copyOk =
					!want.has('copy') ||
					normalizeCopy(rust.copy) === normalizeCopy(ts.copy);
				const ok = selOk && hlOk && caretOk && copyOk;
				if (!ok) failed++;
				rows.push({
					case: c.id,
					kind: 'live',
					ok,
					selOk,
					hlOk,
					caretOk,
					copyOk,
					rust,
					ts,
				});
				writeFileSync(
					join(OUT, `${c.id}.json`),
					JSON.stringify({ rust, ts, ok }, null, 2),
				);
			} finally {
				await ctx.close();
			}
		}
	} finally {
		await browser.close();
		await server.close();
	}

	const summary = [
		`selection live: ${failed === 0 ? 'PASS' : 'FAIL'} (${rows.filter((r) => r.ok).length}/${rows.length}) ±${CARET_TOLERANCE_PX}px`,
		...rows.map((r) => {
			const tag = r.ok ? 'OK' : 'FAIL';
			const detail = [
				r.selOk ? '' : 'selection',
				r.hlOk ? '' : 'highlight',
				r.caretOk ? '' : 'caret',
				r.copyOk ? '' : 'copy',
			]
				.filter(Boolean)
				.join(',') || 'match';
			return `  ${tag} ${r.case} ${detail} rustSel=${JSON.stringify(r.rust.selection)} tsSel=${JSON.stringify(r.ts.selection)}`;
		}),
	].join('\n');
	return { ok: failed === 0, rows, summary };
}

if (import.meta.main) {
	const { ok, summary } = await runLiveChecks();
	console.log(summary);
	process.exit(ok ? 0 : 1);
}
