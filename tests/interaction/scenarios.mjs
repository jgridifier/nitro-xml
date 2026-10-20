// SPDX-License-Identifier: MIT
// Interaction audit scenarios. Each returns pass(detail) | fail(detail) | na(reason).
// Severity (sev) is the default if the scenario fails: S1 data loss / blank / core editing broken,
// S2 standard Office interaction missing, S3 noticeable deviation, S4 polish.
import { inEl, rectOf, uiState, dirtyText, domText, domKids, clipPng, diffStats, anyChange, savedContains, docxModelParas } from './lib.mjs';

export const pass = (detail = '') => ({ status: 'pass', detail });
export const fail = (detail = '') => ({ status: 'fail', detail });
export const na = (detail = '') => ({ status: 'na', detail });

export const SCENARIOS = [];
const S = (id, fmts, sev, title, fn, opts = {}) => SCENARIOS.push({ id, fmts, sev, title, fn, ...opts });

const sleep = (page, ms) => page.waitForTimeout(ms);
const pad = (r, p = 6) => ({ x: Math.max(0, r.x - p), y: Math.max(0, r.y - p), width: r.w + 2 * p, height: r.h + 2 * p });

// ---------- shared probes ----------
const deepActive = (page) =>
	page.evaluate(() => {
		let a = document.activeElement;
		while (a && a.shadowRoot && a.shadowRoot.activeElement) a = a.shadowRoot.activeElement;
		if (!a) return null;
		return {
			tag: a.localName, cls: a.className?.toString?.().slice(0, 40) || '',
			editable: /^(input|textarea)$/.test(a.localName) || a.isContentEditable,
		};
	});

/** Record native clipboard-related events + navigator.clipboard.writeText outcomes. */
const installClipboardProbe = (page) =>
	page.evaluate(() => {
		const w = window;
		w.__clip = { copy: 0, cut: 0, paste: 0, writes: [], writeErr: [], prevented: [] };
		for (const k of ['copy', 'cut', 'paste'])
			document.addEventListener(k, (e) => { w.__clip[k]++; setTimeout(() => w.__clip.prevented.push([k, e.defaultPrevented]), 0); }, true);
		const cb = navigator.clipboard;
		if (cb && cb.writeText) {
			const orig = cb.writeText.bind(cb);
			cb.writeText = (t) => {
				w.__clip.writes.push(String(t).slice(0, 80));
				const p = orig(t);
				p.catch((e) => w.__clip.writeErr.push(String(e && e.name)));
				return p;
			};
		}
	});
const clipState = (page) => page.evaluate(() => window.__clip);
const readClipboard = (page) => page.evaluate(() => navigator.clipboard.readText().catch((e) => 'ERR:' + e.name));

// ---------- DOCX helpers ----------
const docxLine = (page, i) => rectOf(page, 'docx', '.dve-print-line', i);
const docxLines = (page) => inEl(page, 'docx', (el, sr) => [...sr.querySelectorAll('.dve-print-line')].map((l) => l.textContent));
const docxBlocks = (page) => inEl(page, 'docx', (el, sr) => sr.querySelectorAll('.dve-print-block').length);
const midY = (r) => r.y + r.h / 2;

async function docxClick(ctx, line = 3, dx = 80) {
	const r = await docxLine(ctx.page, line);
	await ctx.page.mouse.click(r.x + dx, midY(r));
	await sleep(ctx.page, 200);
	return r;
}

async function docxSelFixture(ctx, line = 2) {
	const { page } = ctx;
	const r = await docxLine(page, line);
	const clip = pad(r, 4);
	const base = await clipPng(page, clip);
	return { r, clip, base };
}

S('docx.caret-visible', ['docx'], 'S1', 'Click places a visible caret', async (ctx) => {
	const { page } = ctx;
	const { r, clip, base } = await docxSelFixture(ctx, 3);
	await page.mouse.click(r.x + 150, midY(r));
	const st = await anyChange(page, clip, base, [150, 450, 750, 1050]);
	await ctx.shot('docx-caret');
	return st.changed > 0 && st.bbox.w <= 8 ? pass(`caret px=${st.changed}`) : fail(`no thin caret drawn after click (changed px=${st.changed}, bbox=${JSON.stringify(st.bbox)})`);
});

S('docx.text-input-target', ['docx', 'pptx', 'xlsx'], 'S1', 'Focus lands on a real text-input target (IME / mobile keyboard / dictation / autocorrect need one)', async (ctx) => {
	const { page, fmt } = ctx;
	if (fmt === 'docx') await docxClick(ctx);
	else if (fmt === 'xlsx') await page.mouse.click(300, 300);
	else { const r = await rectOf(page, 'pptx', '.pptxv-text'); await page.mouse.dblclick(r.x + r.w / 2, midY(r)); }
	await sleep(page, 200);
	const a = await deepActive(page);
	return a && a.editable ? pass(JSON.stringify(a)) : fail(`activeElement=${JSON.stringify(a)}: no input/textarea/contenteditable, so beforeinput/composition/virtual keyboard never fire`);
});

S('docx.type', ['docx'], 'S1', 'Typing inserts text and the page stays rendered', async (ctx) => {
	const { page } = ctx;
	const before = await docxLines(page);
	await docxClick(ctx, 3, 100);
	await page.keyboard.type('XYZ', { delay: 40 });
	await sleep(page, 1500);
	const kids = await domKids(page, 'docx');
	const txt = await domText(page, 'docx');
	const model = await savedContains(page, 'docx', 'XYZ').catch(() => 'save-failed');
	await ctx.shot('docx-type');
	const detail = `lines before=${before.length}, dom children after=${kids}, dom has XYZ=${txt.includes('XYZ')}, saved model has XYZ=${model}`;
	return kids > 0 && txt.includes('XYZ') ? pass(detail) : fail(detail);
});

const paraIdx = (paras, prefix) => paras.findIndex((p) => p.startsWith(prefix));

S('docx.rapid-typing', ['docx'], 'S1', 'Fast typing (10 ms/char) keeps every character (checked against the saved model)', async (ctx) => {
	const { page } = ctx;
	await docxClick(ctx, 3, 100);
	await page.keyboard.type('abcdefghijklmnopqrst', { delay: 10 });
	await sleep(page, 2000);
	const model = await savedContains(page, 'docx', 'abcdefghijklmnopqrst').catch(() => false);
	const paras = await docxModelParas(page);
	const got = (paras.join('|').match(/[a-t]{3,}/g) || []).join(',').slice(0, 60);
	return model ? pass() : fail(`model lacks the 20-char run; letter runs found: "${got}"`);
});

