// SPDX-License-Identifier: MIT
/**
 * Live-viewer selection/caret geometry (#43): Playwright drives the same
 * actions on upstream TS and `<nitro-xlsx>` / `<nitro-pptx>`, then compares
 * selection, outline/caret rectangles (±1 CSS px), and copied text.
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

async function waitReady(page, timeoutMs = 60_000) {
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

function parseA1(ref) {
	const m = /^([A-Z]+)(\d+)$/i.exec(ref.trim());
	if (!m) throw new Error(`bad A1 ${ref}`);
	let col = 0;
	for (const ch of m[1].toUpperCase()) col = col * 26 + (ch.charCodeAt(0) - 64);
	return { row: Number(m[2]) - 1, col: col - 1 };
}

/** Grid-relative CSS px center of a cell (headers included). */
async function cellPoint(page, side, ref) {
	const { row, col } = parseA1(ref);
	return page.evaluate(
		({ side, row, col }) => {
			const stage = document.getElementById('stage');
			const root =
				side === 'rust'
					? stage?.querySelector('nitro-xlsx')?.shadowRoot
					: stage?.querySelector('xlsx-editor')?.shadowRoot ||
						stage?.querySelector('xlsx-editor');
			const view =
				side === 'rust'
					? root?.querySelector('canvas') || root?.querySelector('.v')
					: root?.querySelector('.xg-view') || root?.querySelector('.xve-grid-host');
			if (!view) throw new Error('no grid view');
			const corner = root?.querySelector('.xg-corner');
			const hw = corner?.offsetWidth || 33;
			const hh = corner?.offsetHeight || 20;
			const colW = 64;
			const rowH = 20;
			const box = view.getBoundingClientRect();
			return {
				x: box.x + hw + col * colW + colW / 2,
				y: box.y + hh + row * rowH + rowH / 2,
			};
		},
		{ side, row, col },
	);
}

async function tickRust(page) {
	await page.evaluate(async () => {
		const el = document.querySelector('nitro-xlsx, nitro-pptx');
		if (!el?.document) return;
		for (let i = 0; i < 4; i++) {
			await el.document.tick();
			await new Promise((r) => requestAnimationFrame(r));
		}
	});
}

async function readState(page, side, format) {
	return page.evaluate(
		async ({ side, format }) => {
			const stage = document.getElementById('stage');
			const box = (el) => {
				if (!el) return null;
				const r = el.getBoundingClientRect();
				if (r.width <= 0 && r.height <= 0) return null;
				return { x: r.x, y: r.y, w: r.width, h: r.height };
			};

			if (format === 'xlsx') {
				if (side === 'rust') {
					const el = stage?.querySelector('nitro-xlsx');
					const ui = await el.shellAction('ui');
					const root = el.shadowRoot;
					const outline = root?.querySelector('.xg-outline:not([hidden])');
					const active = root?.querySelector('.xg-active:not([hidden])');
					return {
						selection: ui?.ui?.selection ?? '',
						copy: '',
						highlight: box(outline),
						caret: box(active) || box(outline),
					};
				}
				const el = stage?.querySelector('xlsx-editor');
				const root = el.shadowRoot || el;
				const outline = root?.querySelector('.xg-outline:not([hidden])');
				const active = root?.querySelector('.xg-active:not([hidden])');
				return {
					selection: el.getSelection(),
					copy: '',
					highlight: box(outline),
					caret: box(active) || box(outline),
				};
			}

			// pptx
			if (side === 'rust') {
				const el = stage?.querySelector('nitro-pptx');
				const ui = await el.shellAction('ui');
				const root = el.shadowRoot;
				const outline = root?.querySelector('.pptx-sel');
				const domSel = root?.getSelection?.() || null;
				const textSel = domSel?.toString() || '';
				let caret = box(outline);
				if (textSel && domSel?.rangeCount) {
					const r = domSel.getRangeAt(0).getBoundingClientRect();
					if (r.width || r.height) caret = { x: r.x, y: r.y, w: r.width, h: r.height };
				}
				return {
					selection: ui?.ui?.selection ?? '',
					copy: '',
					highlight: box(outline),
					caret,
					textCopy: textSel,
				};
			}
			// TS reading paint: selection stamped by harness helpers on stage.dataset
			const root = stage?.querySelector('#ts-wrap') || stage;
			const id = stage?.dataset.selId || '';
			const ids = stage?.dataset.selIds || id;
			const node = id
				? root.querySelector(`[data-element-id="${CSS.escape(id)}"]`)
				: null;
			const textSel = window.getSelection()?.toString() || '';
			return {
				selection: ids,
				copy: '',
				highlight: box(node),
				caret: textSel
					? (() => {
							const r = window.getSelection()?.getRangeAt(0)?.getBoundingClientRect();
							return r && (r.width || r.height)
								? { x: r.x, y: r.y, w: r.width, h: r.height }
								: null;
						})()
					: box(node),
				textCopy: textSel,
			};
		},
		{ side, format },
	);
}

