// SPDX-License-Identifier: MIT
/**
 * Bidi text-editing parity (#57): Rust `textSurfaceEval` vs Chromium `<textarea>`
 * (dir=auto, unicode-bidi: plaintext). UTF-16 anchor/focus; optional WebKit/Firefox smoke.
 */

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../../..');
const VISUAL = join(ROOT, 'parity/visual');
const OUT = join(ROOT, 'tests/bidi/edit/out');

const AR = 'مرحبا بالعالم';
const HE = 'שלום עולם';
const MIXED = 'Hello שלום world مرحبا';

/** @type {{ id: string, text: string, anchor: number, focus: number, ops: object[] }[]} */
export const EDIT_CASES = [
	{
		id: 'ar-arrow-mid',
		text: AR,
		anchor: 6,
		focus: 6,
		ops: [{ op: 'key', key: 'ArrowRight' }, { op: 'key', key: 'ArrowLeft' }],
	},
	{
		id: 'ar-shift-left',
		text: AR,
		anchor: 6,
		focus: 6,
		ops: [
			{ op: 'key', key: 'ArrowLeft', shift: true },
			{ op: 'key', key: 'ArrowLeft', shift: true },
		],
	},
	{
		id: 'ar-collapse-on-arrow',
		text: AR,
		anchor: 3,
		focus: 9,
		ops: [{ op: 'key', key: 'ArrowLeft' }],
	},
	{
		id: 'he-arrow-mid',
		text: HE,
		anchor: 4,
		focus: 4,
		ops: [{ op: 'key', key: 'ArrowRight' }, { op: 'key', key: 'ArrowLeft' }],
	},
	{
		id: 'mixed-ltr-paragraph-arrows',
		text: MIXED,
		anchor: 5,
		focus: 5,
		ops: [{ op: 'key', key: 'ArrowRight' }, { op: 'key', key: 'ArrowLeft' }],
	},
	{
		id: 'negative-pure-ltr',
		text: 'hello world',
		anchor: 11,
		focus: 11,
		ops: [{ op: 'key', key: 'ArrowLeft' }, { op: 'key', key: 'ArrowLeft', shift: true }],
	},
	{
		id: 'mixed-delete-logical',
		text: MIXED,
		anchor: 10,
		focus: 16,
		ops: [{ op: 'key', key: 'Backspace' }],
	},
	{
		id: 'ar-copy-logical',
		text: AR,
		anchor: 6,
		focus: 6,
		ops: [
			{ op: 'key', key: 'ArrowLeft', shift: true },
			{ op: 'key', key: 'ArrowLeft', shift: true },
			{ op: 'copy' },
		],
	},
];

function ensureWasm() {
	const glue = join(ROOT, 'packages/nitro-xml/pkg/nitro_xml_wasm.js');
	if (existsSync(glue)) return;
	const r = spawnSync('bash', [join(ROOT, 'scripts/build-wasm.sh')], {
		cwd: ROOT,
		encoding: 'utf8',
		maxBuffer: 32 * 1024 * 1024,
	});
	if (r.status !== 0) throw new Error(`build-wasm: ${r.stderr || r.stdout}`);
}

function ensurePlaywright() {
	const entry = join(VISUAL, 'node_modules/playwright/index.mjs');
	if (existsSync(entry)) return;
	spawnSync('bun', ['install'], { cwd: VISUAL, encoding: 'utf8' });
	spawnSync('bunx', ['playwright', 'install', 'chromium'], { cwd: VISUAL, encoding: 'utf8' });
}

async function loadWasm() {
	ensureWasm();
	const pkgDir = join(ROOT, 'packages/nitro-xml/pkg');
	const js = join(pkgDir, 'nitro_xml_wasm.js');
	const mod = await import(pathToFileURL(js).href);
	if (typeof mod.default === 'function') await mod.default();
	else if (typeof mod.initSync === 'function') {
		const { readFileSync } = await import('node:fs');
		mod.initSync({ module: readFileSync(join(pkgDir, 'nitro_xml_wasm_bg.wasm')) });
	}
	return mod;
}

/**
 * @param {import('playwright').BrowserType} browserType
 * @param {typeof EDIT_CASES[0]} c
 */
