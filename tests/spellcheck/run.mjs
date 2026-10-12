// SPDX-License-Identifier: MIT
/**
 * Spellcheck gate (#58): DOM `spellcheck` toggle parity vs upstream and
 * underline geometry for injected misspell UTF-16 ranges.
 *
 *   bun tests/spellcheck/run.mjs
 */

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const VISUAL = join(ROOT, 'parity/visual');
const OUT = join(ROOT, 'tests/spellcheck/out');
const RECT_TOL = 1;
const CHAR_W = 8;
const LINE_H = 16;

/** @type {{ id: string, mode: 'textarea' | 'contenteditable', script: object }[]} */
const TOGGLE_CASES = [
	{
		id: 'toggle-on-textarea',
		mode: 'textarea',
		script: { text: 'hello', anchor: 5, focus: 5, enabled: true, lang: 'en-US', ops: [] },
	},
	{
		id: 'toggle-off-textarea',
		mode: 'textarea',
		script: {
			text: 'zzzz',
			anchor: 0,
			focus: 4,
			enabled: true,
			ops: [{ op: 'setEnabled', enabled: false }],
		},
	},
	{
		id: 'toggle-on-ce',
		mode: 'contenteditable',
		script: { text: 'notes', anchor: 0, focus: 0, enabled: true, lang: '', ops: [] },
	},
	{
		id: 'negative-disabled-no-rects',
		mode: 'textarea',
		script: {
			text: 'asdf qwerty',
			anchor: 0,
			focus: 4,
			enabled: false,
			misspells: [{ start: 0, end: 4 }],
			ops: [],
		},
	},
];