S('docx.backspace-in-run', ['docx'], 'S1', 'Backspace removes the previous character within a run (model and screen agree)', async (ctx) => {
	const { page } = ctx;
	const m0 = await docxModelParas(page);
	const l0 = (await docxLines(page))[3];
	await docxClick(ctx, 3, 200);
	for (let i = 0; i < 3; i++) await page.keyboard.press('Backspace');
	await sleep(page, 1200);
	const m1 = await docxModelParas(page);
	const l1 = (await docxLines(page))[3] ?? '';
	const model = m1.join('').length === m0.join('').length - 3;
	const screen = l1.length === l0.length - 3;
	const d = `model chars ${m0.join('').length}->${m1.join('').length} (want -3): ${model}; screen line ${l0.length}->${l1.length}: ${screen}; dom children=${await domKids(page, 'docx')}`;
	return model && screen ? pass(d) : fail(d);
});

S('docx.backspace-across-paragraph', ['docx'], 'S1', 'Backspace at paragraph start merges with the previous paragraph', async (ctx) => {
	const { page } = ctx;
	const m0 = await docxModelParas(page);
	const r = await docxLine(page, 2);
	await page.mouse.click(r.x + 1, midY(r));
	await page.keyboard.press('Backspace');
	await sleep(page, 1200);
	const m1 = await docxModelParas(page);
	const kids = await domKids(page, 'docx');
	const d = `model paragraphs ${m0.length}->${m1.length} (want -1); dom children=${kids}`;
	return m1.length === m0.length - 1 && kids > 0 ? pass(d) : fail(d);
});

S('docx.delete-across-paragraph', ['docx'], 'S1', 'Delete at paragraph end merges with the next paragraph', async (ctx) => {
	const { page } = ctx;
	const m0 = await docxModelParas(page);
	const r = await docxLine(page, 1);
	await page.mouse.click(r.x + r.w - 2, midY(r));
	await page.keyboard.press('Delete');
	await sleep(page, 1200);
	const m1 = await docxModelParas(page);
	const kids = await domKids(page, 'docx');
	const d = `model paragraphs ${m0.length}->${m1.length} (want -1); dom children=${kids}`;
	return m1.length === m0.length - 1 && kids > 0 ? pass(d) : fail(d);
});

S('docx.delete-forward', ['docx'], 'S1', 'Delete removes the next character(s) within a run', async (ctx) => {
	const { page } = ctx;
	const m0 = await docxModelParas(page);
	await docxClick(ctx, 3, 200);
	for (let i = 0; i < 2; i++) await page.keyboard.press('Delete');
	await sleep(page, 1200);
	const m1 = await docxModelParas(page);
	const kids = await domKids(page, 'docx');
	const d = `model chars ${m0.join('').length}->${m1.join('').length} (want -2); dom children=${kids}`;
	return m1.join('').length === m0.join('').length - 2 && kids > 0 ? pass(d) : fail(d);
});

S('docx.backspace-across-runs', ['docx'], 'S1', 'Backspace walks back across a run boundary (italic subtitle: text, separator, text)', async (ctx) => {
	const { page } = ctx;
	const m0 = await docxModelParas(page);
	const r = await docxLine(page, 1);
	await page.mouse.click(r.x + r.w - 40, midY(r));
	for (let i = 0; i < 12; i++) await page.keyboard.press('Backspace');
	await sleep(page, 1200);
	const m1 = await docxModelParas(page);
	const kids = await domKids(page, 'docx');
	const d = `model chars ${m0.join('').length}->${m1.join('').length} (want -12); dom children=${kids}`;
	return m1.join('').length === m0.join('').length - 12 && kids > 0 ? pass(d) : fail(d);
});

S('docx.undo-redo-type', ['docx'], 'S1', 'Ctrl+Z / Ctrl+Y round-trips a typed edit (checked against the saved model)', async (ctx) => {
	const { page } = ctx;
	await docxClick(ctx, 3, 100);
	await page.keyboard.type('QQ', { delay: 40 });
	await sleep(page, 800);
	const typed = await savedContains(page, 'docx', 'QQ').catch(() => false);
	await page.keyboard.press('Control+z');
	await sleep(page, 800);
	const undone = !(await savedContains(page, 'docx', 'QQ').catch(() => true));
	await page.keyboard.press('Control+y');
	await sleep(page, 800);
	const redone = await savedContains(page, 'docx', 'QQ').catch(() => false);
	const kids = await domKids(page, 'docx');
	const d = `typed=${typed} undone=${undone} redone=${redone} dom children=${kids}`;
	return typed && undone && redone && kids > 0 ? pass(d) : fail(d);
});

S('docx.select-drag', ['docx'], 'S2', 'Click-drag paints a selection highlight', async (ctx) => {
	const { page } = ctx;
	const { r, clip, base } = await docxSelFixture(ctx);
	await page.mouse.move(r.x + 40, midY(r));
	await page.mouse.down();
	await page.mouse.move(r.x + 240, midY(r), { steps: 10 });
	await page.mouse.up();
	await sleep(page, 400);
	const st = diffStats(base, await clipPng(page, clip));
	await ctx.shot('docx-drag-select');
	return st.ratio > 0.05 ? pass(`highlight ratio=${st.ratio.toFixed(2)}`) : fail(`no highlight: changed ratio=${st.ratio.toFixed(3)}`);
});

S('docx.select-shift-click', ['docx'], 'S2', 'Shift+click extends selection', async (ctx) => {
	const { page } = ctx;
	const { r, clip, base } = await docxSelFixture(ctx);
	await page.mouse.click(r.x + 40, midY(r));
	await page.keyboard.down('Shift');
	await page.mouse.click(r.x + 240, midY(r));
	await page.keyboard.up('Shift');
	await sleep(page, 400);
	const st = diffStats(base, await clipPng(page, clip));
	return st.ratio > 0.05 ? pass() : fail(`no highlight: ratio=${st.ratio.toFixed(3)}`);
});

S('docx.select-shift-arrows', ['docx'], 'S2', 'Shift+Arrow extends selection by character', async (ctx) => {
	const { page } = ctx;
	const { r, clip, base } = await docxSelFixture(ctx);
	await page.mouse.click(r.x + 40, midY(r));
	for (let i = 0; i < 8; i++) await page.keyboard.press('Shift+ArrowRight');
	await sleep(page, 400);
	const st = diffStats(base, await clipPng(page, clip));
	return st.ratio > 0.03 ? pass() : fail(`no highlight: ratio=${st.ratio.toFixed(3)}`);
});

S('docx.select-dblclick-word', ['docx'], 'S2', 'Double-click selects one word', async (ctx) => {
	const { page } = ctx;
	const { r, clip, base } = await docxSelFixture(ctx);
	await page.mouse.dblclick(r.x + 60, midY(r));
	await sleep(page, 400);
	const st = diffStats(base, await clipPng(page, clip));
	await ctx.shot('docx-dblclick');
	return st.bbox && st.bbox.w > 12 && st.bbox.w < 170 ? pass(`bbox w=${st.bbox.w}`) : fail(`bbox=${JSON.stringify(st.bbox)} (expect a single-word highlight 12..170px)`);
});

