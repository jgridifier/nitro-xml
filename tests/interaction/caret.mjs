// SPDX-License-Identifier: MIT
/**
 * Docx caret regression gate (run from tests/selection/run.mjs, so it rides the `selection-parity`
 * job). Real mouse/keyboard events against the local demo build in every selection browser.
 * Each check names the user-visible bug it catches (AGENTS.md rule 0).
 */
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { startServer } from './server.mjs';
import { openDemo, inEl, rectOf, domKids, docxModelParas, zipEntries } from './lib.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const sleep = (page, ms) => page.waitForTimeout(ms);
const midY = (r) => r.y + r.h / 2;
const line = (page, i) => rectOf(page, 'docx', '.dve-print-line', i);
const lineText = (page, i) => inEl(page, 'docx', (el, sr, n) => sr.querySelectorAll('.dve-print-line')[n]?.textContent ?? '', i);

/** Caret overlay rect (client px) or null when hidden. */
const caretRect = (page) =>
	inEl(page, 'docx', (el, sr) => {
		const c = sr.querySelector('.nxc');
		if (!c || c.style.display === 'none' || !c.style.display) return null;
		const r = c.getBoundingClientRect();
		return { x: r.x, y: r.y, h: r.height, blinking: c.getAnimations().length > 0 };
	});

async function documentXml(page) {
	const arr = await inEl(page, 'docx', async (el) => Array.from(await el.save()));
	return zipEntries(Buffer.from(arr))['word/document.xml']().toString('utf8');
}

const press = async (page, key, n = 1) => {
	for (let i = 0; i < n; i++) {
		await page.keyboard.press(key);
		await sleep(page, 120);
	}
};

