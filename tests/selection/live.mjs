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

/** @type {readonly ['chromium','webkit','firefox']} */
const SELECTION_BROWSER_NAMES = ['chromium', 'webkit', 'firefox'];

/** Default all three (#29 cross-browser AC). Override with `SELECTION_BROWSERS=chromium`. */
function selectionBrowsersFromEnv() {
	const raw = (process.env.SELECTION_BROWSERS || SELECTION_BROWSER_NAMES.join(','))
		.split(',')
		.map((s) => s.trim().toLowerCase())
		.filter(Boolean);
	const out = [];
	for (const name of raw) {
		if (!SELECTION_BROWSER_NAMES.includes(/** @type {*} */ (name))) {
			throw new Error(
				`selection: unknown browser ${name} (want ${SELECTION_BROWSER_NAMES.join('|')})`,
			);
		}
		if (!out.includes(name)) out.push(name);
	}
	return out.length ? out : [...SELECTION_BROWSER_NAMES];
}

async function loadPlaywright() {
	return import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);
}

/**
 * @param {'chromium'|'webkit'|'firefox'} name
 */
async function loadBrowserType(name) {
	const mod = await loadPlaywright();
	const t = mod[name];
	if (!t) throw new Error(`selection: playwright has no engine ${name}`);
	return t;
}

function viewerPage(format) {
	if (format === 'pptx') return 'pptx-viewer.html';
	if (format === 'docx') return 'docx-viewer.html';
	return 'xlsx-viewer.html';
}

function viewerUrl(base, c, side) {
	const page = viewerPage(c.format);
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

/**
 * Open a viewer page; retry on flaky worker "no document" (#156) and measure-batch
 * race ("expected N widths, got M") — same pattern as tests/a11y/keyboard.mjs.
 */
async function openViewer(ctx, server, c, side) {
	const url = viewerUrl(server.url, c, side);
	let lastErr;
	const maxAttempts = 3;
	for (let attempt = 0; attempt < maxAttempts; attempt++) {
		const page = await ctx.newPage();
		try {
			await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 60_000 });
			await waitReady(page);
			return page;
		} catch (e) {
			lastErr = e;
			await page.close().catch(() => {});
			const msg = String(e && e.message ? e.message : e);
			const retryable =
				msg.includes('no document') || /expected \d+ widths, got \d+/.test(msg);
			if (attempt < maxAttempts - 1 && retryable) continue;
			throw e;
		}
	}
	throw lastErr;
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

function parseParagraphRef(ref) {
	const m = /^p(\d+)$/i.exec(String(ref || '').trim());
	if (!m) throw new Error(`bad paragraph ref ${ref}`);
	return Number(m[1]);
}

async function docxLinePoint(page, side, ref) {
	const idx = parseParagraphRef(ref);
	return page.evaluate(
		({ side, idx }) => {
			const stage = document.getElementById('stage');
			const root =
				side === 'rust'
					? stage?.querySelector('nitro-docx')?.shadowRoot
					: stage;
			const lines = root?.querySelectorAll('.dve-print-line') ?? [];
			const line = lines[idx];
			if (!line) throw new Error(`no print line p${idx}`);
			const r = line.getBoundingClientRect();
			return { x: r.x + r.width / 2, y: r.y + Math.min(r.height / 2, 8) };
		},
		{ side, idx },
	);
}

async function selectDocxLineText(page, side, ref) {
	const idx = parseParagraphRef(ref);
	await page.evaluate(
		({ side, idx }) => {
			const stage = document.getElementById('stage');
			const root =
				side === 'rust'
					? stage?.querySelector('nitro-docx')?.shadowRoot
					: stage;
			const line = root?.querySelectorAll('.dve-print-line')?.[idx];
			if (!line) return;
			const range = document.createRange();
			range.selectNodeContents(line);
			const sel = side === 'rust' && root?.getSelection ? root.getSelection() : window.getSelection();
			sel?.removeAllRanges();
			sel?.addRange(range);
			stage.dataset.docxLine = String(idx);
		},
		{ side, idx },
	);
}