async function rustCopy(page) {
	return page.evaluate(async () => {
		const el = document.querySelector('nitro-xlsx, nitro-pptx');
		const r = await el.shellAction('copy', '');
		return r?.c ?? r?.copyText ?? '';
	});
}

async function tsXlsxCopy(page, selection) {
	// Use the same oracle as shell.mjs via fixture bytes is hard in-page;
	// pull displayed TSV from the editor's selection through copyRange is not
	// exposed. Fall back to reading cell texts from the selection model via
	// getSelection + a small core import is unavailable in the bundle.
	// Instead: dispatch Ctrl+C is unreliable; return empty and let the Node
	// side fill from copyRange when we pass selection back.
	return { selection, needsOracle: true };
}

async function applySelectRef(page, side, format, ref) {
	if (format === 'xlsx') {
		if (side === 'rust') {
			await page.evaluate(async (ref) => {
				const el = document.querySelector('nitro-xlsx');
				await el.shellAction('select', ref);
				for (let i = 0; i < 4; i++) {
					await el.document.tick();
					await new Promise((r) => requestAnimationFrame(r));
				}
			}, ref);
		} else {
			await page.evaluate((ref) => {
				document.querySelector('xlsx-editor').select(ref);
			}, ref);
			await page.waitForTimeout(50);
		}
		return;
	}
	// pptx select by element id
	if (side === 'rust') {
		await page.evaluate(async (ref) => {
			const el = document.querySelector('nitro-pptx');
			await el.shellAction('select', ref);
			for (let i = 0; i < 4; i++) {
				await el.document.tick();
				await new Promise((r) => requestAnimationFrame(r));
			}
		}, ref);
	} else {
		await page.evaluate((ref) => {
			const stage = document.getElementById('stage');
			const root = stage.querySelector('#ts-wrap') || stage;
			const node = root.querySelector(`[data-element-id="${CSS.escape(ref)}"]`);
			if (!node) throw new Error(`no element ${ref}`);
			stage.dataset.selId = ref;
			stage.dataset.selIds = ref;
			node.scrollIntoView({ block: 'nearest' });
		}, ref);
	}
}

async function applyClick(page, side, format, ref, shift) {
	if (shift) await page.keyboard.down('Shift');
	try {
		if (format === 'xlsx') {
			const pt = await cellPoint(page, side, ref);
			await page.mouse.click(pt.x, pt.y);
			if (side === 'rust') await tickRust(page);
			else await page.waitForTimeout(80);
			return;
		}
		// pptx: click center of [data-element-id]
		const pt = await page.evaluate(
			({ side, ref }) => {
				const stage = document.getElementById('stage');
				const root =
					side === 'rust'
						? stage.querySelector('nitro-pptx')?.shadowRoot
						: stage.querySelector('#ts-wrap') || stage;
				const node = root.querySelector(`[data-element-id="${CSS.escape(ref)}"]`);
				if (!node) throw new Error(`no element ${ref}`);
				const r = node.getBoundingClientRect();
				return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
			},
			{ side, ref },
		);
		await page.mouse.click(pt.x, pt.y);
		if (side === 'rust') await tickRust(page);
		else {
			await page.evaluate((ref) => {
				const stage = document.getElementById('stage');
				stage.dataset.selId = ref;
				stage.dataset.selIds = ref;
			}, ref);
			await page.waitForTimeout(50);
		}
	} finally {
		if (shift) await page.keyboard.up('Shift');
	}
}

async function applyDrag(page, side, format, from, to) {
	if (format !== 'xlsx') throw new Error('drag only for xlsx');
	const a = await cellPoint(page, side, from);
	const b = await cellPoint(page, side, to);
	await page.mouse.move(a.x, a.y);
	await page.mouse.down();
	await page.mouse.move(b.x, b.y, { steps: 8 });
	await page.mouse.up();
	if (side === 'rust') await tickRust(page);
	else await page.waitForTimeout(80);
}

