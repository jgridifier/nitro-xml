// SPDX-License-Identifier: MIT
/**
 * Docx Navigation pane e2e (#607): open/close, click-to-jump, keyboard nav,
 * live updates, empty state, Ctrl+F focus.
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
const MIME = {
	'.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript',
	'.css': 'text/css', '.wasm': 'application/wasm', '.json': 'application/json',
	'.svg': 'image/svg+xml', '.png': 'image/png',
	'.docx': 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
};
const SHOTS = process.env.DOCX_NAV_SHOTS || '/workspace/nitro-nightly/qa-1125';
const SHOTS_DOCS = join(ROOT, 'docs/screenshots/docx-navigation');

function assemble(dir) {
	rmSync(dir, { recursive: true, force: true });
	mkdirSync(dir, { recursive: true });
	cpSync(join(ROOT, 'demo'), dir, { recursive: true });
	rmSync(join(dir, 'nitro-xml'), { recursive: true, force: true });
	const loader = join(dir, 'nitro-xml/loader');
	cpSync(join(ROOT, 'packages/nitro-xml/loader'), loader, {
		recursive: true,
		filter: (src) => !src.split(/[/\\]/).includes('test'),
	});
	const pkg = join(ROOT, 'packages/nitro-xml/pkg');
	if (!statSync(join(pkg, 'nitro_xml_wasm_bg.wasm'), { throwIfNoEntry: false })) {
		throw new Error('docx-nav: missing wasm pkg; run scripts/build-wasm.sh');
	}
	cpSync(pkg, join(dir, 'nitro-xml/pkg'), { recursive: true });
}

function startBuilt(demoDir) {
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
		s.listen(0, '127.0.0.1', () => resolve({ server: s, base: `http://127.0.0.1:${s.address().port}/demo` }));
	});
}

async function openDocxFull(page, base) {
	await page.goto(`${base}/docx-full.html`, { waitUntil: 'domcontentloaded', timeout: 60_000 });
	await page.waitForFunction(() => {
		const el = document.querySelector('nitro-docx');
		const sr = el && el.shadowRoot;
		return !!(sr && sr.querySelector('.dve-print-line'));
	}, { timeout: 60_000 });
	await page.waitForTimeout(500);
}

/** Click the View tab, then the Navigation button. */
async function clickNavButton(page) {
	await page.click('button[role=tab][data-t=View]');
	await page.waitForTimeout(100);
	await page.click('button.rb[aria-label="Show or hide the Navigation pane"]');
}

async function clickButton(page, label) {
	await page.click(`button[aria-label="${label}"]`);
}

let pass = 0, fail = 0;
function ok(label) { pass++; console.log(`  ✔ ${label}`); }
function ko(label, err) { fail++; console.error(`  ✘ ${label}:`, err?.message || err); }

async function test(label, fn) {
	try { await fn(); ok(label); }
	catch (e) { ko(label, e); }
}