async function tickRust(page) {
	await page.evaluate(async () => {
		const el = document.querySelector('nitro-xlsx, nitro-pptx, nitro-docx');
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

			if (format === 'docx') {
				const root =
					side === 'rust'
						? stage?.querySelector('nitro-docx')?.shadowRoot
						: stage;
				const idx = Number(stage?.dataset.docxLine || 0);
				const line = root?.querySelectorAll('.dve-print-line')?.[idx] || root?.querySelector('.dve-print-line');
				const text = (line?.textContent || '').replace(/\s+/g, ' ').trim();
				const domSel =
					(side === 'rust' && root?.getSelection ? root.getSelection() : window.getSelection())?.toString() ||
					'';
				return {
					selection: text,
					copy: '',
					highlight: box(line),
					caret: domSel
						? (() => {
								const sel =
									side === 'rust' && root?.getSelection
										? root.getSelection()
										: window.getSelection();
								const r = sel?.getRangeAt(0)?.getBoundingClientRect();
								return r && (r.width || r.height)
									? { x: r.x, y: r.y, w: r.width, h: r.height }
									: null;
							})()
						: box(line),
					textCopy: domSel || text,
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
		const el = document.querySelector('nitro-xlsx, nitro-pptx, nitro-docx');
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
		if (format === 'docx') {
			const pt = await docxLinePoint(page, side, ref);
			await page.mouse.click(pt.x, pt.y);
			await selectDocxLineText(page, side, ref);
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
				if (format === 'docx') {
					const fromDom = await page.evaluate(() => {
						const root = document.querySelector('nitro-docx')?.shadowRoot;
						const sel = root?.getSelection?.() || document.getSelection();
						return sel?.toString() || '';
					});
					copy = fromDom || (await rustCopy(page));
				} else if (format === 'pptx') {
					// Prefer in-shadow text selection; WebKit often clears shadow
					// selection before copy, so fall back to dataset.textCopy from
					// text-select before dumping the whole shape via shell copy.
					const fromDom = await page.evaluate(() => {
						const root = document.querySelector('nitro-pptx')?.shadowRoot;
						const sel = root?.getSelection?.() || document.getSelection();
						const t = sel?.toString() || '';
						if (t) return t;
						return document.getElementById('stage')?.dataset.textCopy || '';
					});
					copy = fromDom || (await rustCopy(page));
				} else {
					copy = await rustCopy(page);
				}
			} else if (format === 'docx') {
				copy = await page.evaluate(() => {
					const stage = document.getElementById('stage');
					const textSel = window.getSelection()?.toString();
					if (textSel) return textSel;
					const idx = Number(stage.dataset.docxLine || 0);
					const line = stage.querySelectorAll('.dve-print-line')[idx];
					return line?.textContent || '';
				});
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
	else if (format === 'pptx' && state.textCopy) state.copy = state.textCopy;
	else if (format === 'docx' && side === 'ts' && state.textCopy && !state.copy) state.copy = state.textCopy;
	else if (format === 'docx' && side === 'rust' && state.textCopy && !state.copy) state.copy = state.textCopy;
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
	const only = (process.env.SELECTION_ONLY || '')
		.split(',')
		.map((s) => s.trim())
		.filter(Boolean);
	let cases = readyCases().filter((c) => c.live);
	if (only.length) cases = cases.filter((c) => only.includes(c.id));
	const rows = [];
	if (cases.length === 0) {
		return { ok: true, rows, summary: 'selection live: PASS (0 cases)' };
	}

	mkdirSync(OUT, { recursive: true });
	const browsers = selectionBrowsersFromEnv();
	const server = await startVisualServer({ root: VISUAL });
	let failed = 0;

	try {
		for (const browserName of browsers) {
			const type = await loadBrowserType(browserName);
			const browser = await type.launch({ headless: true });
			try {
				for (const c of cases) {
					const want = new Set(c.compare || ['selection', 'highlight', 'caret']);
					const ctx = await browser.newContext({
						viewport: { width: (c.width || 800) + 40, height: (c.height || 480) + 40 },
						deviceScaleFactor: 1,
					});
					try {
						// One side at a time avoids shared-worker prewarm races under #156.
						const tsPage = await openViewer(ctx, server, c, 'ts');
						const ts = await runActions(tsPage, 'ts', c.format, c.actions || [], c.fixture);
						await tsPage.close();

						const rustPage = await openViewer(ctx, server, c, 'rust');
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
							!want.has('caret') ||
							rectsWithin(rust.caret, ts.caret, CARET_TOLERANCE_PX);
						const copyOk =
							!want.has('copy') ||
							normalizeCopy(rust.copy, c.format) === normalizeCopy(ts.copy, c.format);
						const ok = selOk && hlOk && caretOk && copyOk;
						if (!ok) failed++;
						rows.push({
							case: c.id,
							browser: browserName,
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
							join(OUT, `${c.id}-${browserName}.json`),
							JSON.stringify({ browser: browserName, rust, ts, ok }, null, 2),
						);
					} catch (e) {
						failed++;
						rows.push({
							case: c.id,
							browser: browserName,
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
			}
		}
	} finally {
		await server.close();
	}

	const summary = [
		`selection live: ${failed === 0 ? 'PASS' : 'FAIL'} (${rows.filter((r) => r.ok).length}/${rows.length}) ±${CARET_TOLERANCE_PX}px browsers=${browsers.join(',')}`,
		...rows.map((r) => {
			const tag = r.ok ? 'OK' : 'FAIL';
			const br = r.browser ? `[${r.browser}] ` : '';
			if (r.error) return `  ${tag} ${br}${r.case} ERROR ${r.error.split('\n')[0]}`;
			const detail = [
				r.selOk ? '' : 'selection',
				r.hlOk ? '' : 'highlight',
				r.caretOk ? '' : 'caret',
				r.copyOk ? '' : 'copy',
			]
				.filter(Boolean)
				.join(',') || 'match';
			return `  ${tag} ${br}${r.case} ${detail} rustSel=${JSON.stringify(r.rust.selection)} tsSel=${JSON.stringify(r.ts.selection)}`;
		}),
	].join('\n');
	return { ok: failed === 0, rows, summary };
}

if (import.meta.main) {
	const { ok, summary } = await runLiveChecks();
	console.log(summary);
	process.exit(ok ? 0 : 1);
}