async function applyKeyboardExtend(page, side, key, count) {
	// Focus via a cell click so the editor's key sink is armed (TS Ready mode).
	if (side === 'ts') {
		const pt = await cellPoint(page, side, 'A1');
		await page.mouse.click(pt.x, pt.y);
		await page.waitForTimeout(50);
		// Re-select A1 without merge expansion side-effects for the extend baseline.
		await applySelectRef(page, side, 'xlsx', 'A1');
	} else {
		await page.evaluate(() => {
			const v = document.querySelector('nitro-xlsx')?.shadowRoot?.querySelector('.v');
			v?.focus();
		});
	}
	await page.keyboard.down('Shift');
	try {
		for (let i = 0; i < count; i++) {
			await page.keyboard.press(key);
			if (side === 'rust') await tickRust(page);
			else await page.waitForTimeout(30);
		}
	} finally {
		await page.keyboard.up('Shift');
	}
}

async function applySelectAll(page, side, format) {
	if (side === 'rust') {
		await page.evaluate((format) => {
			const sel = format === 'pptx' ? 'nitro-pptx' : 'nitro-xlsx';
			document.querySelector(sel)?.shadowRoot?.querySelector('.v')?.focus();
		}, format);
		await page.keyboard.press('Control+a');
		await tickRust(page);
		return;
	}
	if (format === 'pptx') {
		await page.evaluate(() => {
			const stage = document.getElementById('stage');
			const root = stage.querySelector('#ts-wrap') || stage;
			const nodes = [...root.querySelectorAll('[data-element-id]')].filter((el) => {
				const parent = el.parentElement?.closest?.('[data-element-id]');
				return !parent || parent === el;
			});
			const ids = nodes.map((n) => n.getAttribute('data-element-id')).filter(Boolean);
			stage.dataset.selIds = ids.join(',');
			stage.dataset.selId = ids[0] || '';
		});
		await page.waitForTimeout(50);
		return;
	}
	// xlsx TS Ready mode: keys land on the in-cell textarea sink.
	await page.evaluate(() => {
		const el = document.querySelector('xlsx-editor');
		el?.focusGrid?.();
		const root = el?.shadowRoot || el;
		const sink =
			root?.querySelector('textarea') ||
			root?.querySelector('.xg-edit') ||
			root?.querySelector('[contenteditable]') ||
			el;
		sink?.focus?.();
	});
	await page.keyboard.press('Control+a');
	await page.waitForTimeout(120);
}

async function applyTextSelect(page, side, elementId, substring) {
	await page.evaluate(
		({ side, elementId, substring }) => {
			const stage = document.getElementById('stage');
			const root =
				side === 'rust'
					? stage.querySelector('nitro-pptx')?.shadowRoot
					: stage.querySelector('#ts-wrap') || stage;
			const shape = root.querySelector(`[data-element-id="${CSS.escape(elementId)}"]`);
			if (!shape) throw new Error(`no shape ${elementId}`);
			const textEl =
				shape.querySelector('.pptxv-text') ||
				shape.querySelector('[data-pptx-kind="text"]') ||
				shape;
			const walker = document.createTreeWalker(textEl, NodeFilter.SHOW_TEXT);
			let node = walker.nextNode();
			let full = '';
			const parts = [];
			while (node) {
				parts.push(node);
				full += node.textContent || '';
				node = walker.nextNode();
			}
			const idx = full.indexOf(substring);
			if (idx < 0) throw new Error(`substring not found: ${substring}`);
			let at = 0;
			let startNode = null;
			let startOff = 0;
			let endNode = null;
			let endOff = 0;
			for (const n of parts) {
				const len = (n.textContent || '').length;
				if (!startNode && at + len > idx) {
					startNode = n;
					startOff = idx - at;
				}
				if (!endNode && at + len >= idx + substring.length) {
					endNode = n;
					endOff = idx + substring.length - at;
					break;
				}
				at += len;
			}
			const range = document.createRange();
			range.setStart(startNode, startOff);
			range.setEnd(endNode, endOff);
			const sel = window.getSelection();
			sel.removeAllRanges();
			sel.addRange(range);
			stage.dataset.selId = elementId;
			stage.dataset.selIds = elementId;
			stage.dataset.textCopy = substring;
		},
		{ side, elementId, substring },
	);
	if (side === 'rust') {
		// Keep shape selected for shell selection_ref; copy prefers DOM selection.
		await applySelectRef(page, 'rust', 'pptx', elementId);
		// Re-apply the text range inside the shadow root after paint.
		await page.evaluate(
			({ elementId, substring }) => {
				const root = document.querySelector('nitro-pptx')?.shadowRoot;
				const shape = root?.querySelector(`[data-element-id="${CSS.escape(elementId)}"]`);
				const textEl = shape?.querySelector('.pptxv-text') || shape;
				if (!textEl) return;
				const walker = document.createTreeWalker(textEl, NodeFilter.SHOW_TEXT);
				let node = walker.nextNode();
				let full = '';
				const parts = [];
				while (node) {
					parts.push(node);
					full += node.textContent || '';
					node = walker.nextNode();
				}
				const idx = full.indexOf(substring);
				if (idx < 0) return;
				let at = 0;
				let startNode = null;
				let startOff = 0;
				let endNode = null;
				let endOff = 0;
				for (const n of parts) {
					const len = (n.textContent || '').length;
					if (!startNode && at + len > idx) {
						startNode = n;
						startOff = idx - at;
					}
					if (!endNode && at + len >= idx + substring.length) {
						endNode = n;
						endOff = idx + substring.length - at;
						break;
					}
					at += len;
				}
				const range = document.createRange();
				range.setStart(startNode, startOff);
				range.setEnd(endNode, endOff);
				const sel = root.getSelection ? root.getSelection() : window.getSelection();
				sel.removeAllRanges();
				sel.addRange(range);
			},
			{ elementId, substring },
		);
	}
}