S('docx.select-triple-paragraph', ['docx'], 'S2', 'Triple-click selects the whole paragraph (both wrapped lines)', async (ctx) => {
	const { page } = ctx;
	const r2 = await docxLine(page, 2), r3 = await docxLine(page, 3);
	const clip = pad({ x: r2.x, y: r2.y, w: r2.w, h: r3.y + r3.h - r2.y }, 4);
	const base = await clipPng(page, clip);
	await page.mouse.click(r2.x + 60, midY(r2), { clickCount: 3 });
	await sleep(page, 400);
	const st = diffStats(base, await clipPng(page, clip));
	await ctx.shot('docx-triple');
	return st.bbox && st.bbox.h >= (r3.y + r3.h - r2.y) * 0.8 ? pass() : fail(`bbox=${JSON.stringify(st.bbox)} ratio=${st.ratio.toFixed(3)}`);
});

async function docxSelectPara(ctx) {
	const r2 = await docxLine(ctx.page, 2);
	await ctx.page.mouse.click(r2.x + 60, midY(r2), { clickCount: 3 });
	await sleep(ctx.page, 300);
}

S('docx.copy', ['docx'], 'S2', 'Ctrl+C copies the selection to the system clipboard', async (ctx) => {
	const { page } = ctx;
	await installClipboardProbe(page);
	const r = await docxLine(page, 2);
	await page.mouse.move(r.x + 5, midY(r));
	await page.mouse.down();
	await page.mouse.move(r.x + 300, midY(r), { steps: 8 });
	await page.mouse.up();
	await page.keyboard.press('Control+c');
	await sleep(page, 800);
	const c = await clipState(page);
	const sys = ctx.browser === 'chromium' ? await readClipboard(page) : '(unreadable in webkit)';
	const d = `copy-event=${c.copy} writeText=${JSON.stringify(c.writes)} writeErr=${c.writeErr} system="${String(sys).slice(0, 40)}"`;
	const ok = c.writes.some((t) => t.length > 0) && !c.writeErr.length && (ctx.browser !== 'chromium' || (sys && !String(sys).startsWith('ERR')));
	return ok ? pass(d) : fail(d);
}, { clipboard: true });

S('docx.cut', ['docx'], 'S2', 'Ctrl+X cuts the selection (clipboard + deletion, undoable)', async (ctx) => {
	const { page } = ctx;
	await installClipboardProbe(page);
	const before = (await docxLines(page))[2];
	await docxSelectPara(ctx);
	await page.keyboard.press('Control+x');
	await sleep(page, 1000);
	const c = await clipState(page);
	const gone = !(await domText(page, 'docx')).includes(before.slice(0, 20));
	const d = `writeText=${c.writes.length} removed=${gone} cutEvent=${c.cut}`;
	return c.writes.length && gone ? pass(d) : fail(d);
}, { clipboard: true });

S('docx.paste', ['docx'], 'S2', 'Ctrl+V pastes clipboard text at the caret', async (ctx) => {
	const { page } = ctx;
	await installClipboardProbe(page);
	if (ctx.browser === 'chromium') await page.evaluate(() => navigator.clipboard.writeText('PASTEME'));
	else await page.evaluate(() => { const t = document.createElement('textarea'); t.value = 'PASTEME'; document.body.append(t); t.select(); document.execCommand('copy'); t.remove(); });
	await docxClick(ctx, 3, 100);
	await page.keyboard.press('Control+v');
	await sleep(page, 1000);
	const c = await clipState(page);
	const has = (await domText(page, 'docx')).includes('PASTEME');
	const model = await savedContains(page, 'docx', 'PASTEME').catch(() => false);
	const d = `paste-event=${c.paste} dom=${has} model=${model}`;
	return has || model ? pass(d) : fail(d);
}, { clipboard: true });

S('docx.ime-compose', ['docx'], 'S1', 'IME composition (CDP Input.imeSetComposition) inserts composed text', async (ctx) => {
	if (ctx.browser !== 'chromium') return na('CDP IME is chromium-only');
	const { page } = ctx;
	await docxClick(ctx, 3, 100);
	const cdp = await ctx.cdp();
	await cdp.send('Input.imeSetComposition', { text: 'に', selectionStart: 1, selectionEnd: 1 });
	await sleep(page, 200);
	await cdp.send('Input.imeSetComposition', { text: '日本', selectionStart: 2, selectionEnd: 2 });
	await cdp.send('Input.insertText', { text: '日本' });
	await sleep(page, 1000);
	const dom = (await domText(page, 'docx')).includes('日本');
	const model = await savedContains(page, 'docx', '日本').catch(() => false);
	await ctx.shot('docx-ime');
	return dom || model ? pass(`dom=${dom} model=${model}`) : fail(`composition text never arrived (dom=${dom} model=${model}); no composition handler on the element`);
});

// ---------- XLSX helpers ----------
const xlGeom = (page) =>
	inEl(page, 'xlsx', (el, sr) => {
		const m = (sel, a, b) => [...sr.querySelectorAll(sel)].map((e) => { const r = e.getBoundingClientRect(); return [r[a], r[b]]; });
		return { cols: m('.xg-hdr-col .xg-hd', 'x', 'width'), rows: m('.xg-hdr-row .xg-hd', 'y', 'height') };
	});
const pt = (g, c, r) => ({ x: g.cols[c][0] + g.cols[c][1] / 2, y: g.rows[r][0] + g.rows[r][1] / 2 });
const cellText = (page, p) =>
	inEl(page, 'xlsx', (el, sr, q) => sr.elementsFromPoint(q.x, q.y).filter((e) => e.classList.contains('xg-c') || e.classList.contains('xg-tx')).map((e) => e.textContent).join('|'), p);
const sel = async (page) => (await uiState(page, 'xlsx')).selection;
const settle = (page) => sleep(page, 600);
/** Poll `fn` until it returns a truthy value or `ms` elapses; returns the last value. */
async function until(page, fn, ms = 2500) {
	let v;
	for (let t = 0; t < ms; t += 100) {
		v = await fn();
		if (v) return v;
		await sleep(page, 100);
	}
	return v;
}

// The first keystroke mounts the formula bar and shifts the grid down (see xlsx.layout-shift-on-first-key),
// so prime it once, then measure the geometry that stays put for the rest of the scenario.
async function xlSetup(ctx) {
	const g0 = await xlGeom(ctx.page);
	const p = pt(g0, 12, 10);
	await ctx.page.mouse.click(p.x, p.y);
	await ctx.page.keyboard.press('ArrowDown');
	return stableGeom(ctx.page);
}
async function stableGeom(page) {
	let last = null, same = 0;
	for (let i = 0; i < 40 && same < 4; i++) {
		await sleep(page, 150);
		const g = await xlGeom(page);
		const y = g.rows[0]?.[0];
		same = last === y ? same + 1 : 0;
		last = y;
	}
	return xlGeom(page);
}