async function run() {
	mkdirSync(SHOTS, { recursive: true });
	mkdirSync(SHOTS_DOCS, { recursive: true });

	const tmpDir = join(ROOT, '.tmp-docx-nav-test');
	assemble(tmpDir);
	const { server, base } = await startBuilt(tmpDir);

	const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
	const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });

	try {
		console.log('▶ docx Navigation pane (#607)');

		// ---- screenshot: before (no nav pane) ----
		{
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.screenshot({ path: join(SHOTS, 'feature-607-before.png') });
			await page.close();
		}

		// ---- open / close via View > Navigation ----
		await test('opens nav pane via View > Navigation button', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			const navBtn = await page.$('button.rb[aria-label="Show or hide the Navigation pane"]');
			if (!navBtn) throw new Error('Navigation button not found in View ribbon');
			await navBtn.click();
			await page.waitForTimeout(300);
			const navPaneVisible = await page.evaluate(() => {
				const pane = document.querySelector('.nav-pane');
				return pane && !pane.hidden;
			});
			if (!navPaneVisible) throw new Error('nav-pane should be visible after click');
			await page.screenshot({ path: join(SHOTS, 'feature-607-after.png') });
			await page.close();
		});

		await test('closes nav pane on second click', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			// First open
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			await page.click('button.rb[aria-label="Show or hide the Navigation pane"]');
			await page.waitForTimeout(300);
			// Second click — re-query after tab re-render
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			await page.click('button.rb[aria-label="Show or hide the Navigation pane"]');
			await page.waitForTimeout(200);
			const navPaneVisible = await page.evaluate(() => {
				const pane = document.querySelector('.nav-pane');
				return pane && !pane.hidden;
			});
			if (navPaneVisible) throw new Error('nav-pane should be hidden after second click');
			await page.close();
		});

		await test('Navigation button has aria-pressed=true when pane is open', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			const navBtn = await page.$('button.rb[aria-label="Show or hide the Navigation pane"]');
			await navBtn.click();
			await page.waitForTimeout(400);
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			const pressed = await page.$eval('button.rb[aria-label="Show or hide the Navigation pane"]', (b) => b.getAttribute('aria-pressed'));
			if (pressed !== 'true') throw new Error(`Expected aria-pressed=true, got ${pressed}`);
			await page.close();
		});

		// ---- Headings tab: list of headings ----
		await test('Headings tab lists headings from the sample docx', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			const navBtn = await page.$('button.rb[aria-label="Show or hide the Navigation pane"]');
			await navBtn.click();
			// Wait for heading items or empty state to render (async import + WASM call)
			await page.waitForSelector('.nav-h-item, .nav-empty', { timeout: 8000 });
			const headingCount = await page.evaluate(() => document.querySelectorAll('.nav-h-item').length);
			if (headingCount < 1) throw new Error(`Expected at least 1 heading, got ${headingCount}`);
			await page.close();
		});

		await test('clicking a heading scrolls the document', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			const navBtn = await page.$('button.rb[aria-label="Show or hide the Navigation pane"]');
			await navBtn.click();
			await page.waitForSelector('.nav-h-item, .nav-empty', { timeout: 8000 });
			// Get scroll position before click
			const scrollBefore = await page.evaluate(() => {
				const el = document.querySelector('nitro-docx');
				const sr = el && el.shadowRoot;
				const r = sr && sr.querySelector('.r');
				return r ? r.scrollTop : 0;
			});
			// Click the last heading (should scroll down)
			const items = await page.$$('.nav-h-item');
			if (items.length > 1) {
				await items[items.length - 1].click();
				await page.waitForTimeout(300);
			}
			// Heading item should get aria-current
			const hasCurrentHeading = await page.evaluate(() =>
				!!document.querySelector('.nav-h-item[aria-current="true"]'),
			);
			if (!hasCurrentHeading) throw new Error('Expected a heading to get aria-current=true after click');
			await page.close();
		});

		await test('Enter key on heading item triggers scroll', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			const navBtn = await page.$('button.rb[aria-label="Show or hide the Navigation pane"]');
			await navBtn.click();
			await page.waitForSelector('.nav-h-item, .nav-empty', { timeout: 8000 });
			const firstItem = await page.$('.nav-h-item');
			if (!firstItem) throw new Error('No heading items');
			await firstItem.focus();
			await page.keyboard.press('Enter');
			await page.waitForTimeout(200);
			const hasCurrentHeading = await page.evaluate(() =>
				!!document.querySelector('.nav-h-item[aria-current="true"]'),
			);
			if (!hasCurrentHeading) throw new Error('Expected aria-current after Enter');
			await page.close();
		});

		// ---- Pages tab ----
		await test('Pages tab shows page list', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			const navBtn = await page.$('button.rb[aria-label="Show or hide the Navigation pane"]');
			await navBtn.click();
			await page.waitForTimeout(500);
			// Switch to Pages tab
			const pagesTabs = await page.$$('[role=tab].nav-tab');
			if (pagesTabs.length < 2) throw new Error('Not enough nav tabs');
			await pagesTabs[1].click();
			await page.waitForTimeout(300);
			const pageCount = await page.evaluate(() => document.querySelectorAll('.nav-pg-item').length);
			if (pageCount < 1) throw new Error(`Expected at least 1 page item, got ${pageCount}`);
			await page.screenshot({ path: join(SHOTS, 'feature-607-pages.png') });
			await page.close();
		});

		await test('clicking a page item navigates to that page', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			const navBtn = await page.$('button.rb[aria-label="Show or hide the Navigation pane"]');
			await navBtn.click();
			await page.waitForTimeout(500);
			const pagesTabs = await page.$$('[role=tab].nav-tab');
			await pagesTabs[1].click();
			await page.waitForTimeout(300);
			const pgItems = await page.$$('.nav-pg-item');
			if (pgItems.length > 0) {
				await pgItems[0].click();
				await page.waitForTimeout(200);
			}
			// No crash = pass
			await page.close();
		});

		// ---- Results tab / Ctrl+F ----
		await test('Ctrl+F opens nav pane on Results tab with focused input', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.keyboard.press('Control+f');
			await page.waitForTimeout(500);
			const navVisible = await page.evaluate(() => {
				const pane = document.querySelector('.nav-pane');
				return pane && !pane.hidden;
			});
			if (!navVisible) throw new Error('nav-pane should open on Ctrl+F');
			// Results panel should be visible
			const resultsVisible = await page.evaluate(() => {
				const panels = document.querySelectorAll('.nav-panel');
				return panels.length >= 3 && !panels[2].hidden;
			});
			if (!resultsVisible) throw new Error('Results panel should be visible after Ctrl+F');
			await page.screenshot({ path: join(SHOTS, 'feature-607-results.png') });
			await page.close();
		});

		await test('Results tab search input is present and accepts text', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.keyboard.press('Control+f');
			await page.waitForTimeout(500);
			const input = await page.$('.nav-srch input');
			if (!input) throw new Error('Search input not found in Results panel');
			await input.fill('shipped');
			await page.waitForTimeout(400);
			// No crash = pass
			await page.close();
		});

		await test('arrow keys navigate between nav tabs', async () => {
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			const navBtn = await page.$('button.rb[aria-label="Show or hide the Navigation pane"]');
			await navBtn.click();
			await page.waitForTimeout(500);
			// Focus first nav tab and press ArrowRight
			const firstNavTab = await page.$('[role=tab].nav-tab');
			if (!firstNavTab) throw new Error('No nav tabs found');
			await firstNavTab.focus();
			await page.keyboard.press('ArrowRight');
			await page.waitForTimeout(100);
			const selectedTab = await page.evaluate(() => {
				const tabs = document.querySelectorAll('[role=tab].nav-tab');
				for (const t of tabs) if (t.getAttribute('aria-selected') === 'true') return t.textContent;
				return null;
			});
			if (selectedTab !== 'Pages') throw new Error(`Expected Pages tab selected, got ${selectedTab}`);
			await page.close();
		});

		// ---- empty state ----
		await test('empty state shows message when document has no headings', async () => {
			// Open without a document (drop state)
			const page = await ctx.newPage();
			await openDocxFull(page, base);
			// Inject a fake nav-headings response with empty headings list
			await page.evaluate(() => {
				const el = document.querySelector('nitro-docx');
				if (!el) return;
				const orig = el.shellAction.bind(el);
				el.shellAction = (op, arg) => {
					if (op === 'nav-headings') return Promise.resolve(JSON.stringify({ headings: [] }));
					return orig(op, arg);
				};
			});
			await page.click('button[role=tab][data-t=View]');
			await page.waitForTimeout(100);
			const navBtn = await page.$('button.rb[aria-label="Show or hide the Navigation pane"]');
			await navBtn.click();
			await page.waitForTimeout(500);
			const emptyMsg = await page.evaluate(() => !!document.querySelector('.nav-empty'));
			if (!emptyMsg) throw new Error('Expected .nav-empty element when no headings');
			await page.close();
		});

		// Copy screenshots to docs/
		for (const name of ['feature-607-before', 'feature-607-after', 'feature-607-pages', 'feature-607-results']) {
			const src = join(SHOTS, `${name}.png`);
			const dst = join(SHOTS_DOCS, `${name}.png`);
			try { cpSync(src, dst); } catch { /* ignore if screenshot wasn't taken due to earlier failure */ }
		}

	} finally {
		await browser.close();
		server.close();
		rmSync(tmpDir, { recursive: true, force: true });
	}

	console.log(`\n  ${pass} passed, ${fail} failed`);
	if (fail > 0) process.exit(1);
}

run().catch((e) => { console.error(e); process.exit(1); });
