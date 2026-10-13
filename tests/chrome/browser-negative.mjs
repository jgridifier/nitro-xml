// SPDX-License-Identifier: MIT
/**
 * Browser negative control for editor chrome (#53 / #54).
 * Readonly must not paint Ribbon, dialog, or menu overlays; edit mode must
 * expose Ribbon and can open dialog/menu via chromeOverlayEval.
 *
 *   bun tests/chrome/browser-negative.mjs
 */
import { existsSync, readFileSync } from 'node:fs';
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

async function loadWasm() {
	ensureWasm();
	const pkgDir = join(ROOT, 'packages/nitro-xml/pkg');
	const mod = await import(pathToFileURL(join(pkgDir, 'nitro_xml_wasm.js')).href);
	if (typeof mod.default === 'function') await mod.default();
	else if (typeof mod.initSync === 'function') {
		mod.initSync({ module: readFileSync(join(pkgDir, 'nitro_xml_wasm_bg.wasm')) });
	}
	if (typeof mod.chromeOverlayEval !== 'function') {
		throw new Error('chromeOverlayEval missing from wasm exports');
	}
	return mod;
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
			hasDialog: html.includes('role="dialog"') || !!root?.querySelector('[role="dialog"]'),
			hasMenu: html.includes('role="menu"') || !!root?.querySelector('[role="menu"]'),
			hasFormulaBar:
				html.includes('part="formula-bar"') || !!root?.querySelector('[part="formula-bar"]'),
		};
	});
}

async function main() {
	ensurePlaywright();
	const wasm = await loadWasm();

	// Headless overlay negative: readonly open* must not produce dialog/menu HTML.
	const roOverlay = JSON.parse(
		wasm.chromeOverlayEval(JSON.stringify({ readOnly: true, op: 'openDialog' })),
	);
	if (roOverlay.hasDialog || roOverlay.hasMenu || roOverlay.hasOverlay) {
		throw new Error(`overlay negative FAIL readonly dialog: ${JSON.stringify(roOverlay)}`);
	}
	const roMenu = JSON.parse(
		wasm.chromeOverlayEval(JSON.stringify({ readOnly: true, op: 'openContextMenu' })),
	);
	if (roMenu.hasDialog || roMenu.hasMenu || roMenu.hasOverlay) {
		throw new Error(`overlay negative FAIL readonly menu: ${JSON.stringify(roMenu)}`);
	}
	const edDialog = JSON.parse(
		wasm.chromeOverlayEval(
			JSON.stringify({ readOnly: false, op: 'openDialog', name: 'confirm', heading: 'Confirm' }),
		),
	);
	if (!edDialog.hasDialog || !edDialog.hasRibbon) {
		throw new Error(`edit dialog missing: ${JSON.stringify(edDialog)}`);
	}
	const edMenu = JSON.parse(
		wasm.chromeOverlayEval(
			JSON.stringify({ readOnly: false, op: 'openContextMenu', x: 40, y: 40, vw: 375, vh: 667 }),
		),
	);
	if (!edMenu.hasMenu) {
		throw new Error(`edit menu missing: ${JSON.stringify(edMenu)}`);
	}

	const chromium = await loadChromium();
	const server = await startVisualServer({ root: VISUAL });
	const browser = await chromium.launch({ headless: true });
	try {
		const page = await browser.newPage();
		const ro = await probe(page, server.url, true);
		if (ro.hasRibbon || ro.hasBold || ro.hasDialog || ro.hasMenu || ro.hasFormulaBar) {
			throw new Error(`negative control FAIL: chrome present in readonly (${JSON.stringify(ro)})`);
		}
		const ed = await probe(page, server.url, false);
		if (!ed.hasRibbon || !ed.hasBold || !ed.hasFormulaBar) {
			throw new Error(`edit mode missing ribbon or formula bar (${JSON.stringify(ed)})`);
		}
		if (ed.hasDialog || ed.hasMenu) {
			throw new Error(`edit mode unexpectedly open overlay (${JSON.stringify(ed)})`);
		}
		console.log(
			'chrome-browser-negative: PASS (readonly empty; edit Ribbon+formula bar; overlay eval dialog/menu)',
		);
	} finally {
		await browser.close();
		await server.close();
	}
}

try {
	await main();
} catch (e) {
	console.error(String(e && e.stack ? e.stack : e));
	process.exit(1);
}