async function runActions(page, side, format, actions, fixtureRel) {
	let copy = '';
	let lastSelection = '';
	for (const a of actions) {
		if (a.op === 'select-ref') {
			await applySelectRef(page, side, format, a.ref || 'A1');
		} else if (a.op === 'click') {
			await applyClick(page, side, format, a.ref, false);
		} else if (a.op === 'shift-click') {
			await applyClick(page, side, format, a.ref, true);
		} else if (a.op === 'drag') {
			await applyDrag(page, side, format, a.from, a.to);
		} else if (a.op === 'keyboard-extend') {
			await applyKeyboardExtend(page, side, a.key || 'ArrowRight', a.count || 1);
		} else if (a.op === 'select-all') {
			await applySelectAll(page, side, format);
		} else if (a.op === 'text-select') {
			await applyTextSelect(page, side, a.elementId, a.substring || '');
		} else if (a.op === 'copy') {
			if (side === 'rust') {
				if (format === 'pptx') {
					// Prefer in-shadow text selection (text-select cases); else shell copy.
					const fromDom = await page.evaluate(() => {
						const root = document.querySelector('nitro-pptx')?.shadowRoot;
						const sel = root?.getSelection?.() || document.getSelection();
						const t = sel?.toString() || '';
						return t;
					});
					copy = fromDom || (await rustCopy(page));
				} else {
					copy = await rustCopy(page);
				}
			} else if (format === 'xlsx') {
				const st = await readState(page, side, format);
				lastSelection = st.selection;
				copy = await oracleCopy(fixtureRel, st.selection);
			} else {
				// pptx TS: prefer window selection text, else join selected shape texts
				// with paragraph breaks (matches DumpElement.text / Rust copy).
				copy = await page.evaluate(() => {
					const stage = document.getElementById('stage');
					const textSel = window.getSelection()?.toString();
					if (textSel) return textSel;
					if (stage.dataset.textCopy) return stage.dataset.textCopy;
					const root = stage.querySelector('#ts-wrap') || stage;
					const shapeText = (n) => {
						const body = n?.querySelector('.pptxv-text') || n;
						const paras = [...(body?.querySelectorAll?.('p,div') || [])];
						if (paras.length) {
							return paras
								.map((p) => (p.textContent || '').replace(/\r\n/g, '\n'))
								.filter((t) => t.length)
								.join('\n');
						}
						return (body?.textContent || '').replace(/\r\n/g, '\n');
					};
					const ids = (stage.dataset.selIds || '').split(',').filter(Boolean);
					return ids
						.map((id) => {
							const n = root.querySelector(`[data-element-id="${CSS.escape(id)}"]`);
							return shapeText(n).trimEnd();
						})
						.filter(Boolean)
						.join('\n');
				});
			}
		} else {
			throw new Error(`unknown action ${a.op}`);
		}
	}
	const state = await readState(page, side, format);
	if (copy) state.copy = copy;
	else if (format === 'pptx' && side === 'ts' && state.textCopy) state.copy = state.textCopy;
	if (lastSelection) state.selection = state.selection || lastSelection;
	return state;
}