async function textareaOracle(browserType, c) {
	const browser = await browserType.launch({ headless: true });
	const page = await browser.newPage();
	await page.setContent(`<!doctype html><textarea id="t" dir="auto" style="
		unicode-bidi: plaintext;
		font: 16px / 16px monospace;
		padding: 0; border: 0; width: 800px; height: 48px;
	"></textarea>`);
	const ta = page.locator('#t');
	await ta.click();
	await ta.fill(c.text);
	await ta.evaluate(
		(el, { anchor, focus }) => {
			el.setSelectionRange(anchor, focus);
		},
		{ anchor: c.anchor, focus: c.focus },
	);

	let clipboard = '';
	for (const op of c.ops) {
		if (op.op === 'key') {
			const mods = [];
			if (op.shift) mods.push('Shift');
			if (op.ctrl) mods.push('Control');
			if (op.meta) mods.push('Meta');
			if (op.alt) mods.push('Alt');
			for (const m of mods) await page.keyboard.down(m);
			await page.keyboard.press(op.key);
			for (const m of mods.reverse()) await page.keyboard.up(m);
		} else if (op.op === 'copy') {
			const a = await ta.evaluate((el) => {
				const s = el.selectionStart;
				const e = el.selectionEnd;
				return el.value.slice(Math.min(s, e), Math.max(s, e));
			});
			clipboard = a;
		} else if (op.op === 'cut') {
			const a = await ta.evaluate((el) => {
				const s = el.selectionStart;
				const e = el.selectionEnd;
				const t = el.value.slice(Math.min(s, e), Math.max(s, e));
				el.value = el.value.slice(0, Math.min(s, e)) + el.value.slice(Math.max(s, e));
				el.setSelectionRange(Math.min(s, e), Math.min(s, e));
				return t;
			});
			clipboard = a;
		}
	}

	const snap = await ta.evaluate((el) => {
		const start = el.selectionStart;
		const end = el.selectionEnd;
		const dir = el.selectionDirection || 'none';
		const anchor = dir === 'backward' ? end : start;
		const focus = dir === 'backward' ? start : end;
		return {
			text: el.value,
			anchor,
			focus,
			selected: el.value.slice(Math.min(start, end), Math.max(start, end)),
		};
	});
	await browser.close();
	return { ...snap, clipboard };
}

function rustEval(wasm, c) {
	const script = {
		text: c.text,
		anchor: c.anchor,
		focus: c.focus,
		ops: c.ops,
		charWidth: 8,
		lineHeight: 16,
	};
	return JSON.parse(wasm.textSurfaceEval(JSON.stringify(script)));
}

/**
 * @param {import('playwright').BrowserType} chromium
 */
export async function runEditParity(chromium, { browserName = 'chromium' } = {}) {
	mkdirSync(OUT, { recursive: true });
	const wasm = await loadWasm();
	const results = [];
	let failed = 0;
	for (const c of EDIT_CASES) {
		const rust = rustEval(wasm, c);
		const ts = await textareaOracle(chromium, c);
		const ok =
			rust.text === ts.text &&
			rust.anchor === ts.anchor &&
			rust.focus === ts.focus &&
			rust.selected === ts.selected &&
			(c.ops.some((o) => o.op === 'copy') ? rust.clipboard === ts.clipboard : true);
		if (!ok) failed++;
		const summary = ok
			? `OK [${browserName}] ${c.id}`
			: `FAIL [${browserName}] ${c.id}: rust=${JSON.stringify({
					text: rust.text,
					a: rust.anchor,
					f: rust.focus,
					sel: rust.selected,
					clip: rust.clipboard,
				})} ts=${JSON.stringify(ts)}`;
		console.log(summary);
		results.push({ id: c.id, browser: browserName, ok, summary, rust, ts });
	}
	const line =
		failed === 0
			? `bidi-edit: PASS [${browserName}] (${EDIT_CASES.length}/${EDIT_CASES.length})`
			: `bidi-edit: FAIL [${browserName}] (${EDIT_CASES.length - failed}/${EDIT_CASES.length})`;
	return { ok: failed === 0, summary: line, results, failed };
}

export async function runEditGate() {
	ensurePlaywright();
	const { chromium, webkit, firefox } = await import(
		pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href
	);
	const chromiumResult = await runEditParity(chromium, { browserName: 'chromium' });
	let smokeOk = true;
	for (const [name, bt] of [
		['webkit', webkit],
		['firefox', firefox],
	]) {
		try {
			const r = await runEditParity(bt, { browserName: name });
			if (!r.ok) smokeOk = false;
		} catch (e) {
			console.log(`bidi-edit: SKIP [${name}] smoke (${e.message})`);
		}
	}
	writeFileSync(
		join(OUT, 'edit-parity.json'),
		JSON.stringify({ chromium: chromiumResult.results }, null, 2),
	);
	return {
		ok: chromiumResult.ok,
		summary: [chromiumResult.summary, smokeOk ? 'bidi-edit smoke: ok' : 'bidi-edit smoke: see logs'].join(
			'\n',
		),
		chromium: chromiumResult,
	};
}