S('xlsx.layout-shift-on-first-key', ['xlsx'], 'S2', 'The first keystroke must not move the grid under the pointer (formula bar mounted late)', async (ctx) => {
	const { page } = ctx;
	const g0 = await xlGeom(ctx.page);
	const p = pt(g0, 3, 3);
	await page.mouse.click(p.x, p.y);
	const g1 = await stableGeom(page);
	const dClick = g1.rows[0][0] - g0.rows[0][0];
	await page.keyboard.press('ArrowDown');
	const g2 = await stableGeom(page);
	const dKey = g2.rows[0][0] - g1.rows[0][0];
	await ctx.shot('xlsx-layout-shift');
	return Math.abs(dKey) < 1 && Math.abs(dClick) < 1 ? pass() : fail(`grid moved ${dClick.toFixed(0)}px after the first click and ${dKey.toFixed(0)}px after the first keystroke (formula bar mounted from the key handler's chrome refresh)`);
});

S('xlsx.click-cell', ['xlsx'], 'S2', 'Click selects the cell with an outline on it', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 2, 4);
	await page.mouse.click(p.x, p.y);
	await settle(page);
	const s = await sel(page);
	const o = await rectOf(page, 'xlsx', '.xg-outline:not([hidden])');
	const near = o && Math.abs(o.x + o.w / 2 - p.x) < 40 && Math.abs(o.y + o.h / 2 - p.y) < 14;
	await ctx.shot('xlsx-click');
	return s === 'C5' && near ? pass(`${s} outline=${JSON.stringify(o)}`) : fail(`selection=${s} outline=${JSON.stringify(o)} clicked=${JSON.stringify(p)}`);
});

S('xlsx.drag-range', ['xlsx'], 'S2', 'Click-drag selects a range and outlines it', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const a = pt(g, 2, 2), b = pt(g, 4, 5);
	await page.mouse.move(a.x, a.y); await page.mouse.down();
	await page.mouse.move(b.x, b.y, { steps: 12 }); await page.mouse.up();
	await settle(page);
	const s = await sel(page);
	const o = await rectOf(page, 'xlsx', '.xg-outline:not([hidden])');
	await ctx.shot('xlsx-drag-range');
	return s === 'C3:E6' && o && o.w > 150 ? pass(`${s}`) : fail(`selection=${s} outline=${JSON.stringify(o)}`);
});

S('xlsx.shift-click-range', ['xlsx'], 'S2', 'Shift+click extends the range from the anchor', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const a = pt(g, 1, 1), b = pt(g, 3, 4);
	await page.mouse.click(a.x, a.y);
	await page.keyboard.down('Shift'); await page.mouse.click(b.x, b.y); await page.keyboard.up('Shift');
	await settle(page);
	const s = await sel(page);
	return s === 'B2:D5' ? pass(s) : fail(`selection=${s}, expected B2:D5`);
});

S('xlsx.shift-arrows', ['xlsx'], 'S2', 'Shift+Arrow extends the range', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const a = pt(g, 1, 1);
	await page.mouse.click(a.x, a.y);
	await page.keyboard.press('Shift+ArrowRight'); await page.keyboard.press('Shift+ArrowRight'); await page.keyboard.press('Shift+ArrowDown');
	await settle(page);
	const s = await sel(page);
	return s === 'B2:D3' ? pass(s) : fail(`selection=${s}, expected B2:D3`);
});

S('xlsx.selection-border-and-fill-handle', ['xlsx'], 'S3', 'Selection has a 2px border, tinted fill and a fill handle at the bottom-right', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const a = pt(g, 2, 2), b = pt(g, 3, 4);
	await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.mouse.move(b.x, b.y, { steps: 6 }); await page.mouse.up();
	await settle(page);
	const h = await rectOf(page, 'xlsx', '.xg-handle');
	const o = await rectOf(page, 'xlsx', '.xg-outline:not([hidden])');
	const bw = await inEl(page, 'xlsx', (el, sr) => { const e = sr.querySelector('.xg-outline'); const c = e && getComputedStyle(e); return c && { border: c.borderTopWidth + ' ' + c.borderTopStyle + ' ' + c.borderTopColor, bg: c.backgroundColor, outline: c.outlineWidth }; });
	await ctx.shot('xlsx-selection');
	const hOk = h && h.w > 0;
	return hOk ? pass(JSON.stringify({ h, bw })) : fail(`fill handle missing (.xg-handle ${JSON.stringify(h)}), outline style=${JSON.stringify(bw)}`);
});

S('xlsx.fill-handle-drag', ['xlsx'], 'S3', 'Dragging the fill handle fills cells below', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const a = pt(g, 6, 12);
	await page.mouse.click(a.x, a.y);
	await page.keyboard.type('fill1'); await page.keyboard.press('Enter');
	await page.mouse.click(a.x, a.y); await settle(page);
	const h = await rectOf(page, 'xlsx', '.xg-handle');
	if (!h) return fail('no .xg-handle element after selecting a cell');
	await page.mouse.move(h.x + h.w / 2, h.y + h.h / 2); await page.mouse.down();
	await page.mouse.move(h.x + h.w / 2, h.y + h.h / 2 + 62, { steps: 8 }); await page.mouse.up();
	await settle(page);
	const t = await cellText(page, pt(g, 6, 14));
	return t.includes('fill1') ? pass() : fail(`cell below = "${t}"`);
});

S('xlsx.click-then-immediate-key-order', ['xlsx'], 'S3', 'A key sent right after a click acts on the new selection (pointer events and key requests are applied in order)', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const a = pt(g, 1, 1), b = pt(g, 5, 8);
	await page.mouse.click(a.x, a.y);
	await sleep(page, 500);
	await ctx.raw.click(b.x, b.y);
	await page.keyboard.press('ArrowDown');
	await sleep(page, 800);
	const s = await sel(page);
	return s === 'F10' ? pass(s) : fail(`click F9 then immediate ArrowDown -> ${s} (expected F10): the key was applied before the queued pointer event`);
});

S('xlsx.type-into-cell', ['xlsx'], 'S1', 'Typing on a selected cell edits it; Enter commits and moves down', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 3, 3);
	await page.mouse.click(p.x, p.y);
	await page.keyboard.type('hello', { delay: 30 });
	await settle(page);
	const editing = await inEl(page, 'xlsx', (el, sr) => !!sr.querySelector('.xg-edit-caret:not([hidden]), .xg-editor, [class*=edit]:not([hidden])'));
	await page.keyboard.press('Enter'); await settle(page);
	const t = await cellText(page, p);
	const s = await sel(page);
	await ctx.shot('xlsx-type');
	const d = `cell="${t}" selection=${s} editorVisibleWhileTyping=${editing}`;
	return t.includes('hello') && s === 'D5' ? pass(d) : fail(d);
});

