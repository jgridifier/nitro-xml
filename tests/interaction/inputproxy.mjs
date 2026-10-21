// SPDX-License-Identifier: MIT
/**
 * Text-input proxy regression gate (run from tests/selection/run.mjs, so it rides the
 * `selection-parity` job). Real mouse + keyboard events (Playwright `insertText` produces the
 * `beforeinput`/`input` stream a soft keyboard does; CDP drives real IME composition on Chromium).
 * Each check names the user-visible bug it catches (AGENTS.md rule 0).
 */
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { startServer } from './server.mjs';
import { openDemo, inEl, rectOf, docxModelParas, savedContains } from './lib.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const sleep = (page, ms) => page.waitForTimeout(ms);
const midY = (r) => r.y + r.h / 2;
const line = (page, i) => rectOf(page, 'docx', '.dve-print-line', i);
const activeClass = (page, fmt) => inEl(page, fmt, (el, sr) => (sr.activeElement && sr.activeElement.className) || '');
const joined = async (page) => (await docxModelParas(page)).join('\n');

/** Dispatch a paste carrying `text` at the focused element inside the viewer (what Ctrl+V / the context menu do). */
const paste = (page, fmt, text) =>
	inEl(
		page,
		fmt,
		(el, sr, t) => {
			const dt = new DataTransfer();
			dt.setData('text/plain', t);
			const ev = new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true });
			// Firefox hands listeners an emptied DataTransfer for synthetic paste events.
			if (!ev.clipboardData || ev.clipboardData.getData('text/plain') !== t) {
				Object.defineProperty(ev, 'clipboardData', { value: { getData: (k) => (k === 'text/plain' ? t : '') } });
			}
			(sr.activeElement || el).dispatchEvent(ev);
		},
		text,
	);

const clickDocx = async (page, li = 3, dx = 100) => {
	const r = await line(page, li);
	await page.mouse.click(r.x + dx, midY(r));
	await sleep(page, 200);
};

