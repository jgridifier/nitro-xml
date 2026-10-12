// SPDX-License-Identifier: MIT
/**
 * Differential caret/selection parity for the single-line text surface (#55).
 * Drives the same key/pointer ops on a browser <textarea> (upstream oracle
 * behaviour) and Rust `textSurfaceEval`, comparing UTF-16 offsets and
 * synthetic caret rects within 1 CSS px (#43 tolerance).
 *
 *   bun tests/text/caret_parity.mjs
 */
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const VISUAL = join(ROOT, 'parity/visual');
const OUT = join(ROOT, 'tests/text/out');
const CARET_TOL = 1;
const CHAR_W = 8;
const LINE_H = 16;

/** @type {{ id: string, text: string, anchor: number, focus: number, ops: object[] }[]} */
const CASES = [
	{
		id: 'arrows-and-shift',
		text: 'hello world',
		anchor: 11,
		focus: 11,
		ops: [
			{ op: 'key', key: 'ArrowLeft' },
			{ op: 'key', key: 'ArrowLeft' },
			{ op: 'key', key: 'ArrowLeft', shift: true },
			{ op: 'key', key: 'ArrowLeft', shift: true },
		],
	},
	{
		id: 'mod-word-left',
		text: 'hello world',
		anchor: 11,
		focus: 11,
		ops: [{ op: 'key', key: 'ArrowLeft', ctrl: true }],
	},
	{
		id: 'home-end',
		text: 'abc',
		anchor: 2,
		focus: 2,
		ops: [
			{ op: 'key', key: 'Home' },
			{ op: 'key', key: 'End', shift: true },
		],
	},
	{
		id: 'pointer-click',
		text: 'abcdef',
		anchor: 0,
		focus: 0,
		// x=18 is nearer 16 (offset 2) than 24 — unambiguous vs Math.round ties.
		ops: [{ op: 'pointer', x: 18 }],
	},
	{
		id: 'cut-paste',
		text: 'abcdef',
		anchor: 1,
		focus: 4,
		ops: [{ op: 'cut' }, { op: 'paste', text: 'XYZ' }],
	},
	{
		id: 'negative-readonly-noop',
		// Negative control: empty ops leave selection unchanged (no phantom edits).
		text: 'stay',
		anchor: 2,
		focus: 2,
		ops: [],
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
	const require = createRequire(import.meta.url);
	// Prefer Node-target glue when present; fall back to bundler glue + wasm.
	const pkgDir = join(ROOT, 'packages/nitro-xml/pkg');
	const js = join(pkgDir, 'nitro_xml_wasm.js');
	const mod = await import(pathToFileURL(js).href);
	if (typeof mod.default === 'function') {
		await mod.default();
	} else if (typeof mod.initSync === 'function') {
		const wasmPath = join(pkgDir, 'nitro_xml_wasm_bg.wasm');
		const { readFileSync } = await import('node:fs');
		mod.initSync({ module: readFileSync(wasmPath) });
	}
	if (typeof mod.textSurfaceEval !== 'function') {
		throw new Error('textSurfaceEval missing from wasm exports');
	}
	return mod;
}

async function textareaOracle(chromium, c) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage();
	await page.setContent(`<!doctype html><textarea id="t" style="
		font: ${LINE_H}px / ${LINE_H}px monospace;
		letter-spacing: 0;
		padding: 0; border: 0; width: 400px; height: 40px;
	"></textarea>`);
	const result = await page.evaluate(
		({ text, anchor, focus, ops, charW }) => {
			const el = /** @type {HTMLTextAreaElement} */ (document.getElementById('t'));
			el.value = text;
			el.focus();
			el.setSelectionRange(anchor, focus);
			let clipboard = '';
			for (const op of ops) {
				if (op.op === 'key') {
					const start = el.selectionStart;
					const end = el.selectionEnd;
					const dir = el.selectionDirection || 'none';
					const forward = start === end || dir !== 'backward';
					const caret = forward ? end : start;
					const other = forward ? start : end;
					const mod = !!op.ctrl || !!op.meta;
					const shift = !!op.shift;
					const collapseTo = (pos) => el.setSelectionRange(pos, pos);
					const extendTo = (pos) => {
						const a = shift ? other : pos;
						const f = pos;
						if (a <= f) el.setSelectionRange(a, f, 'forward');
						else el.setSelectionRange(f, a, 'backward');
					};
					const isWord = (ch) => /[A-Za-z0-9_]/.test(ch);
					const nextWord = (from) => {
						let i = from;
						while (i < el.value.length && /\s/.test(el.value[i])) i++;
						if (i >= el.value.length) return el.value.length;
						if (isWord(el.value[i])) {
							while (i < el.value.length && isWord(el.value[i])) i++;
							return i;
						}
						return i + 1;
					};
					const prevWord = (from) => {
						let i = from;
						while (i > 0 && /\s/.test(el.value[i - 1])) i--;
						if (i <= 0) return 0;
						if (isWord(el.value[i - 1])) {
							while (i > 0 && isWord(el.value[i - 1])) i--;
							return i;
						}
						return i - 1;
					};
					const nextG = (from) => {
						const seg = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
						for (const s of seg.segment(el.value.slice(from))) {
							return from + s.segment.length;
						}
						return from;
					};
					const prevG = (from) => {
						const seg = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
						let last = 0;
						for (const s of seg.segment(el.value.slice(0, from))) {
							last += s.segment.length;
						}
						// find previous boundary
						let at = 0;
						let prev = 0;
						for (const s of seg.segment(el.value)) {
							const end = at + s.segment.length;
							if (end >= from) return at;
							prev = at;
							at = end;
						}
						return prev;
					};
					if (op.key === 'ArrowLeft') {
						if (mod) {
							const p = prevWord(caret);
							if (shift) extendTo(p);
							else collapseTo(p);
						} else if (!shift && start !== end) collapseTo(Math.min(start, end));
						else {
							const p = prevG(caret);
							if (shift) extendTo(p);
							else collapseTo(p);
						}
					} else if (op.key === 'ArrowRight') {
						if (mod) {
							const p = nextWord(caret);
							if (shift) extendTo(p);
							else collapseTo(p);
						} else if (!shift && start !== end) collapseTo(Math.max(start, end));
						else {
							const p = nextG(caret);
							if (shift) extendTo(p);
							else collapseTo(p);
						}
					} else if (op.key === 'Home') {
						const lineStart = el.value.lastIndexOf('\n', Math.max(0, caret - 1)) + 1;
						if (shift) extendTo(lineStart);
						else collapseTo(lineStart);
					} else if (op.key === 'End') {
						const nl = el.value.indexOf('\n', caret);
						const lineEnd = nl < 0 ? el.value.length : nl;
						if (shift) extendTo(lineEnd);
						else collapseTo(lineEnd);
					}
				} else if (op.op === 'pointer') {
					// Nearest grapheme/char boundary under monospace widths (matches Rust).
					let best = 0;
					let bestDist = Math.abs(op.x);
					for (let i = 0; i <= el.value.length; i++) {
						const gx = i * charW;
						const dist = Math.abs(gx - op.x);
						if (dist < bestDist) {
							best = i;
							bestDist = dist;
						}
					}
					const pos = best;
					if (op.extend) {
						const a = el.selectionStart;
						el.setSelectionRange(Math.min(a, pos), Math.max(a, pos));
					} else el.setSelectionRange(pos, pos);
				} else if (op.op === 'cut') {
					const a = el.selectionStart;
					const b = el.selectionEnd;
					clipboard = el.value.slice(a, b);
					el.value = el.value.slice(0, a) + el.value.slice(b);
					el.setSelectionRange(a, a);
				} else if (op.op === 'paste') {
					const t = op.text ?? clipboard;
					const a = el.selectionStart;
					const b = el.selectionEnd;
					el.value = el.value.slice(0, a) + t + el.value.slice(b);
					const c = a + t.length;
					el.setSelectionRange(c, c);
				} else if (op.op === 'insert') {
					const t = op.text ?? '';
					const a = el.selectionStart;
					const b = el.selectionEnd;
					el.value = el.value.slice(0, a) + t + el.value.slice(b);
					const c = a + t.length;
					el.setSelectionRange(c, c);
				}
			}
			const selStart = el.selectionStart;
			const selEnd = el.selectionEnd;
			const dir = el.selectionDirection || 'none';
			// Emit anchor/focus (caret = focus) like Rust; reverse when direction is backward.
			const outAnchor = dir === 'backward' ? selEnd : selStart;
			const outFocus = dir === 'backward' ? selStart : selEnd;
			const caretX = outFocus * charW;
			return {
				text: el.value,
				anchor: outAnchor,
				focus: outFocus,
				selected: el.value.slice(Math.min(selStart, selEnd), Math.max(selStart, selEnd)),
				clipboard,
				caret: { x: caretX, y: 0, w: 1, h: 16 },
			};
		},
		{ text: c.text, anchor: c.anchor, focus: c.focus, ops: c.ops, charW: CHAR_W },
	);
	await browser.close();
	return result;
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

async function main() {
	ensurePlaywright();
	const wasm = await loadWasm();
	const { chromium } = await import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);

	mkdirSync(OUT, { recursive: true });
	const results = [];
	let failed = 0;
	for (const c of CASES) {
		const script = {
			text: c.text,
			anchor: c.anchor,
			focus: c.focus,
			ops: c.ops,
			charWidth: CHAR_W,
			lineHeight: LINE_H,
		};
		const rust = JSON.parse(wasm.textSurfaceEval(JSON.stringify(script)));
		const ts = await textareaOracle(chromium, c);
		const selOk = rust.text === ts.text && rust.focus === ts.focus && rust.anchor === ts.anchor;
		// Caret x: Rust uses focus; compare within 1px.
		const caretOk = rectsWithin(
			{ x: rust.caret.x, y: rust.caret.y, w: rust.caret.w, h: rust.caret.h },
			{ x: ts.caret.x, y: ts.caret.y, w: ts.caret.w, h: LINE_H },
			CARET_TOL,
		);
		const ok = selOk && caretOk;
		if (!ok) failed++;
		const summary = ok
			? `OK ${c.id}: text/sel match; caret ±${CARET_TOL}px`
			: `FAIL ${c.id}: rust=${JSON.stringify({ text: rust.text, a: rust.anchor, f: rust.focus, caret: rust.caret })} ts=${JSON.stringify({ text: ts.text, a: ts.anchor, f: ts.focus, caret: ts.caret })}`;
		console.log(summary);
		results.push({ id: c.id, ok, summary, rust, ts });
	}

	writeFileSync(join(OUT, 'caret-parity.json'), JSON.stringify(results, null, 2));
	const gate = failed === 0
		? `text-caret-parity: PASS (${CASES.length}/${CASES.length})`
		: `text-caret-parity: FAIL (${CASES.length - failed}/${CASES.length})`;
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