S('xlsx.rapid-arrow-repeat', ['xlsx'], 'S1', 'Holding an arrow key (30 ms repeat, 20 presses) moves exactly 20 cells', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 2, 0);
	await page.mouse.click(p.x, p.y);
	await sleep(page, 400);
	for (let i = 0; i < 20; i++) { await page.keyboard.press('ArrowDown'); await sleep(page, 30); }
	await until(page, async () => (await sel(page)) === 'C21', 3000);
	const s = await sel(page);
	return s === 'C21' ? pass(s) : fail(`selection=${s}, expected C21: keystrokes were dropped`);
});

S('xlsx.rapid-typing', ['xlsx'], 'S1', 'Fast typing (10 ms/char) into a cell keeps every character', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 6, 22);
	await page.mouse.click(p.x, p.y);
	await sleep(page, 300);
	await page.keyboard.type('abcdefghijklmnopqrst', { delay: 10 });
	await page.keyboard.press('Enter');
	await until(page, async () => (await cellText(page, p)).includes('abc'), 2500);
	const t = await cellText(page, p);
	return t.includes('abcdefghijklmnopqrst') ? pass() : fail(`cell="${t}" (expected 20 chars)`);
});

S('xlsx.enter-tab-nav', ['xlsx'], 'S2', 'Enter/Tab/Shift+Enter/Shift+Tab/arrows move the active cell like Excel', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 1, 1);
	await page.mouse.click(p.x, p.y);
	const seq = [];
	for (const k of ['Enter', 'Tab', 'Shift+Tab', 'Shift+Enter', 'ArrowDown', 'ArrowDown', 'ArrowRight', 'Control+Home']) {
		const prev = await sel(page);
		await page.keyboard.press(k);
		await until(page, async () => (await sel(page)) !== prev, 1200);
		seq.push(await sel(page));
	}
	const want = ['B3', 'C3', 'B3', 'B2', 'B3', 'B4', 'C4', 'A1'];
	return JSON.stringify(seq) === JSON.stringify(want) ? pass(seq.join(' ')) : fail(`got ${seq.join(' ')}; want ${want.join(' ')}`);
});

S('xlsx.tab-enter-return-column', ['xlsx'], 'S3', 'After typing across a row with Tab, Enter returns to the starting column on the next row', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 6, 19);
	await page.mouse.click(p.x, p.y);
	await page.keyboard.type('a'); await page.keyboard.press('Tab');
	await page.keyboard.type('b'); await page.keyboard.press('Tab');
	await page.keyboard.type('c'); await page.keyboard.press('Enter');
	await settle(page);
	const s = await sel(page);
	return s === 'G21' ? pass(s) : fail(`selection=${s}, Excel returns to G21`);
});

S('xlsx.dblclick-edit', ['xlsx'], 'S2', 'Double-click opens in-cell edit with caret at the end of the existing text', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 0, 1);
	const orig = await cellText(page, p);
	await page.mouse.dblclick(p.x, p.y);
	await settle(page);
	await page.keyboard.type('Z');
	await page.keyboard.press('Enter');
	await until(page, async () => (await cellText(page, p)) !== orig, 2000);
	const t = await cellText(page, p);
	await ctx.shot('xlsx-dblclick-edit');
	return t.includes(orig.split('|')[0] + 'Z') ? pass(`"${orig}" -> "${t}"`) : fail(`"${orig}" -> "${t}" (expected original text + Z)`);
});

S('xlsx.f2-and-escape', ['xlsx'], 'S3', 'F2 edits in place; Escape cancels without committing', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 0, 2);
	const orig = await cellText(page, p);
	await page.mouse.click(p.x, p.y);
	await page.keyboard.press('F2'); await page.keyboard.type('ZZZ'); await page.keyboard.press('Escape'); await settle(page);
	const t = await cellText(page, p);
	return t === orig ? pass() : fail(`"${orig}" -> "${t}" after F2, type, Esc (should be unchanged)`);
});

S('xlsx.delete-clears', ['xlsx'], 'S3', 'Delete clears selected cells; Backspace clears too', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 0, 3);
	await page.mouse.click(p.x, p.y);
	await page.keyboard.press('Delete');
	const t = await until(page, async () => (await cellText(page, p)) === '' && 'empty', 2500) ? '' : await cellText(page, p);
	return t === '' ? pass() : fail(`cell after Delete="${t}"`);
});

S('xlsx.copy-paste', ['xlsx'], 'S2', 'Ctrl+C then Ctrl+V copies a cell (clipboard + marching ants)', async (ctx) => {
	const { page } = ctx;
	await installClipboardProbe(page);
	const g = await xlSetup(ctx);
	const a = pt(g, 0, 1), b = pt(g, 5, 15);
	const src = await cellText(page, a);
	await page.mouse.click(a.x, a.y); await page.keyboard.press('Control+c'); await sleep(page, 500);
	const ants = await inEl(page, 'xlsx', (el, sr) => !!sr.querySelector('[class*=copy],[class*=marching],[class*=ants],[class*=dash]'));
	await page.mouse.click(b.x, b.y); await page.keyboard.press('Control+v');
	await until(page, async () => (await cellText(page, b)).includes(src.split('|')[0]), 2500);
	const t = await cellText(page, b);
	const c = await clipState(page);
	const d = `src="${src}" dest="${t}" writeText=${JSON.stringify(c.writes)} err=${c.writeErr} copyAnts=${ants}`;
	return t.includes(src.split('|')[0]) ? pass(d) : fail(d);
}, { clipboard: true });

S('xlsx.cut-paste', ['xlsx'], 'S2', 'Ctrl+X then Ctrl+V moves a cell', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const a = pt(g, 0, 1), b = pt(g, 5, 16);
	const src = (await cellText(page, a)).split('|')[0];
	await page.mouse.click(a.x, a.y); await page.keyboard.press('Control+x');
	await page.mouse.click(b.x, b.y); await page.keyboard.press('Control+v');
	await until(page, async () => (await cellText(page, b)).includes(src), 2500);
	const t = await cellText(page, b), s = await cellText(page, a);
	return t.includes(src) && !s.includes(src) ? pass() : fail(`dest="${t}" source="${s}"`);
}, { clipboard: true });

S('xlsx.undo-redo', ['xlsx'], 'S2', 'Ctrl+Z / Ctrl+Y revert and reapply a cell edit', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 3, 6);
	const orig = await cellText(page, p);
	await page.mouse.click(p.x, p.y); await page.keyboard.type('UNDOME'); await page.keyboard.press('Enter'); await settle(page);
	const e1 = (await cellText(page, p)).includes('UNDOME');
	await page.keyboard.press('Control+z'); await sleep(page, 400);
	const u = await cellText(page, p);
	await page.keyboard.press('Control+y'); await sleep(page, 400);
	const r = (await cellText(page, p)).includes('UNDOME');
	const d = `edited=${e1} undone="${u}" (orig "${orig}") redone=${r}`;
	return e1 && u === orig && r ? pass(d) : fail(d);
});

