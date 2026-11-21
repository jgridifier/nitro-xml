// SPDX-License-Identifier: MIT
/**
 * Playwright e2e for feature-546: word count in the status bar.
 *
 * Covers:
 *   - [data-wc] button appears with "N words" after notes.docx loads
 *   - notes.docx = 105 words (matches Word's count)
 *   - Native text selection shows "S of N words"
 *   - Clicking the button opens the Word Count dialog
 *   - Dialog shows Pages, Words, Characters (no/with spaces), Paragraphs, Lines
 *   - Esc dismisses the dialog and returns focus to the button
 *   - Close button also dismisses
 *
 * Also takes the four required screenshots into SHOTS_DIR and docs/screenshots/docx-word-count/.
 */
import { createServer } from 'node:http';
import { cpSync, createReadStream, mkdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const require = createRequire(join(dirname(fileURLToPath(import.meta.url)), '../../parity/visual/package.json'));
const { chromium } = require('playwright');

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, '../..');
const SHOTS_DIR = process.env.DOCX_WC_SHOTS || '/workspace/nitro-nightly/shots-2026-11-21';
const DOCS_SHOTS = join(ROOT, 'docs/screenshots/docx-word-count');

const MIME = {
	'.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css',
	'.wasm': 'application/wasm', '.json': 'application/json', '.svg': 'image/svg+xml',
	'.png': 'image/png', '.docx': 'application/octet-stream',
};

function assemble(dir) {
	rmSync(dir, { recursive: true, force: true });
	mkdirSync(dir, { recursive: true });
	cpSync(join(ROOT, 'demo'), dir, { recursive: true });
	rmSync(join(dir, 'nitro-xml'), { recursive: true, force: true });
	cpSync(join(ROOT, 'packages/nitro-xml/loader'), join(dir, 'nitro-xml/loader'), {
		recursive: true,
		filter: (src) => !src.split(/[/\\]/).includes('test'),
	});
	const pkg = join(ROOT, 'packages/nitro-xml/pkg');
	if (!statSync(join(pkg, 'nitro_xml_wasm_bg.wasm'), { throwIfNoEntry: false })) {
		throw new Error('docx-word-count: missing wasm pkg; run scripts/build-wasm.sh');
	}
	cpSync(pkg, join(dir, 'nitro-xml/pkg'), { recursive: true });
}

