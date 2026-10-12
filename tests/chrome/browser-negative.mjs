// SPDX-License-Identifier: MIT
/**
 * Browser negative control for editor chrome (#53).
 * Readonly must not paint a Ribbon; edit mode must.
 *
 *   bun tests/chrome/browser-negative.mjs
 */
import { existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';
import { startVisualServer } from '../../parity/visual/server.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const VISUAL = join(ROOT, 'parity/visual');
const FIXTURE = 'fixtures/real/xlsx/poi-45544.xlsx';

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

async function loadChromium() {
	const mod = await import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);
	return mod.chromium;
}

function ensurePlaywright() {
	const entry = join(VISUAL, 'node_modules/playwright/index.mjs');
	if (existsSync(entry)) return;
	spawnSync('bun', ['install'], { cwd: VISUAL, encoding: 'utf8' });
	spawnSync('bunx', ['playwright', 'install', 'chromium'], { cwd: VISUAL, encoding: 'utf8' });
}

async function probe(page, base, readonly) {
	const q = new URLSearchParams({
		side: 'rust',
		id: 'chrome-neg',
		fixture: `/repo/${FIXTURE}`,
		width: '960',
		height: '200',
	});
	if (!readonly) q.set('edit', '1');
	await page.goto(`${base}/repo/tests/a11y/pages/xlsx-a11y.html?${q}`, {
		waitUntil: 'domcontentloaded',
		timeout: 60_000,
	});
	await page.waitForFunction(
		() => {
			const el = document.getElementById('stage');
			return el && (el.dataset.ready === '1' || el.dataset.ready === 'err');
		},
		null,
		{ timeout: 60_000 },
	);
	const ready = await page.evaluate(() => document.getElementById('stage')?.dataset.ready);
	if (ready !== '1') {
		const body = await page.evaluate(() => document.getElementById('stage')?.textContent?.slice(0, 400));
		throw new Error(`host not ready: ${body}`);
	}
	return page.evaluate(() => {
		const host = document.querySelector('nitro-xlsx');
		const root = host?.shadowRoot;
		const html = root?.querySelector('.chrome')?.innerHTML || '';
		return {
			hasRibbon: html.includes('aria-label="Ribbon"') || !!root?.querySelector('[aria-label="Ribbon"]'),
			hasBold: html.includes('data-g="home.bold"') || !!root?.querySelector('[data-g="home.bold"]'),
		};
	});
}

async function main() {
	ensureWasm();
	ensurePlaywright();
	const chromium = await loadChromium();
	const server = await startVisualServer({ root: VISUAL });
	const browser = await chromium.launch({ headless: true });
	try {
		const page = await browser.newPage();
		const ro = await probe(page, server.url, true);
		if (ro.hasRibbon || ro.hasBold) {
			throw new Error(`negative control FAIL: ribbon present in readonly (${JSON.stringify(ro)})`);
		}
		const ed = await probe(page, server.url, false);
		if (!ed.hasRibbon || !ed.hasBold) {
			throw new Error(`edit mode missing ribbon (${JSON.stringify(ed)})`);
		}
		// Explicit broken-probe: a forged readonly HTML with Ribbon would fail the check above.
		console.log('chrome-browser-negative: PASS (readonly empty; edit has Ribbon+Bold)');
	} finally {
		await browser.close();
		await server.close();
	}
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
	main().catch((e) => {
		console.error(String(e && e.stack ? e.stack : e));
		process.exit(1);
	});
}