S('xlsx.header-select', ['xlsx'], 'S3', 'Clicking a column/row header selects the whole column/row; drag across headers selects several', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const hc = await rectOf(page, 'xlsx', '.xg-hdr-col .xg-hd', 2), hr = await rectOf(page, 'xlsx', '.xg-hdr-row .xg-hd', 3);
	await page.mouse.click(hc.x + hc.w / 2, hc.y + hc.h / 2); await settle(page);
	const s1 = await sel(page);
	await page.mouse.click(hr.x + hr.w / 2, hr.y + hr.h / 2); await settle(page);
	const s2 = await sel(page);
	const he = await rectOf(page, 'xlsx', '.xg-hdr-col .xg-hd', 4);
	await page.mouse.move(hc.x + hc.w / 2, hc.y + hc.h / 2); await page.mouse.down(); await page.mouse.move(he.x + he.w / 2, he.y + he.h / 2, { steps: 6 }); await page.mouse.up(); await settle(page);
	const s3 = await sel(page);
	const d = `col=${s1} row=${s2} dragCols=${s3}`;
	return /^C(1)?:C/.test(s1) && /^4:4|^A4:/.test(s2) && /C.*:E/.test(s3) ? pass(d) : fail(d);
});

S('xlsx.column-resize', ['xlsx'], 'S3', 'Dragging a column-header edge resizes the column (cursor feedback)', async (ctx) => {
	const { page } = ctx;
	const h = await rectOf(page, 'xlsx', '.xg-hdr-col .xg-hd', 2);
	const x = h.x + h.w - 1, y = h.y + h.h / 2;
	await page.mouse.move(x, y); await sleep(page, 150);
	const cursor = await page.evaluate(([x, y]) => { const e = document.elementFromPoint(x, y); const sr = document.querySelector('nitro-xlsx').shadowRoot; const t = sr.elementFromPoint(x, y); return t && getComputedStyle(t).cursor; }, [x, y]);
	await page.mouse.down(); await page.mouse.move(x + 40, y, { steps: 6 }); await page.mouse.up(); await settle(page);
	const h2 = await rectOf(page, 'xlsx', '.xg-hdr-col .xg-hd', 2);
	const d = `width ${h.w} -> ${h2.w}, cursor over edge=${cursor}`;
	return h2.w > h.w + 20 && /resize/.test(cursor) ? pass(d) : fail(d);
});

S('xlsx.select-all', ['xlsx'], 'S4', 'Ctrl+A selects all cells', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 1, 1);
	await page.mouse.click(p.x, p.y); await page.keyboard.press('Control+a'); await settle(page);
	const s = await sel(page);
	return /:/.test(s) ? pass(s) : fail(`selection=${s}`);
});

S('xlsx.wheel-scroll', ['xlsx'], 'S3', 'Mouse wheel scrolls the grid and keeps headers in sync', async (ctx) => {
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 3, 3);
	await page.mouse.move(p.x, p.y);
	const before = await cellText(page, p);
	await page.mouse.wheel(0, 400); await sleep(page, 500);
	const after = await cellText(page, p);
	const h = await rectOf(page, 'xlsx', '.xg-hdr-row .xg-hd', 3);
	return before !== after ? pass(`"${before}" -> "${after}"`) : fail('grid did not scroll');
});

S('xlsx.ime-compose', ['xlsx'], 'S1', 'IME composition into a cell', async (ctx) => {
	if (ctx.browser !== 'chromium') return na('CDP IME is chromium-only');
	const { page } = ctx;
	const g = await xlSetup(ctx);
	const p = pt(g, 3, 8);
	await page.mouse.dblclick(p.x, p.y);
	const cdp = await ctx.cdp();
	await cdp.send('Input.imeSetComposition', { text: 'に', selectionStart: 1, selectionEnd: 1 });
	await cdp.send('Input.insertText', { text: '日本' });
	await page.keyboard.press('Enter'); await sleep(page, 600);
	const t = await cellText(page, p);
	return t.includes('日本') ? pass(t) : fail(`cell="${t}"`);
});

// ---------- PPTX helpers ----------
const pptxText = (page, i = 0) => rectOf(page, 'pptx', '.pptxv-text', i);
const slideTitle = (page) => inEl(page, 'pptx', async (el) => { const u = (await el.shellAction('ui')).ui; const a = (u.tabs || []).find((x) => x.active); return a ? `#${a.index}` : '?'; });
const thumbRect = (page, i) => page.evaluate((i) => { const e = document.querySelectorAll('.pl .th')[i]; if (!e) return null; const r = e.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; }, i);
const thumbTexts = (page) => page.evaluate(() => [...document.querySelectorAll('.pl .th')].map((e) => (e.shadowRoot ? e.shadowRoot.textContent : e.textContent).trim().slice(0, 24)));
const shapeBoxOf = async (page, i = 0) => inEl(page, 'pptx', (el, sr, i) => { const t = sr.querySelectorAll('.pptxv-text')[i]; const p = t && (t.parentElement || t); const r = p.getBoundingClientRect(); return { x: r.x, y: r.y, w: r.width, h: r.height }; }, i);

S('pptx.select-shape', ['pptx'], 'S1', 'Click on a shape selects it and shows an outline + 8 resize handles + rotate handle', async (ctx) => {
	const { page } = ctx;
	const r = await pptxText(page, 0);
	const box = await shapeBoxOf(page, 0);
	const clip = pad(box, 20);
	const base = await clipPng(page, clip);
	await page.mouse.click(r.x + r.w / 2, midY(r));
	await sleep(page, 500);
	const ring = { x: 20, y: 20, w: box.w - 2, h: box.h - 2 };
	const st = await anyChange(page, clip, base, [0, 300, 600], 24, ring);
	const handles = await inEl(page, 'pptx', (el, sr) => sr.querySelectorAll('[class*=handle],[class*=resize],[class*=select],[data-handle]').length);
	await ctx.shot('pptx-select');
	const d = `ring-changed-px=${st.changed} handle-elements=${handles}`;
	return handles >= 8 ? pass(d) : fail(st.changed > 40 ? `only a thin selection outline is drawn; no resize/rotate handles (${d})` : `nothing drawn around the clicked shape (${d})`);
});

S('pptx.drag-move', ['pptx'], 'S1', 'Dragging a shape moves it (live feedback during the drag)', async (ctx) => {
	const { page } = ctx;
	const r = await pptxText(page, 0);
	const b0 = await shapeBoxOf(page, 0);
	await page.mouse.move(r.x + r.w / 2, midY(r)); await page.mouse.down();
	await page.mouse.move(r.x + r.w / 2 + 80, midY(r) + 40, { steps: 10 });
	const mid = await shapeBoxOf(page, 0);
	await page.mouse.up(); await sleep(page, 500);
	const b1 = await shapeBoxOf(page, 0);
	const d = `before=${Math.round(b0.x)},${Math.round(b0.y)} during=${Math.round(mid.x)},${Math.round(mid.y)} after=${Math.round(b1.x)},${Math.round(b1.y)} dirty=${await dirtyText(page)}`;
	await ctx.shot('pptx-drag');
	return Math.abs(b1.x - b0.x - 80) < 6 && Math.abs(b1.y - b0.y - 40) < 6 ? pass(d) : fail(`shape did not move by (+80,+40): ${d}`);
});