const CHECKS = [
	{
		id: 'docx.caret-click-type',
		// Catches: clicks never placing a caret, so typed text always landed at the start of the first paragraph.
		fn: async (page) => {
			const before = await docxModelParas(page);
			const r = await line(page, 3);
			await page.mouse.click(r.x + 150, midY(r));
			await sleep(page, 200);
			const c = await caretRect(page);
			if (!c) return 'no caret overlay after click';
			if (Math.abs(c.x - (r.x + 150)) > 14 || c.y > midY(r) || c.y + c.h < midY(r)) return `caret ${JSON.stringify(c)} not at click (${r.x + 150},${midY(r)})`;
			if (!c.blinking) return 'caret has no blink animation';
			await page.keyboard.type('ZQ', { delay: 30 });
			await sleep(page, 600);
			const after = await docxModelParas(page);
			const changed = after.map((t, i) => (t !== before[i] ? i : -1)).filter((i) => i >= 0);
			if (changed.length !== 1 || !after[changed[0]].includes('ZQ')) return `edit landed in paragraphs ${changed} (want exactly the clicked one)`;
			if (after[0] !== before[0]) return 'title paragraph was edited';
			const c2 = await caretRect(page);
			if (!c2 || c2.x <= c.x) return `caret did not advance after typing (${c && c.x} -> ${c2 && c2.x})`;
			return null;
		},
	},
	{
		id: 'docx.type-keeps-page',
		// Catches: the first keystroke clearing the rendered page (empty frame wiped the DOM).
		fn: async (page) => {
			const r = await line(page, 3);
			await page.mouse.click(r.x + 100, midY(r));
			await page.keyboard.type('abc', { delay: 15 });
			for (let t = 0; t < 8; t++) {
				if ((await domKids(page, 'docx')) < 1) return 'page DOM was cleared while typing';
				await sleep(page, 60);
			}
			const txt = await lineText(page, 3);
			return txt.includes('abc') ? null : `typed text not rendered: "${txt.slice(0, 80)}"`;
		},
	},
	{
		id: 'docx.arrows-home-end-up-down',
		// Catches: arrow keys not moving the caret, Home/End and Up/Down ignoring wrapped lines.
		fn: async (page) => {
			const r = await line(page, 3);
			await page.mouse.click(r.x + 200, midY(r));
			await sleep(page, 200);
			const a = await caretRect(page);
			await press(page, 'ArrowLeft', 3);
			const l = await caretRect(page);
			await press(page, 'ArrowRight', 3);
			const back = await caretRect(page);
			if (!(l.x < a.x - 4)) return `ArrowLeft did not move caret left (${a.x} -> ${l.x})`;
			if (Math.abs(back.x - a.x) > 1.5) return `Right x3 after Left x3 did not return (${a.x} vs ${back.x})`;
			await press(page, 'Home');
			const h = await caretRect(page);
			if (Math.abs(h.x - r.x) > 3) return `Home not at line start (${h.x} vs ${r.x})`;
			await press(page, 'End');
			const e = await caretRect(page);
			if (!(e.x > a.x)) return `End not past mid-line caret (${e.x} vs ${a.x})`;
			await press(page, 'ArrowDown');
			const d = await caretRect(page);
			if (!(d.y > e.y + 2)) return `ArrowDown did not move to a lower line (${e.y} -> ${d.y})`;
			await press(page, 'ArrowUp');
			const u = await caretRect(page);
			if (Math.abs(u.y - e.y) > 2) return `ArrowUp did not return to the line (${e.y} vs ${u.y})`;
			return null;
		},
	},
	{
		id: 'docx.backspace-delete-join',
		// Catches: Backspace/Delete doing nothing at run and paragraph boundaries.
		fn: async (page) => {
			const m0 = await docxModelParas(page);
			const r = await line(page, 2);
			await page.mouse.click(r.x + 1, midY(r));
			await press(page, 'Backspace');
			const m1 = await docxModelParas(page);
			if (m1.length !== m0.length - 1) return `Backspace at paragraph start: paragraphs ${m0.length} -> ${m1.length}`;
			if (!(await caretRect(page))) return 'caret lost after join';
			await press(page, 'Delete', 2);
			const m2 = await docxModelParas(page);
			const chars = (m) => m.join('').length;
			if (chars(m2) !== chars(m1) - 2 && m2.length !== m1.length - 1) return `Delete removed nothing (${chars(m1)} -> ${chars(m2)})`;
			const r1 = await line(page, 1);
			await page.mouse.click(r1.x + r1.w - 40, midY(r1));
			await press(page, 'Backspace', 12);
			const m3 = await docxModelParas(page);
			return chars(m3) === chars(m2) - 12 ? null : `Backspace x12 across runs removed ${chars(m2) - chars(m3)}`;
		},
	},
	{
		id: 'docx.enter-splits-and-keeps-runs',
		// Catches: typing flattening a mixed-format paragraph into one run, and Enter not splitting.
		fn: async (page) => {
			const x0 = await documentXml(page);
			const runs = (x) => (x.match(/<w:r[ >]/g) || []).length;
			const paras = (x) => (x.match(/<w:p[ >]/g) || []).length;
			const r = await line(page, 1);
			await page.mouse.click(r.x + 30, midY(r));
			await page.keyboard.type('k', { delay: 20 });
			await sleep(page, 400);
			const x1 = await documentXml(page);
			if (runs(x1) < runs(x0)) return `typing merged runs (${runs(x0)} -> ${runs(x1)}): formatting flattened`;
			await press(page, 'Enter');
			const x2 = await documentXml(page);
			if (paras(x2) !== paras(x1) + 1) return `Enter: paragraphs ${paras(x1)} -> ${paras(x2)}`;
			return null;
		},
	},
	{
		id: 'docx.undo-typing-is-one-step',
		// Catches: Ctrl+Z not undoing typed text, or needing one press per character.
		fn: async (page) => {
			const m0 = await docxModelParas(page);
			const r = await line(page, 3);
			await page.mouse.click(r.x + 100, midY(r));
			await page.keyboard.type('XYZ', { delay: 40 });
			await sleep(page, 500);
			if (!(await docxModelParas(page)).join('').includes('XYZ')) return 'typing did not land';
			await page.keyboard.press('Control+z');
			await sleep(page, 500);
			const u = await docxModelParas(page);
			if (u.join('') !== m0.join('')) return `one Ctrl+Z did not restore the document (${u.join('').length} vs ${m0.join('').length} chars)`;
			await page.keyboard.press('Control+y');
			await sleep(page, 500);
			if (!(await docxModelParas(page)).join('').includes('XYZ')) return 'redo did not restore typing';
			return (await caretRect(page)) ? null : 'caret hidden after undo/redo';
		},
	},
];

/** Run the caret checks in each browser. Returns { ok, rows, summary } like the other selection stages. */
export async function runCaretChecks(browserNames) {
	const pw = await import(pathToFileURL(join(HERE, '../../parity/visual/node_modules/playwright/index.mjs')).href);
	const server = await startServer();
	const rows = [];
	try {
		for (const name of browserNames) {
			const browser = await pw[name].launch();
			try {
				for (const c of CHECKS) {
					const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
					const page = await context.newPage();
					let msg;
					try {
						await openDemo(page, server.url, 'docx');
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
	const summary = `docx caret: ${rows.length - bad.length}/${rows.length} ok` + bad.map((r) => `\n  FAIL ${r.case}: ${r.detail}`).join('');
	return { ok: bad.length === 0, rows, summary };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
	const names = (process.env.SELECTION_BROWSERS || 'chromium,webkit,firefox').split(',');
	const res = await runCaretChecks(names);
	console.log(res.summary);
	process.exit(res.ok ? 0 : 1);
}
