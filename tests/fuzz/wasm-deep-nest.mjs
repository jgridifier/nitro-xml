// SPDX-License-Identifier: MIT
/**
 * Browser WASM open-and-render for MAX_XML_DEPTH fixtures (#30 / #178).
 *
 * Loads deep-nest-4096.xlsx and deep-nest-over-max.xlsx through the shipped
 * wasm package in Chromium (not the native parser alone) and asserts each
 * path completes without a WASM trap.
 *
 *   bun tests/fuzz/wasm-deep-nest.mjs
 */

import { existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { startVisualServer } from '../../parity/visual/server.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const VISUAL = join(ROOT, 'parity/visual');

const CASES = [
	{
		id: 'deep-nest-4096',
		fixture: 'fixtures/fuzz/xlsx/deep-nest-4096.xlsx',
		expect: 'open',
	},
	{
		id: 'deep-nest-over-max',
		fixture: 'fixtures/fuzz/xlsx/deep-nest-over-max.xlsx',
		expect: 'reject',
	},
];

function ensureWasm() {
	const glue = join(ROOT, 'packages/nitro-xml/pkg/nitro_xml_wasm.js');
	const wasm = join(ROOT, 'packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm');
	if (existsSync(glue) && existsSync(wasm)) return;
	const r = spawnSync('bash', [join(ROOT, 'scripts/build-wasm.sh')], {
		cwd: ROOT,
		encoding: 'utf8',
		maxBuffer: 32 * 1024 * 1024,
	});
	if (r.status !== 0) throw new Error(`build-wasm: ${r.stderr || r.stdout}`);
}

async function loadChromium() {
	const mod = await import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);
	return mod.chromium;
}

function ensurePlaywright() {
	const entry = join(VISUAL, 'node_modules/playwright/index.mjs');
	if (existsSync(entry)) return;
	const install = spawnSync('bun', ['install'], {
		cwd: VISUAL,
		encoding: 'utf8',
		maxBuffer: 32 * 1024 * 1024,
	});
	if (install.status !== 0) {
		throw new Error(`parity/visual bun install: ${install.stderr || install.stdout}`);
	}
	const pw =
		spawnSync('bunx', ['playwright', 'install', 'chromium'], {
			cwd: VISUAL,
			encoding: 'utf8',
			maxBuffer: 32 * 1024 * 1024,
		}).status === 0
			? null
			: spawnSync('bunx', ['playwright', 'install', '--with-deps', 'chromium'], {
					cwd: VISUAL,
					encoding: 'utf8',
					maxBuffer: 32 * 1024 * 1024,
				});
	if (pw && pw.status !== 0) {
		throw new Error(`playwright install: ${pw.stderr || pw.stdout}`);
	}
}

/**
 * @returns {Promise<{ ok: boolean, rows: object[], summary: string }>}
 */
export async function runWasmDeepNest() {
	ensureWasm();
	ensurePlaywright();
	const chromium = await loadChromium();
	const server = await startVisualServer({ root: VISUAL });
	const browser = await chromium.launch({ headless: true });
	const rows = [];
	let failed = 0;
	try {
		for (const c of CASES) {
			const page = await browser.newPage();
			const q = new URLSearchParams({
				fixture: `/repo/${c.fixture}`,
				expect: c.expect,
			});
			const url = `${server.url}/repo/tests/fuzz/pages/xlsx-deep-nest.html?${q}`;
			let payload = null;
			try {
				await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 60_000 });
				await page.waitForFunction(
					() => document.documentElement.dataset.ready === '1' || document.documentElement.dataset.ready === 'err',
					null,
					{ timeout: 60_000 },
				);
				payload = await page.evaluate(() => window.__deepNest);
			} catch (e) {
				payload = {
					ok: false,
					outcome: 'browser-error',
					error: String(e && e.message ? e.message : e),
				};
			}
			await page.close();
			const ok = !!(payload && payload.ok);
			if (!ok) failed++;
			rows.push({ id: c.id, expect: c.expect, ok, ...payload });
		}
	} finally {
		await browser.close();
		await server.close();
	}
	const summary = failed
		? `wasm-deep-nest: FAIL ${failed}/${rows.length}`
		: `wasm-deep-nest: PASS ${rows.length}/${rows.length} (open+paint / reject, no trap)`;
	return { ok: failed === 0, rows, summary };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
	const { ok, rows, summary } = await runWasmDeepNest();
	console.log(summary);
	for (const r of rows) {
		console.log(`- ${r.id}: ${r.ok ? 'ok' : 'FAIL'} (${r.outcome || '?'})`);
	}
	process.exit(ok ? 0 : 1);
}