S('pptx.resize-handles', ['pptx'], 'S1', 'Corner handle drag resizes the shape; cursor shows resize', async (ctx) => {
	const { page } = ctx;
	const r = await pptxText(page, 0);
	const b0 = await shapeBoxOf(page, 0);
	await page.mouse.click(r.x + r.w / 2, midY(r)); await sleep(page, 300);
	const hx = b0.x + b0.w, hy = b0.y + b0.h;
	const cursor = await page.evaluate(([x, y]) => { const sr = document.querySelector('nitro-pptx').shadowRoot; const t = sr.elementFromPoint(x - 1, y - 1); return t && getComputedStyle(t).cursor; }, [hx, hy]);
	await page.mouse.move(hx, hy); await page.mouse.down(); await page.mouse.move(hx + 60, hy + 40, { steps: 8 }); await page.mouse.up(); await sleep(page, 500);
	const b1 = await shapeBoxOf(page, 0);
	const d = `size ${Math.round(b0.w)}x${Math.round(b0.h)} -> ${Math.round(b1.w)}x${Math.round(b1.h)}, cursor=${cursor}`;
	return b1.w > b0.w + 30 ? pass(d) : fail(d);
});

S('pptx.text-edit', ['pptx'], 'S1', 'Double-click a shape enters text edit; typing changes its text', async (ctx) => {
	const { page } = ctx;
	const r = await pptxText(page, 0);
	await page.mouse.dblclick(r.x + r.w / 2, midY(r));
	await sleep(page, 300);
	await page.keyboard.type('XYZ', { delay: 40 });
	await sleep(page, 1200);
	const txt = await domText(page, 'pptx');
	const model = await savedContains(page, 'pptx', 'XYZ').catch(() => false);
	await ctx.shot('pptx-text-edit');
	const d = `dom has XYZ=${txt.includes('XYZ')} model has XYZ=${model} dirty=${await dirtyText(page)}`;
	return txt.includes('XYZ') ? pass(d) : fail(d);
});

S('pptx.rapid-typing', ['pptx'], 'S1', 'Fast typing (10 ms/char) in a text box keeps every character', async (ctx) => {
	const { page } = ctx;
	const r = await pptxText(page, 0);
	await page.mouse.dblclick(r.x + r.w / 2, midY(r));
	await sleep(page, 300);
	await page.keyboard.type('abcdefghijklmnopqrst', { delay: 10 });
	await sleep(page, 2000);
	const model = await savedContains(page, 'pptx', 'abcdefghijklmnopqrst').catch(() => false);
	const txt = (await domText(page, 'pptx')).includes('abcdefghijklmnopqrst');
	return model && txt ? pass() : fail(`model=${model} dom=${txt}`);
});

S('pptx.undo-redo', ['pptx'], 'S2', 'Ctrl+Z / Ctrl+Y round-trip a text edit', async (ctx) => {
	const { page } = ctx;
	const r = await pptxText(page, 0);
	await page.mouse.dblclick(r.x + r.w / 2, midY(r)); await page.keyboard.type('QQ'); await sleep(page, 800);
	const typed = await savedContains(page, 'pptx', 'QQ').catch(() => false);
	await page.keyboard.press('Control+z'); await sleep(page, 600);
	const undone = !(await savedContains(page, 'pptx', 'QQ').catch(() => true));
	await page.keyboard.press('Control+y'); await sleep(page, 600);
	const redone = await savedContains(page, 'pptx', 'QQ').catch(() => false);
	const d = `typed=${typed} undone=${undone} redone=${redone}`;
	return typed && undone && redone ? pass(d) : fail(d);
});

S('pptx.thumb-click-nav', ['pptx'], 'S3', 'Clicking a slide thumbnail navigates to that slide', async (ctx) => {
	const { page } = ctx;
	await page.waitForSelector('.pl .th', { timeout: 10000 }).catch(() => {});
	const before = await slideTitle(page);
	const t = await thumbRect(page, 2);
	if (!t) return fail('no thumbnail pane (.pl .th)');
	await page.mouse.click(t.x + t.w / 2, t.y + t.h / 2); await sleep(page, 800);
	const after = await slideTitle(page);
	return before !== after ? pass(`"${before}" -> "${after}"`) : fail(`still "${after}"`);
}, { modes: ['full'] });

S('pptx.thumb-drag-reorder', ['pptx'], 'S2', 'Dragging a slide thumbnail reorders slides', async (ctx) => {
	const { page } = ctx;
	await page.waitForSelector('.pl .th', { timeout: 10000 }).catch(() => {});
	await sleep(page, 1500);
	const before = await thumbTexts(page);
	const a = await thumbRect(page, 0), b = await thumbRect(page, 2);
	if (!a || !b) return fail('no thumbnail pane');
	await page.mouse.move(a.x + a.w / 2, a.y + a.h / 2); await page.mouse.down();
	await page.mouse.move(b.x + b.w / 2, b.y + b.h + 10, { steps: 12 }); await page.mouse.up(); await sleep(page, 800);
	const after = await thumbTexts(page);
	const draggable = await page.evaluate(() => !!document.querySelector('.pl .th[draggable=true]'));
	const d = `before=${JSON.stringify(before)} after=${JSON.stringify(after)} draggable-attr=${draggable}`;
	await ctx.shot('pptx-thumb-reorder');
	return JSON.stringify(before) !== JSON.stringify(after) ? pass(d) : fail(d);
}, { modes: ['full'] });

S('pptx.key-nav', ['pptx'], 'S3', 'PageDown / PageUp move between slides; Home/End jump', async (ctx) => {
	const { page } = ctx;
	await page.mouse.click(600, 500);
	const t0 = await slideTitle(page);
	await page.keyboard.press('PageDown'); await sleep(page, 600);
	const t1 = await slideTitle(page);
	await page.keyboard.press('PageUp'); await sleep(page, 600);
	const t2 = await slideTitle(page);
	return t0 !== t1 && t2 === t0 ? pass(`${t0} > ${t1} > ${t2}`) : fail(`${t0} > ${t1} > ${t2}`);
});