async function oracleCopy(fixtureRel, selectionRef) {
	const { readFileSync } = await import('node:fs');
	const up = process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
	const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);
	const { copyRange } = await import(`${up}/src/core/xlsx/edit/clipboard.ts`);
	const { parseRange } = await import(`${up}/src/core/xlsx/address.ts`);
	const bytes = new Uint8Array(readFileSync(join(ROOT, fixtureRel)));
	const workbook = await loadXlsx(bytes);
	const sheet = workbook.activeSheet ?? 0;
	// Multi-range: first range only (matches Rust single-range copy today).
	const first = (selectionRef || 'A1').split(',')[0];
	// Whole-sheet refs from Ctrl+A second press — clip via copyRange.
	const range = parseRange(first.includes(':') ? first : first);
	// selectionRef may be `A1:T50` or `1:1048576`
	let parsed = parseRange(first);
	if (!parsed && /^\d+:\d+$/.test(first)) {
		// whole rows — copyRange handles via parseSelection-like; use parseRange from address
		const { parseSelectionRef } = await import(`${up}/src/core/xlsx/ui/selection.ts`);
		const sel = parseSelectionRef(first, workbook);
		parsed = sel?.ranges?.[0];
	}
	if (!parsed) throw new Error(`oracle: bad selection ${selectionRef}`);
	return copyRange(workbook, sheet, parsed).tsv ?? '';
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
				viewport: { width: (c.width || 800) + 40, height: (c.height || 480) + 40 },
				deviceScaleFactor: 1,
			});
			try {
				const tsPage = await ctx.newPage();
				const rustPage = await ctx.newPage();
				await tsPage.goto(viewerUrl(server.url, c, 'ts'), {
					waitUntil: 'domcontentloaded',
					timeout: 60_000,
				});
				await waitReady(tsPage);
				await rustPage.goto(viewerUrl(server.url, c, 'rust'), {
					waitUntil: 'domcontentloaded',
					timeout: 60_000,
				});
				await waitReady(rustPage);

				const ts = await runActions(tsPage, 'ts', c.format, c.actions || [], c.fixture);
				const rust = await runActions(
					rustPage,
					'rust',
					c.format,
					c.actions || [],
					c.fixture,
				);

				const selOk = !want.has('selection') || rust.selection === ts.selection;
				const hlOk =
					!want.has('highlight') ||
					rectsWithin(rust.highlight, ts.highlight, CARET_TOLERANCE_PX);
				const caretOk =
					!want.has('caret') || rectsWithin(rust.caret, ts.caret, CARET_TOLERANCE_PX);
				const copyOk =
					!want.has('copy') ||
					normalizeCopy(rust.copy, c.format) === normalizeCopy(ts.copy, c.format);
				const ok = selOk && hlOk && caretOk && copyOk;
				if (!ok) failed++;
				rows.push({
					case: c.id,
					format: c.format,
					kind: 'live',
					ok,
					selOk,
					hlOk,
					caretOk,
					copyOk,
					rust: {
						selection: rust.selection,
						copy: normalizeCopy(rust.copy, c.format).slice(0, 120),
						highlight: rust.highlight,
						caret: rust.caret,
					},
					ts: {
						selection: ts.selection,
						copy: normalizeCopy(ts.copy, c.format).slice(0, 120),
						highlight: ts.highlight,
						caret: ts.caret,
					},
				});
				writeFileSync(
					join(OUT, `${c.id}.json`),
					JSON.stringify({ rust, ts, ok }, null, 2),
				);
			} catch (e) {
				failed++;
				rows.push({
					case: c.id,
					format: c.format,
					kind: 'live',
					ok: false,
					error: String(e && e.stack ? e.stack : e).slice(0, 400),
					selOk: false,
					hlOk: false,
					caretOk: false,
					copyOk: false,
					rust: {},
					ts: {},
				});
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
			if (r.error) return `  ${tag} ${r.case} ERROR ${r.error.split('\n')[0]}`;
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
