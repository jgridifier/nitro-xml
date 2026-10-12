// SPDX-License-Identifier: MIT
/**
 * Edit-surface accessibility gate (#59): axe + AX-tree + live region vs TS oracle.
 */

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import {
	A11Y,
	ROOT,
	loadChromium,
	startA11yServer,
	ensureVisualBundles,
} from '../host.mjs';

const EDIT = dirname(fileURLToPath(import.meta.url));
const OUT = join(EDIT, 'out');
const AXE_SRC = join(A11Y, 'node_modules/axe-core/axe.min.js');

const SCRIPT_INSERT = {
	text: '=',
	anchor: 1,
	focus: 1,
	ops: [{ op: 'insert', text: 'A1' }],
};

const SCRIPT_READONLY = {
	text: 'ro',
	anchor: 0,
	focus: 0,
	readonly: true,
	ops: [{ op: 'insert', text: 'X' }],
};

/** @type {{ id: string, label: string, readonly: boolean, script: object, expectTextbox: boolean, expectLive: boolean }[]} */
const CASES = [
	{
		id: 'formula-bar-insert',
		label: 'Formula Bar',
		readonly: false,
		script: SCRIPT_INSERT,
		expectTextbox: true,
		expectLive: true,
	},
	{
		id: 'cell-editor-insert',
		label: 'Cell editor',
		readonly: false,
		script: { ...SCRIPT_INSERT, text: 'hi', anchor: 2, focus: 2, ops: [{ op: 'insert', text: '!' }] },
		expectTextbox: true,
		expectLive: true,
	},
	{
		id: 'readonly-negative',
		label: 'Formula Bar',
		readonly: true,
		script: SCRIPT_READONLY,
		expectTextbox: false,
		expectLive: false,
	},
];

function editHostUrl(base, side, c) {
	const q = new URLSearchParams({
		side,
		label: c.label,
		readonly: c.readonly ? '1' : '0',
		script: JSON.stringify({ ...c.script, readonly: c.readonly, label: c.label }),
	});
	return `${base}/repo/tests/a11y/edit/edit-host.html?${q}`;
}

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

async function loadWasmAnnounce(script) {
	ensureWasm();
	const pkgDir = join(ROOT, 'packages/nitro-xml/pkg');
	const mod = await import(pathToFileURL(join(pkgDir, 'nitro_xml_wasm.js')).href);
	if (typeof mod.default === 'function') await mod.default();
	const raw = mod.a11yEval(JSON.stringify(script));
	return JSON.parse(raw);
}

async function waitEditReady(page) {
	await page.waitForFunction(() => document.getElementById('stage')?.dataset.ready === '1', null, {
		timeout: 30_000,
	});
}

/**
 * @param {import('playwright').Page} page
 */
async function runAxe(page) {
	await page.addScriptTag({ path: AXE_SRC });
	return page.evaluate(async () => {
		const results = await axe.run(document, {
			iframes: false,
			resultTypes: ['violations'],
			runOnly: {
				type: 'tag',
				values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'],
			},
		});
		return (results.violations || []).filter(
			(v) => v.impact === 'serious' || v.impact === 'critical',
		);
	});
}

/**
 * @param {import('playwright').Page} page
 */
async function snapshotTextboxes(page) {
	const client = await page.context().newCDPSession(page);
	await client.send('Accessibility.enable');
	const { nodes } = await client.send('Accessibility.getFullAXTree');
	await client.detach();
	return (nodes || [])
		.map((n) => ({
			role: String(n.role?.value ?? n.role ?? ''),
			name: String((n.name?.value ?? n.name ?? '').trim()),
			value: String((n.value?.value ?? n.value ?? '').trim()),
		}))
		.filter((n) => n.role === 'textbox' || n.role === 'textbox multiline');
}

async function main() {
	ensureVisualBundles();
	ensureWasm();
	const chromium = await loadChromium();
	const server = await startA11yServer();
	const base = server.url;

	const results = [];
	let failed = 0;

	const browser = await chromium.launch({ headless: true });

	for (const c of CASES) {
		const rustExpected = await loadWasmAnnounce({
			...c.script,
			label: c.label,
			readonly: c.readonly,
		});

		const context = await browser.newContext();
		const page = await context.newPage();

		// --- rust side: DOM + AX + axe ---
		await page.goto(editHostUrl(base, 'rust', c), { waitUntil: 'domcontentloaded' });
		await waitEditReady(page);
		const liveRust = await page.evaluate(() => document.getElementById('edit-live')?.textContent ?? '');
		const domRust = await page.evaluate(() => {
			const input = document.querySelector('.xg-field-input');
			return {
				label: input?.getAttribute('aria-label') ?? '',
				value: input?.value ?? '',
				present: !!input,
				role: input?.getAttribute('role') ?? '',
			};
		});
		const axRust = await snapshotTextboxes(page);
		const axeRust = await runAxe(page);

		// --- TS oracle (same page, reload) ---
		await page.goto(editHostUrl(base, 'ts', c), { waitUntil: 'domcontentloaded' });
		await waitEditReady(page);
		const liveTs = await page.evaluate(() => document.getElementById('edit-live')?.textContent ?? '');
		const axTs = await snapshotTextboxes(page);
		const axeTs = await runAxe(page);

		await context.close();

		const liveOk = c.expectLive
			? liveRust.length > 0 && liveRust === rustExpected.live
			: liveRust === '' && rustExpected.live === '';

		const textboxOk = c.expectTextbox
			? domRust.role === 'textbox' &&
				domRust.label === c.label &&
				domRust.value === rustExpected.textbox.value &&
				axRust.some((n) => n.name === c.label)
			: !domRust.present && axRust.length === 0 && rustExpected.textbox.hidden;

		const tsAxNameOk = c.expectTextbox
			? axTs.some((n) => n.name === c.label)
			: axTs.length === 0;

		const newViolations = axeRust.filter((v) => !axeTs.some((t) => t.id === v.id));
		const axeOk = newViolations.length === 0;

		const parityLive = !c.expectLive || liveTs === liveRust;

		const ok = liveOk && textboxOk && tsAxNameOk && axeOk && parityLive;
		if (!ok) failed++;
		const summary = ok
			? `OK ${c.id}: textbox=${c.expectTextbox} live="${liveRust}" axe+AX`
			: `FAIL ${c.id}: liveOk=${liveOk} textboxOk=${textboxOk} tsAx=${tsAxNameOk} axeOk=${axeOk} parityLive=${parityLive} rustLive="${liveRust}" expected="${rustExpected.live}" axRust=${JSON.stringify(axRust)} dom=${JSON.stringify(domRust)} newAxe=${newViolations.map((v) => v.id).join(',')}`;
		console.log(summary);
		results.push({ id: c.id, ok, summary });
	}

	await browser.close();
	await server.close();

	mkdirSync(OUT, { recursive: true });
	const gate = failed === 0
		? `a11y-edit: PASS (${CASES.length}/${CASES.length})`
		: `a11y-edit: FAIL (${CASES.length - failed}/${CASES.length})`;
	writeFileSync(join(OUT, 'gate-summary.txt'), gate + '\n');
	console.log(gate);
	process.exit(failed ? 1 : 0);
}

await main();