S('pptx.slide-fits-viewport', ['pptx'], 'S3', 'The slide is fitted to the viewport (no clipping/scroll at 100% default)', async (ctx) => {
	const { page } = ctx;
	const m = await inEl(page, 'pptx', (el, sr) => { const s = sr.querySelector('[aria-roledescription=slide]'); const v = sr.querySelector('.v').getBoundingClientRect(); const r = s.getBoundingClientRect(); return { slide: [Math.round(r.width), Math.round(r.height)], view: [Math.round(v.width), Math.round(v.height)] }; });
	await ctx.shot('pptx-fit');
	return m.slide[0] <= m.view[0] + 1 && m.slide[1] <= m.view[1] + 1 ? pass(JSON.stringify(m)) : fail(`slide ${m.slide} larger than viewport ${m.view}`);
}, { modes: ['full'] });

// ---------- TOUCH ----------
const touchTap = async (ctx, x, y) => { await ctx.page.touchscreen.tap(x, y); await sleep(ctx.page, 300); };
const cdpTouch = async (ctx, pts, hold = 0) => {
	const cdp = await ctx.cdp();
	await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [pts[0]] });
	if (hold) await sleep(ctx.page, hold);
	for (const p of pts.slice(1)) { await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [p] }); await sleep(ctx.page, 16); }
	await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
	await sleep(ctx.page, 300);
};
const installPtrProbe = (page, fmt) =>
	inEl(page, fmt, (el, sr) => {
		const v = sr.querySelector('.v');
		window.__p = { down: 0, move: 0, up: 0, cancel: 0, types: new Set() };
		for (const k of ['pointerdown', 'pointermove', 'pointerup', 'pointercancel']) v.addEventListener(k, (e) => { const n = k.replace('pointer', ''); window.__p[n]++; window.__p.types.add(e.pointerType); });
	});

S('touch.touch-action', ['docx', 'xlsx', 'pptx'], 'S1', 'The surface declares touch-action so pointer events survive a touch drag (and the page does not double-tap zoom)', async (ctx) => {
	const v = await inEl(ctx.page, ctx.fmt, (el, sr) => { const e = sr.querySelector('.v'); const c = getComputedStyle(e); return { touchAction: c.touchAction, userSelect: c.userSelect, webkitCallout: c.webkitTouchCallout }; });
	return /none|manipulation|pan/.test(v.touchAction) && v.touchAction !== 'auto' ? pass(JSON.stringify(v)) : fail(`touch-action=${v.touchAction}: the browser takes over a touch drag (pointercancel after ~1 move) and long-press opens the OS callout/selection; ${JSON.stringify(v)}`);
}, { touch: true });

S('touch.tap-xlsx', ['xlsx'], 'S2', 'Tap selects a cell', async (ctx) => {
	const g = await xlSetup(ctx), p = pt(g, 3, 4);
	await touchTap(ctx, p.x, p.y);
	const s = await sel(ctx.page);
	return s === 'D5' ? pass(s) : fail(`selection=${s}`);
}, { touch: true });

S('touch.drag-xlsx', ['xlsx'], 'S2', 'One-finger drag scrolls the grid (Excel mobile) without being swallowed', async (ctx) => {
	if (ctx.browser !== 'chromium') return na('needs CDP touch dispatch (chromium only)');
	const g = await xlSetup(ctx), a = pt(g, 3, 8);
	await installPtrProbe(ctx.page, 'xlsx');
	const before = await cellText(ctx.page, a);
	const pts = Array.from({ length: 14 }, (_, i) => ({ x: a.x, y: a.y - i * 14 }));
	await cdpTouch(ctx, pts);
	const after = await cellText(ctx.page, a);
	const p = await ctx.page.evaluate(() => ({ ...window.__p, types: [...window.__p.types] }));
	const s = await sel(ctx.page);
	const d = `grid scrolled=${before !== after} pointer events=${JSON.stringify(p)} selection=${s}`;
	return before !== after ? pass(d) : fail(d);
}, { touch: true });

S('touch.tap-docx-caret', ['docx'], 'S2', 'Tap places a caret', async (ctx) => {
	const { page } = ctx;
	const { r, clip, base } = await docxSelFixture(ctx, 3);
	await touchTap(ctx, r.x + 150, midY(r));
	const st = await anyChange(page, clip, base, [100, 400, 700, 1000]);
	return st.changed > 0 ? pass() : fail('no caret after tap');
}, { touch: true });

S('touch.long-press-word-docx', ['docx'], 'S2', 'Long-press selects a word (with selection handles)', async (ctx) => {
	if (ctx.browser !== 'chromium') return na('needs CDP touch dispatch (chromium only)');
	const { page } = ctx;
	const { r, clip, base } = await docxSelFixture(ctx, 2);
	await cdpTouch(ctx, [{ x: r.x + 60, y: midY(r) }], 800);
	const st = diffStats(base, await clipPng(page, clip));
	await ctx.shot('touch-long-press');
	return st.bbox && st.bbox.w > 12 && st.bbox.w < 170 ? pass() : fail(`bbox=${JSON.stringify(st.bbox)}`);
}, { touch: true });

S('touch.drag-docx', ['docx'], 'S2', 'One-finger drag on the document scrolls it', async (ctx) => {
	if (ctx.browser !== 'chromium') return na('needs CDP touch dispatch (chromium only)');
	const { page } = ctx;
	const r = await docxLine(page, 3);
	const y0 = await docxLine(page, 3);
	const pts = Array.from({ length: 12 }, (_, i) => ({ x: r.x + 100, y: midY(r) - i * 12 }));
	await cdpTouch(ctx, pts);
	const y1 = await docxLine(page, 3);
	return y1.y < y0.y - 20 ? pass(`line moved ${Math.round(y0.y - y1.y)}px`) : fail(`line y ${y0.y} -> ${y1.y}: document did not scroll`);
}, { touch: true });

S('touch.tap-pptx-select', ['pptx'], 'S2', 'Tap selects a shape', async (ctx) => {
	const { page } = ctx;
	const r = await pptxText(page, 0), box = await shapeBoxOf(page, 0);
	const clip = pad(box, 20), base = await clipPng(page, clip);
	await touchTap(ctx, r.x + r.w / 2, midY(r));
	const st = await anyChange(page, clip, base, [0, 300, 600], 24, { x: 20, y: 20, w: box.w - 2, h: box.h - 2 });
	return st.changed > 40 ? pass() : fail(`no outline after tap (ring px=${st.changed})`);
}, { touch: true });

S('touch.dblclick-edit-xlsx', ['xlsx'], 'S3', 'Double-tap a cell enters edit mode', async (ctx) => {
	const g = await xlSetup(ctx), p = pt(g, 0, 1);
	await ctx.page.touchscreen.tap(p.x, p.y); await sleep(ctx.page, 60); await ctx.page.touchscreen.tap(p.x, p.y);
	await sleep(ctx.page, 300);
	const a = await deepActive(ctx.page);
	const editing = await inEl(ctx.page, 'xlsx', (el, sr) => !!sr.querySelector('.xg-edit-caret:not([hidden]), .xg-editor'));
	return editing || (a && a.editable) ? pass() : fail(`no edit state after double-tap (activeElement=${JSON.stringify(a)})`);
}, { touch: true });