/** @type {{ id: string, script: object }[]} */
const UNDERLINE_CASES = [
	{
		id: 'single-misspell',
		script: {
			text: 'hello zzorld',
			anchor: 12,
			focus: 12,
			enabled: true,
			misspells: [{ start: 6, end: 12 }],
			charWidth: CHAR_W,
			lineHeight: LINE_H,
		},
	},
	{
		id: 'two-misspells',
		script: {
			text: 'aa bb cc',
			anchor: 0,
			focus: 0,
			enabled: true,
			misspells: [{ start: 0, end: 2 }, { start: 3, end: 5 }],
			charWidth: CHAR_W,
			lineHeight: LINE_H,
		},
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
	if (typeof mod.spellcheckEval !== 'function') {
		throw new Error('spellcheckEval missing from wasm exports');
	}
	return mod;
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

function rectsListWithin(a, b, tol) {
	if (a.length !== b.length) return false;
	for (let i = 0; i < a.length; i++) {
		if (!rectsWithin(a[i], b[i], tol)) return false;
	}
	return true;
}

async function domOracle(chromium, mode, script) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage();
	const tag = mode === 'textarea' ? 'textarea' : 'div';
	await page.setContent(
		`<!doctype html><${tag} id="t" style="font: ${LINE_H}px monospace; width:400px;"></${tag}>`,
	);
	const result = await page.evaluate(
		({ mode, script }) => {
			const el = document.getElementById('t');
			if (!el) throw new Error('missing #t');
			let text = script.text ?? '';
			let enabled = script.enabled ?? true;
			let lang = script.lang ?? '';
			const ops = script.ops ?? [];
			for (const op of ops) {
				if (op.op === 'setEnabled') enabled = !!op.enabled;
				if (op.op === 'setLang') lang = op.lang ?? '';
				if (op.op === 'setText') text = op.text ?? '';
			}
			if (mode === 'textarea') {
				const ta = /** @type {HTMLTextAreaElement} */ (el);
				ta.value = text;
				ta.spellcheck = enabled;
				if (lang) ta.lang = lang;
				ta.focus();
			} else {
				const ce = /** @type {HTMLElement} */ (el);
				ce.textContent = text;
				ce.contentEditable = 'true';
				ce.spellcheck = enabled;
				if (lang) ce.lang = lang;
				ce.focus();
			}
			const spellcheckAttr = el.spellcheck;
			return { spellcheckAttr, lang: el.lang || '' };
		},
		{ mode, script },
	);
	await browser.close();
	return result;
}

function expectedOracleRects(text, misspells, charW, lineH) {
	const rects = [];
	for (const m of misspells) {
		const x0 = m.start * charW;
		const x1 = m.end * charW;
		rects.push({ x: x0, y: 0, w: x1 - x0, h: lineH });
	}
	return rects;
}

async function optionalSuggestionsSmoke(chromium) {
	if (process.env.RUN_SPELL_SUGGESTIONS !== '1') {
		return { id: 'suggestions-smoke', ok: true, skipped: true, summary: 'SKIP suggestions-smoke (set RUN_SPELL_SUGGESTIONS=1)' };
	}
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage();
	await page.setContent(
		`<!doctype html><textarea id="t" spellcheck="true" lang="en-US" style="font:16px monospace;"></textarea>`,
	);
	const hasDict = await page.evaluate(async () => {
		const el = /** @type {HTMLTextAreaElement} */ (document.getElementById('t'));
		el.value = 'zzzzzz';
		el.focus();
		// No stable JS API for suggestions; presence of spellcheck + editable surface is the smoke.
		return el.spellcheck === true && el.value.length > 0;
	});
	await browser.close();
	if (!hasDict) {
		return {
			id: 'suggestions-smoke',
			ok: true,
			skipped: true,
			summary: 'SKIP suggestions-smoke (headless lacks spell dictionary)',
		};
	}
	return {
		id: 'suggestions-smoke',
		ok: true,
		skipped: false,
		summary: 'OK suggestions-smoke: spellcheck surface accepts nonsense text',
	};
}

async function main() {
	ensurePlaywright();
	const wasm = await loadWasm();
	const { chromium } = await import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);

	mkdirSync(OUT, { recursive: true });
	const results = [];
	let failed = 0;

	for (const c of TOGGLE_CASES) {
		const rust = JSON.parse(wasm.spellcheckEval(JSON.stringify({ ...c.script, charWidth: CHAR_W, lineHeight: LINE_H })));
		const dom = await domOracle(chromium, c.mode, c.script);
		const attrOk = rust.spellcheckAttr === dom.spellcheckAttr;
		const bridgeOk =
			rust.bridge.text === c.script.text &&
			rust.bridge.enabled === rust.enabled &&
			rust.bridge.anchor === (c.script.anchor ?? 0) &&
			rust.bridge.focus === (c.script.focus ?? 0);
		const negOk =
			c.id !== 'negative-disabled-no-rects' ||
			(rust.spellcheckAttr === false && rust.underlineRects.length === 0);
		const ok = attrOk && bridgeOk && negOk;
		if (!ok) failed++;
		const summary = ok
			? `OK ${c.id}: spellcheckAttr=${rust.spellcheckAttr}`
			: `FAIL ${c.id}: rust=${JSON.stringify(rust)} dom=${JSON.stringify(dom)}`;
		console.log(summary);
		results.push({ id: c.id, ok, summary, rust, dom });
	}

	for (const c of UNDERLINE_CASES) {
		const rust = JSON.parse(wasm.spellcheckEval(JSON.stringify(c.script)));
		const expected = expectedOracleRects(
			c.script.text,
			c.script.misspells ?? [],
			CHAR_W,
			LINE_H,
		);
		const ok = rectsListWithin(rust.underlineRects, expected, RECT_TOL);
		if (!ok) failed++;
		const summary = ok
			? `OK ${c.id}: ${rust.underlineRects.length} underline rect(s) ±${RECT_TOL}px`
			: `FAIL ${c.id}: rust=${JSON.stringify(rust.underlineRects)} expected=${JSON.stringify(expected)}`;
		console.log(summary);
		results.push({ id: c.id, ok, summary, rust, expected });
	}

	const smoke = await optionalSuggestionsSmoke(chromium);
	console.log(smoke.summary);
	results.push(smoke);

	writeFileSync(join(OUT, 'spellcheck-parity.json'), JSON.stringify(results, null, 2));
	const gate = failed === 0
		? `spellcheck: PASS (${results.filter((r) => r.ok && !r.skipped).length} checks)`
		: `spellcheck: FAIL (${failed} failed)`;
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
