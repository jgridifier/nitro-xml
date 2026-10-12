// SPDX-License-Identifier: MIT
/**
 * Mobile keyboard + touch selection handle gate (#60).
 *
 *   bun tests/mobile/run.mjs
 *
 * Playwright mobile emulation (iPhone / Pixel) plus headless wasm `mobileTextEval`.
 * iOS Safari / Android Chrome: see MANUAL.md.
 */

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const VISUAL = join(ROOT, 'parity/visual');
const OUT = join(dirname(fileURLToPath(import.meta.url)), 'out');
const CHAR_W = 8;
const LINE_H = 16;
/** @type {{ id: string, text: string, anchor: number, focus: number, ops: object[], expect?: object }[]} */
const CASES = [
	{
		id: 'type-and-delete',
		text: '',
		anchor: 0,
		focus: 0,
		ops: [
			{ op: 'config', mobile: true },
			{ op: 'beforeInput', inputType: 'insertText', data: 'hi' },
			{ op: 'beforeInput', inputType: 'deleteContentBackward' },
		],
		expect: { text: 'h', anchor: 1, focus: 1 },
	},
	{
		id: 'autocorrect-replacement',
		text: 'teh',
		anchor: 0,
		focus: 3,
		ops: [
			{ op: 'config', mobile: true },
			{ op: 'beforeInput', inputType: 'insertReplacementText', data: 'the' },
		],
		expect: { text: 'the', anchor: 3, focus: 3 },
	},
	{
		id: 'handle-drag-selection',
		text: 'abcdef',
		anchor: 1,
		focus: 4,
		ops: [
			{ op: 'config', mobile: true },
			{ op: 'handleDrag', which: 'start', utf16: 0 },
			{ op: 'handleDrag', which: 'end', utf16: 6 },
		],
		expect: { text: 'abcdef', anchor: 0, focus: 6, selected: 'abcdef' },
	},
	{
		id: 'negative-desktop-no-autocorrect',
		text: 'teh',
		anchor: 0,
		focus: 3,
		ops: [
			{ op: 'config', mobile: false },
			{ op: 'beforeInput', inputType: 'insertReplacementText', data: 'the' },
		],
		expect: { text: 'teh', anchor: 0, focus: 3, showHandles: false },
	},
	{
		id: 'negative-readonly',
		text: 'stay',
		anchor: 2,
		focus: 2,
		ops: [
			{ op: 'config', mobile: true, readonly: true },
			{ op: 'beforeInput', inputType: 'insertText', data: 'X' },
			{ op: 'handleDrag', which: 'end', utf16: 4 },
		],
		expect: { text: 'stay', anchor: 2, focus: 2, showHandles: false, mobileActive: false },
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
	const { readFileSync } = await import('node:fs');
	const wasmPath = join(pkgDir, 'nitro_xml_wasm_bg.wasm');
	if (typeof mod.initSync === 'function' && existsSync(wasmPath)) {
		mod.initSync({ module: readFileSync(wasmPath) });
	} else if (typeof mod.default === 'function') {
		await mod.default();
	}
	if (typeof mod.mobileTextEval !== 'function') {
		throw new Error('mobileTextEval missing from wasm exports');
	}
	return mod;
}

function runRustCase(wasm, c) {
	const script = {
		text: c.text,
		anchor: c.anchor,
		focus: c.focus,
		ops: c.ops,
		charWidth: CHAR_W,
		lineHeight: LINE_H,
	};
	return JSON.parse(wasm.mobileTextEval(JSON.stringify(script)));
}

/**
 * TS oracle: same beforeinput contract as the hidden-input bridge (UTF-16 indices).
 */
function tsOracle(c) {
	let text = c.text;
	let anchor = c.anchor;
	let focus = c.focus;
	let mobile = true;
	let readonly = false;

	const splice = (start, end, insert) => {
		const s = Math.min(start, end);
		const e = Math.max(start, end);
		text = text.slice(0, s) + insert + text.slice(e);
		const caret = s + insert.length;
		anchor = caret;
		focus = caret;
	};

	for (const op of c.ops) {
		if (op.op === 'config') {
			if (typeof op.mobile === 'boolean') mobile = op.mobile;
			if (typeof op.readonly === 'boolean') readonly = op.readonly;
		} else if (op.op === 'beforeInput') {
			if (readonly) continue;
			const type = op.inputType;
			if (type === 'insertReplacementText' && !mobile) continue;
			const data = op.data ?? '';
			if (
				type === 'insertText' ||
				type === 'insertReplacementText' ||
				type === 'insertFromPaste' ||
				type === 'insertCompositionText'
			) {
				splice(anchor, focus, data);
			} else if (type === 'deleteContentBackward') {
				if (anchor !== focus) splice(anchor, focus, '');
				else if (focus > 0) splice(focus - 1, focus, '');
			} else if (type === 'deleteContentForward') {
				if (anchor !== focus) splice(anchor, focus, '');
				else if (focus < text.length) splice(focus, focus + 1, '');
			}
		} else if (op.op === 'handleDrag') {
			if (!mobile || readonly) continue;
			const pos = op.utf16 ?? 0;
			if (op.which === 'start') anchor = pos;
			else focus = pos;
		}
	}

	const selected =
		anchor === focus
			? ''
			: text.slice(Math.min(anchor, focus), Math.max(anchor, focus));
	const active = mobile && !readonly;
	return {
		text,
		anchor,
		focus,
		selected,
		showHandles: active,
		mobileActive: active,
	};
}

function matchExpect(got, exp) {
	for (const [k, v] of Object.entries(exp)) {
		if (got[k] !== v) return false;
	}
	return true;
}

async function playwrightMobileSmoke(chromium, devices, wasm) {
	const results = [];
	for (const deviceName of ['iPhone 13', 'Pixel 5']) {
		const device = devices[deviceName];
		if (!device) {
			results.push({ device: deviceName, ok: false, summary: 'device descriptor missing' });
			continue;
		}
		const browser = await chromium.launch({ headless: true });
		const context = await browser.newContext({ ...device });
		const page = await context.newPage();
		await page.setContent(`<!doctype html>
<input id="bridge" type="text" inputmode="text" enterkeyhint="done"
  style="font: ${LINE_H}px monospace; width: 320px;" />`);
		const input = page.locator('#bridge');
		await input.focus();
		await page.keyboard.type('ab');
		await page.keyboard.press('Backspace');
		const domValue = await input.inputValue();
		const rust = runRustCase(wasm, {
			id: `pw-${deviceName}`,
			text: '',
			anchor: 0,
			focus: 0,
			ops: [
				{ op: 'config', mobile: true },
				{ op: 'beforeInput', inputType: 'insertText', data: 'a' },
				{ op: 'beforeInput', inputType: 'insertText', data: 'b' },
				{ op: 'beforeInput', inputType: 'deleteContentBackward' },
			],
		});
		const ok = domValue === rust.text && rust.text === 'a';
		results.push({
			device: deviceName,
			ok,
			summary: ok
				? `OK ${deviceName}: keyboard type+delete → "${rust.text}"`
				: `FAIL ${deviceName}: dom="${domValue}" rust="${rust.text}"`,
		});
		await browser.close();
	}
	return results;
}

async function main() {
	ensurePlaywright();
	const wasm = await loadWasm();
	const pw = await import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);
	const { chromium, devices } = pw;

	mkdirSync(OUT, { recursive: true });
	const rows = [];
	let failed = 0;

	for (const c of CASES) {
		const rust = runRustCase(wasm, c);
		const ts = tsOracle(c);
		const parityOk =
			rust.text === ts.text &&
			rust.anchor === ts.anchor &&
			rust.focus === ts.focus &&
			rust.selected === ts.selected;
		const expectOk = c.expect ? matchExpect(rust, c.expect) : true;
		const handlesOk =
			c.expect?.showHandles === undefined || rust.showHandles === c.expect.showHandles;
		const ok = parityOk && expectOk && handlesOk;
		if (!ok) failed++;
		const summary = ok
			? `OK ${c.id}: rust/ts parity`
			: `FAIL ${c.id}: rust=${JSON.stringify({
					text: rust.text,
					a: rust.anchor,
					f: rust.focus,
					showHandles: rust.showHandles,
				})} ts=${JSON.stringify({
					text: ts.text,
					a: ts.anchor,
					f: ts.focus,
					showHandles: ts.showHandles,
				})}`;
		console.log(summary);
		rows.push({ id: c.id, ok, summary, rust, ts });
	}

	const pwRows = await playwrightMobileSmoke(chromium, devices, wasm);
	for (const r of pwRows) {
		console.log(r.summary);
		if (!r.ok) failed++;
		rows.push({ id: `playwright-${r.device}`, ok: r.ok, summary: r.summary });
	}

	writeFileSync(join(OUT, 'mobile-parity.json'), JSON.stringify(rows, null, 2));
	const gate =
		failed === 0
			? `mobile-text: PASS (${rows.length}/${rows.length})`
			: `mobile-text: FAIL (${rows.length - failed}/${rows.length})`;
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