const CHECKS = [
	{
		fmt: 'docx',
		id: 'proxy.focus-and-caret',
		// Catches: focus sitting on a div (no text-input target), so IME / mobile keyboards could never reach the editor; or the caret hiding when focus moved.
		fn: async (page) => {
			await clickDocx(page);
			const cls = await activeClass(page, 'docx');
			if (cls !== 'nxp') return `active element after click is "${cls}", want the input proxy`;
			const vis = await inEl(page, 'docx', (el, sr) => sr.querySelector('.nxc')?.style.display === 'block');
			return vis ? null : 'caret overlay hidden while the proxy has focus';
		},
	},
	{
		fmt: 'docx',
		id: 'proxy.typing-not-doubled',
		// Catches: the textarea echoing characters the keydown path already inserted (every key typed twice).
		fn: async (page) => {
			await clickDocx(page);
			await page.keyboard.type('qzx', { delay: 40 });
			await sleep(page, 500);
			const t = await joined(page);
			const n = (t.match(/qzx/g) || []).length;
			return n === 1 ? null : `typed "qzx" once, found it ${n}x in the model`;
		},
	},
	{
		fmt: 'docx',
		id: 'proxy.soft-keyboard-insert',
		// Catches: text from soft keyboards / dictation / dead keys (beforeinput insertText, no keydown char) being dropped.
		fn: async (page) => {
			await clickDocx(page);
			await page.keyboard.insertText('é☃😀');
			await sleep(page, 600);
			const t = await joined(page);
			return t.includes('é☃😀') ? null : 'beforeinput text never reached the document';
		},
	},
	{
		fmt: 'docx',
		id: 'proxy.altgr-char',
		// Catches: Alt/AltGr/Option chords that produce text being swallowed (the editor key path ignores modified keys).
		fn: async (page) => {
			await clickDocx(page);
			// A layout that yields "@" from AltGr+Q reports ctrl+alt with key "@"; Playwright cannot synthesise the layout, so emit the same event pair.
			await page.evaluate(() => {
				const el = document.querySelector('nitro-docx');
				const ta = el.shadowRoot.activeElement;
				const k = new KeyboardEvent('keydown', { key: '@', code: 'KeyQ', ctrlKey: true, altKey: true, bubbles: true, cancelable: true, composed: true });
				ta.dispatchEvent(k);
				window.__altgrPrevented = k.defaultPrevented;
			});
			const prevented = await page.evaluate(() => window.__altgrPrevented);
			return prevented ? 'AltGr character keydown was cancelled, so the browser would never insert it' : null;
		},
	},
	{
		fmt: 'docx',
		id: 'proxy.paste-text',
		// Catches: paste events being ignored (Ctrl+V / context-menu paste did nothing in docx).
		fn: async (page) => {
			await clickDocx(page);
			await paste(page, 'docx', 'PASTED one');
			await sleep(page, 800);
			const t = await joined(page);
			return t.includes('PASTED one') ? null : 'pasted text missing from the model';
		},
	},
	{
		fmt: 'docx',
		id: 'proxy.paste-multiline',
		// Catches: pasted newlines being inserted as literal characters instead of paragraph breaks.
		fn: async (page) => {
			const before = (await docxModelParas(page)).length;
			await clickDocx(page);
			await paste(page, 'docx', 'LNA\nLNB');
			await sleep(page, 900);
			const p = await docxModelParas(page);
			if (p.length !== before + 1) return `paragraphs ${before} -> ${p.length}, want +1`;
			return p.some((x) => x.includes('LNA')) && p.some((x) => x.startsWith('LNB') || x.includes('LNB')) ? null : 'pasted lines missing';
		},
	},
	{
		fmt: 'docx',
		id: 'proxy.ime-composition',
		chromiumOnly: true,
		// Catches: IME composition (CJK) being dropped, or preedit characters left behind in the document after commit.
		fn: async (page) => {
			await clickDocx(page);
			const cdp = await page.context().newCDPSession(page);
			await cdp.send('Input.imeSetComposition', { text: 'に', selectionStart: 1, selectionEnd: 1 });
			await sleep(page, 400);
			await cdp.send('Input.imeSetComposition', { text: '日本', selectionStart: 2, selectionEnd: 2 });
			await sleep(page, 400);
			await cdp.send('Input.insertText', { text: '日本' });
			await sleep(page, 900);
			const t = await joined(page);
			if (!t.includes('日本')) return 'composed text never reached the document';
			return t.includes('に') ? 'preedit "に" left in the document after commit' : null;
		},
	},
	{
		fmt: 'xlsx',
		id: 'proxy.xlsx-paste-tsv',
		// Catches: Ctrl+V doing nothing in xlsx (copy-paste of a cell range failed at the paste step).
		fn: async (page) => {
			const c = await rectOf(page, 'xlsx', '.xg-c', 5);
			await page.mouse.click(c.x + c.w / 2, midY(c));
			await sleep(page, 300);
			if ((await activeClass(page, 'xlsx')) !== 'nxp') return 'input proxy not focused after clicking a cell';
			await paste(page, 'xlsx', 'pz1\tpz2\npz3\tpz4\n');
			await sleep(page, 1200);
			const txt = await inEl(page, 'xlsx', (el, sr) => sr.querySelector('.dom').textContent);
			const miss = ['pz1', 'pz2', 'pz3', 'pz4'].filter((s) => !txt.includes(s));
			return miss.length ? `cells not pasted: ${miss.join(',')}` : null;
		},
	},
	{
		fmt: 'xlsx',
		id: 'proxy.xlsx-type-once',
		// Catches: proxy echoing typed characters into the cell editor twice.
		fn: async (page) => {
			const c = await rectOf(page, 'xlsx', '.xg-c', 7);
			await page.mouse.click(c.x + c.w / 2, midY(c));
			await sleep(page, 300);
			await page.keyboard.type('kq', { delay: 40 });
			await page.keyboard.press('Enter');
			await sleep(page, 800);
			const txt = await inEl(page, 'xlsx', (el, sr) => sr.querySelector('.dom').textContent);
			return /kqkq/.test(txt) ? 'typed text doubled' : /kq/.test(txt) ? null : 'typed text missing';
		},
	},
	{
		fmt: 'pptx',
		id: 'proxy.pptx-soft-keyboard-insert',
		// Catches: soft-keyboard / dead-key text not reaching a slide shape.
		fn: async (page) => {
			const t = await rectOf(page, 'pptx', '.pptxv-text', 0);
			await page.mouse.dblclick(t.x + t.w / 2, midY(t));
			await sleep(page, 400);
			if ((await activeClass(page, 'pptx')) !== 'nxp') return 'input proxy not focused after clicking a shape';
			await page.keyboard.insertText('üß');
			await sleep(page, 800);
			return (await savedContains(page, 'pptx', 'üß')) ? null : 'beforeinput text never reached the shape';
		},
	},
];

/** Run the input-proxy checks in each browser. Returns { ok, rows, summary } like the other selection stages. */
export async function runInputProxyChecks(browserNames) {
	const pw = await import(pathToFileURL(join(HERE, '../../parity/visual/node_modules/playwright/index.mjs')).href);
	const server = await startServer();
	const rows = [];
	try {
		for (const name of browserNames) {
			const browser = await pw[name].launch();
			try {
				for (const c of CHECKS) {
					if (c.chromiumOnly && name !== 'chromium') continue;
					const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
					const page = await context.newPage();
					let msg;
					try {
						await openDemo(page, server.url, c.fmt);
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
	const summary = `input proxy: ${rows.length - bad.length}/${rows.length} ok` + bad.map((r) => `\n  FAIL ${r.case}: ${r.detail}`).join('');
	return { ok: bad.length === 0, rows, summary };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
	const names = (process.env.SELECTION_BROWSERS || 'chromium,webkit,firefox').split(',');
	const res = await runInputProxyChecks(names);
	console.log(res.summary);
	process.exit(res.ok ? 0 : 1);
}