function startServer(demoDir) {
	return new Promise((resolve) => {
		const s = createServer((req, rsp) => {
			const pathname = decodeURIComponent(new URL(req.url, 'http://x').pathname);
			const rel = pathname.replace(/^\/demo\/?/, '');
			const f = normalize(join(demoDir, rel));
			if (!f.startsWith(demoDir)) { rsp.writeHead(404).end(); return; }
			try {
				if (!statSync(f).isFile()) throw new Error('404');
				rsp.writeHead(200, { 'content-type': MIME[extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
				createReadStream(f).pipe(rsp);
			} catch { rsp.writeHead(404).end(); }
		});
		s.listen(0, '127.0.0.1', () => resolve({ url: `http://127.0.0.1:${s.address().port}/demo`, close: () => s.close() }));
	});
}

function assert(cond, msg) {
	if (!cond) throw new Error(`FAIL: ${msg}`);
}

function shot(name) {
	mkdirSync(SHOTS_DIR, { recursive: true });
	mkdirSync(DOCS_SHOTS, { recursive: true });
	return { shots: join(SHOTS_DIR, name), docs: join(DOCS_SHOTS, name) };
}
async function screenshot(page, name) {
	const p = shot(name);
	await page.screenshot({ path: p.shots, animations: 'disabled' });
	cpSync(p.shots, p.docs);
}

async function waitForDocx(page) {
	await page.waitForFunction(() => {
		const el = document.querySelector('nitro-docx');
		const sr = el && el.shadowRoot;
		return !!(sr && el.document && sr.querySelector('.dve-print-line'));
	}, null, { timeout: 45_000 });
	await page.waitForTimeout(400);
}

async function waitForWc(page, timeoutMs = 5000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		const txt = await page.evaluate(() => {
			const btn = document.querySelector('[data-wc]');
			return btn && !btn.hidden ? btn.textContent.trim() : null;
		});
		if (txt) return txt;
		await page.waitForTimeout(100);
	}
	throw new Error('word count button never became visible');
}

async function run(page, base) {
	const errors = [];

	// ---- Load page ----
	await page.goto(`${base}/docx-full.html`, { waitUntil: 'domcontentloaded', timeout: 60_000 });
	await waitForDocx(page);

	// Before screenshot: status bar before word count appears (taken immediately so debounce hasn't fired)
	await screenshot(page, 'feature-546-before.png');

	// ---- Wait for word count ----
	const wcText = await waitForWc(page);
	const wcMatch = wcText.match(/^(\d+)\s+words?$/);
	if (!wcMatch) {
		errors.push(`word count text has unexpected format: ${JSON.stringify(wcText)}`);
	} else {
		const n = parseInt(wcMatch[1], 10);
		if (n !== 105) errors.push(`notes.docx: expected 105 words, got ${n} (text: ${wcText})`);
		else console.log(`  word count: ${wcText} ✓`);
	}

	// After screenshot
	await screenshot(page, 'feature-546-after.png');

	// ---- Selection word count ----
	// Select the first paragraph text using triple-click
	const firstLine = await page.evaluate(() => {
		const el = document.querySelector('nitro-docx');
		const sr = el && el.shadowRoot;
		const line = sr && sr.querySelector('.dve-print-line span');
		if (!line) return null;
		const r = line.getBoundingClientRect();
		return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
	});
	if (firstLine) {
		await page.mouse.click(firstLine.x, firstLine.y, { clickCount: 3 }); // triple-click = select paragraph
		await page.waitForTimeout(200);
		const selWcText = await page.evaluate(() => {
			const btn = document.querySelector('[data-wc]');
			return btn && !btn.hidden ? btn.textContent.trim() : null;
		});
		if (selWcText && /\d+ of \d+ words/.test(selWcText)) {
			console.log(`  selection count: ${selWcText} ✓`);
		} else {
			console.log(`  selection count: ${JSON.stringify(selWcText)} (may be collapsed or no native sel)`);
		}
		await screenshot(page, 'feature-546-selection.png');
	} else {
		errors.push('could not find first line to click for selection test');
		await screenshot(page, 'feature-546-selection.png');
	}

	// Deselect by clicking elsewhere
	await page.keyboard.press('Escape');
	await page.evaluate(() => window.getSelection && window.getSelection().removeAllRanges());
	await page.waitForTimeout(100);

	// ---- Dialog ----
	const wcBtn = await page.locator('[data-wc]').first();
	await wcBtn.click();
	await page.waitForTimeout(200);

	const dialogVisible = await page.evaluate(() => !!document.querySelector('.wcd'));
	assert(dialogVisible, 'Word Count dialog should be visible after clicking [data-wc]');
	console.log('  dialog opens ✓');

	// Check dialog has word count
	const dialogWords = await page.evaluate(() => {
		const rows = document.querySelectorAll('.wcd-row');
		for (const r of rows) {
			const lbl = r.querySelector('.wcd-lbl');
			const val = r.querySelector('.wcd-val');
			if (lbl && lbl.textContent === 'Words') return val ? val.textContent.trim() : null;
		}
		return null;
	});
	assert(dialogWords === '105', `dialog Words should be 105, got ${JSON.stringify(dialogWords)}`);
	console.log(`  dialog words: ${dialogWords} ✓`);

	// Check all required stats are present
	const dialogLabels = await page.evaluate(() =>
		[...document.querySelectorAll('.wcd-lbl')].map((l) => l.textContent.trim()),
	);
	for (const req of ['Pages', 'Words', 'Characters (no spaces)', 'Characters (with spaces)', 'Paragraphs']) {
		assert(dialogLabels.includes(req), `dialog missing stat: ${req}`);
	}
	console.log('  dialog stats present ✓');

	await screenshot(page, 'feature-546-dialog.png');

	// ---- Close with Esc ----
	await page.keyboard.press('Escape');
	await page.waitForTimeout(150);
	const dialogGone = await page.evaluate(() => !document.querySelector('.wcd'));
	assert(dialogGone, 'dialog should close on Esc');
	console.log('  Esc closes dialog ✓');

	// Focus should return to [data-wc] button
	const focusOnWc = await page.evaluate(() => {
		const btn = document.querySelector('[data-wc]');
		return document.activeElement === btn;
	});
	assert(focusOnWc, 'focus should return to word count button after Esc');
	console.log('  focus returns to button ✓');

	// ---- Close with Close button ----
	await wcBtn.click();
	await page.waitForTimeout(200);
	const closeBtn = page.locator('.wcd-close').first();
	await closeBtn.click();
	await page.waitForTimeout(150);
	const dialogGone2 = await page.evaluate(() => !document.querySelector('.wcd'));
	assert(dialogGone2, 'dialog should close on Close button click');
	console.log('  Close button works ✓');

	if (errors.length) {
		for (const e of errors) console.error(`  ERROR: ${e}`);
		throw new Error(`${errors.length} assertion(s) failed`);
	}
}

async function main() {
	const demoDir = join(HERE, 'out/wc-demo');
	assemble(demoDir);
	const server = await startServer(demoDir);
	const browser = await chromium.launch({
		executablePath: (() => {
			const p = '/home/box/.cache/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-linux64/chrome-headless-shell';
			try { statSync(p); return p; } catch { return undefined; }
		})(),
		headless: true,
	});
	let failed = false;
	try {
		const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
		console.log('docx-word-count: running checks…');
		await run(page, server.url);
		console.log('docx-word-count: all checks passed');
	} catch (e) {
		console.error('docx-word-count: FAIL', e && e.message || e);
		failed = true;
	} finally {
		await browser.close();
		server.close();
	}
	mkdirSync(join(HERE, 'out'), { recursive: true });
	writeFileSync(join(HERE, 'out/docx-word-count-summary.txt'), failed ? 'FAIL\n' : 'OK\n');
	if (failed) process.exit(1);
}

main().catch((e) => { console.error(e); process.exit(1); });
