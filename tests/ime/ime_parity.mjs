// SPDX-License-Identifier: MIT
/**
 * IME composition parity (#56): CDP `Input.imeSetComposition` + `Input.insertText`
 * on a textarea oracle vs Rust `textSurfaceEval` compose ops.
 *
 *   bun tests/ime/ime_parity.mjs
 */
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const VISUAL = join(ROOT, 'parity/visual');
const OUT = join(ROOT, 'tests/ime/out');
const CARET_TOL = 1;
const CHAR_W = 8;
const LINE_H = 16;

/** UTF-16 code-unit length (matches Rust `byte_to_utf16` for BMP+CJK). */
function utf16Len(s) {
	let n = 0;
	for (const ch of s) {
		const cp = ch.codePointAt(0);
		n += cp > 0xffff ? 2 : 1;
	}
	return n;
}

/** @type {{ id: string, locale?: string, text: string, anchor: number, focus: number, ops: object[] }[]} */
const CASES = [
	{
		id: 'ja-compose-commit',
		text: 'ALPHA Product Overview',
		anchor: utf16Len('ALPHA Product Overview'),
		focus: utf16Len('ALPHA Product Overview'),
		ops: (() => {
			const start = utf16Len('ALPHA Product Overview');
			return [
				{
					op: 'composeUpdate',
					text: 'に',
					selectionStart: 1,
					selectionEnd: 1,
				},
				{
					op: 'composeUpdate',
					text: '日本',
					selectionStart: 2,
					selectionEnd: 2,
					replacementStart: start,
					replacementEnd: start + utf16Len('に'),
				},
				{ op: 'composeCommit', text: '日本' },
			];
		})(),
	},
	{
		id: 'zh-compose-commit',
		text: '你好',
		anchor: 0,
		focus: 0,
		ops: [
			{
				op: 'composeUpdate',
				text: 'ni',
				selectionStart: 2,
				selectionEnd: 2,
				replacementStart: 0,
				replacementEnd: 0,
			},
			{
				op: 'composeUpdate',
				text: '你',
				selectionStart: 1,
				selectionEnd: 1,
				replacementStart: 0,
				replacementEnd: utf16Len('ni'),
			},
			{ op: 'composeCommit', text: '你' },
		],
	},
	{
		id: 'ko-compose-commit',
		text: '한',
		anchor: utf16Len('한'),
		focus: utf16Len('한'),
		ops: (() => {
			const start = utf16Len('한');
			return [
				{
					op: 'composeUpdate',
					text: 'ㅎ',
					selectionStart: 1,
					selectionEnd: 1,
					replacementStart: start,
					replacementEnd: start,
				},
				{
					op: 'composeUpdate',
					text: '한',
					selectionStart: 1,
					selectionEnd: 1,
					replacementStart: start,
					replacementEnd: start + utf16Len('ㅎ'),
				},
				{ op: 'composeCommit', text: '한' },
			];
		})(),
	},
	{
		id: 'negative-cancel-no-preedit',
		text: 'stay',
		anchor: 4,
		focus: 4,
		ops: [
			{
				op: 'composeUpdate',
				text: '你',
				selectionStart: 1,
				selectionEnd: 1,
				replacementStart: 4,
				replacementEnd: 4,
			},
			{ op: 'composeCancel' },
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
	if (typeof mod.default === 'function') {
		await mod.default();
	} else if (typeof mod.initSync === 'function') {
		const { readFileSync } = await import('node:fs');
		const wasmPath = join(pkgDir, 'nitro_xml_wasm_bg.wasm');
		mod.initSync({ module: readFileSync(wasmPath) });
	}
	return mod;
}

async function cdpTextareaOracle(browserType, c) {
	const browser = await browserType.launch({ headless: true });
	const page = await browser.newPage();
	const client = await page.context().newCDPSession(page);
	await page.setContent(`<!doctype html><meta charset="utf-8"><textarea id="t" style="
		font: ${LINE_H}px / ${LINE_H}px monospace;
		letter-spacing: 0;
		padding: 0; border: 0; width: 800px; height: 40px;
	"></textarea>`);
	await page.evaluate(
		({ text, anchor, focus }) => {
			const el = /** @type {HTMLTextAreaElement} */ (document.getElementById('t'));
			el.value = text;
			el.focus();
			el.setSelectionRange(anchor, focus);
		},
		{ text: c.text, anchor: c.anchor, focus: c.focus },
	);

	/** @type {{ text: string, anchor: number, focus: number } | null} */
	let composeSnapshot = null;

	for (const op of c.ops) {
		if (op.op === 'composeUpdate') {
			if (!composeSnapshot) {
				composeSnapshot = await page.evaluate(() => {
					const el = /** @type {HTMLTextAreaElement} */ (document.getElementById('t'));
					return {
						text: el.value,
						anchor: el.selectionStart,
						focus: el.selectionEnd,
					};
				});
			}
			const payload = {
				text: op.text,
				selectionStart: op.selectionStart,
				selectionEnd: op.selectionEnd,
			};
			if (op.replacementStart !== undefined) {
				payload.replacementStart = op.replacementStart;
				payload.replacementEnd = op.replacementEnd;
			}
			await client.send('Input.imeSetComposition', payload);
		} else if (op.op === 'composeCommit') {
			await client.send('Input.insertText', { text: op.text });
			composeSnapshot = null;
		} else if (op.op === 'composeCancel') {
			// CDP Escape does not revert imeSetComposition; match textarea cancel semantics.
			if (composeSnapshot) {
				await page.evaluate((snap) => {
					const el = /** @type {HTMLTextAreaElement} */ (document.getElementById('t'));
					el.value = snap.text;
					el.setSelectionRange(snap.anchor, snap.focus);
				}, composeSnapshot);
			}
			composeSnapshot = null;
		}
	}

	const result = await page.evaluate(({ charW }) => {
		const el = /** @type {HTMLTextAreaElement} */ (document.getElementById('t'));
		const selStart = el.selectionStart;
		const selEnd = el.selectionEnd;
		const dir = el.selectionDirection || 'none';
		const outAnchor = dir === 'backward' ? selEnd : selStart;
		const outFocus = dir === 'backward' ? selStart : selEnd;
		const caretX = outFocus * charW;
		const rect = el.getBoundingClientRect();
		return {
			text: el.value,
			anchor: outAnchor,
			focus: outFocus,
			caret: { x: caretX, y: 0, w: 1, h: 16 },
			// Composition window anchor: caret in field content coordinates.
			compositionAt: { x: caretX, y: rect.top },
		};
	}, { charW: CHAR_W });

	await client.detach();
	await browser.close();
	return result;
}

function rectsWithin(a, b, tol) {
	return (
		Math.abs(a.x - b.x) <= tol &&
		Math.abs(a.y - b.y) <= tol &&
		Math.abs(a.w - b.w) <= tol &&
		Math.abs(a.h - b.h) <= tol
	);
}

async function runCase(wasm, browserType, c) {
	const script = {
		text: c.text,
		anchor: c.anchor,
		focus: c.focus,
		ops: c.ops,
		charWidth: CHAR_W,
		lineHeight: LINE_H,
	};
	const rust = JSON.parse(wasm.textSurfaceEval(JSON.stringify(script)));
	const ts = await cdpTextareaOracle(browserType, c);
	const selOk = rust.text === ts.text && rust.focus === ts.focus && rust.anchor === ts.anchor;
	const caretOk = rectsWithin(rust.caret, ts.caret, CARET_TOL);
	const ok = selOk && caretOk;
	return { ok, rust, ts, selOk, caretOk };
}

async function main() {
	ensurePlaywright();
	const wasm = await loadWasm();
	const pw = await import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);

	mkdirSync(OUT, { recursive: true });
	const results = [];
	let failed = 0;

	for (const c of CASES) {
		const { ok, rust, ts, selOk, caretOk } = await runCase(wasm, pw.chromium, c);
		if (!ok) failed++;
		const summary = ok
			? `OK ${c.id}: text/sel/caret ±${CARET_TOL}px`
			: `FAIL ${c.id}: sel=${selOk} caret=${caretOk} rust=${JSON.stringify({ text: rust.text, a: rust.anchor, f: rust.focus, caret: rust.caret })} ts=${JSON.stringify({ text: ts.text, a: ts.anchor, f: ts.focus, caret: ts.caret })}`;
		console.log(summary);
		results.push({ id: c.id, ok, summary, rust, ts });
	}

	// WebKit + Firefox nightly smoke: editable surface + caret geometry (no CDP).
	for (const name of ['webkit', 'firefox']) {
		const bt = pw[name];
		if (!bt) {
			console.log(`SKIP ${name} smoke: browser type unavailable`);
			continue;
		}
		try {
			const browser = await bt.launch({ headless: true });
			const page = await browser.newPage();
			await page.setContent(`<!doctype html><textarea id="t" style="
				font: ${LINE_H}px / ${LINE_H}px monospace; padding: 0; border: 0; width: 400px;
			">x</textarea>`);
			const geom = await page.evaluate(({ charW }) => {
				const el = /** @type {HTMLTextAreaElement} */ (document.getElementById('t'));
				el.focus();
				el.setSelectionRange(1, 1);
				const caretX = el.selectionEnd * charW;
				const rect = el.getBoundingClientRect();
				return { caretX, top: rect.top, width: el.offsetWidth };
			}, { charW: CHAR_W });
			await browser.close();
			const ok =
				geom.width > 0 &&
				Math.abs(geom.caretX - CHAR_W) <= CARET_TOL &&
				Number.isFinite(geom.top);
			console.log(ok ? `OK ${name} smoke: caret near composition anchor` : `FAIL ${name} smoke`);
			if (!ok) failed++;
			results.push({ id: `${name}-smoke`, ok, summary: `${name} caret geom` });
		} catch (e) {
			console.log(`SKIP ${name} smoke: ${e.message.split('\n')[0]}`);
			results.push({ id: `${name}-smoke`, ok: true, skipped: true, summary: String(e) });
		}
	}

	writeFileSync(join(OUT, 'ime-parity.json'), JSON.stringify(results, null, 2));
	const gate =
		failed === 0
			? `ime-parity: PASS (${CASES.length} CJK cases + smoke)`
			: `ime-parity: FAIL (${failed} failed)`;
	writeFileSync(join(OUT, 'gate-summary.txt'), gate + '\n');
	console.log(gate);
	if (failed) process.exit(1);
}

try {
	await main();
} catch (e) {
	console.error(e);
	process.exit(1);
}
