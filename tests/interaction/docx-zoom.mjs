// SPDX-License-Identifier: MIT
/**
 * Docx zoom on the built Pages demos (docx-full, docx-embedded), Chromium and WebKit.
 * Rides the selection-parity job. No new workflow.
 *
 * Catches: Word zoom updating the percent readout while the page stays at 100%,
 * or the caret landing on a different character after the page scale changes.
 */
import { createServer } from 'node:http';
import { cpSync, createReadStream, mkdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const require = createRequire(join(dirname(fileURLToPath(import.meta.url)), '../../parity/visual/package.json'));
const { chromium, webkit } = require('playwright');

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, '../..');
const MIME = {
	'.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css',
	'.wasm': 'application/wasm', '.json': 'application/json', '.svg': 'image/svg+xml',
	'.png': 'image/png', '.docx': 'application/octet-stream',
};
const BROWSERS = (process.env.DOCX_ZOOM_BROWSERS || 'chromium,webkit').split(',').map((s) => s.trim()).filter(Boolean);
const SHOTS = process.env.DOCX_ZOOM_SHOTS || '/opt/cursor/artifacts/docx-zoom';
const ENGINES = { chromium, webkit };

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
		throw new Error('docx-zoom: missing wasm pkg; run scripts/build-wasm.sh');
	}
	cpSync(pkg, join(dir, 'nitro-xml/pkg'), { recursive: true });
}

function startBuilt(demoDir) {
	return new Promise((resolve) => {
		const s = createServer((req, rsp) => {
			const pathname = decodeURIComponent(new URL(req.url, 'http://x').pathname);
			const rel = pathname.replace(/^\/demo\/?/, '');
			const f = normalize(join(demoDir, rel));
			if (!f.startsWith(demoDir)) {
				rsp.writeHead(404).end('not found');
				return;
			}
			try {
				if (!statSync(f).isFile()) throw new Error('404');
				rsp.writeHead(200, { 'content-type': MIME[extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
				createReadStream(f).pipe(rsp);
			} catch {
				rsp.writeHead(404).end('not found');
			}
		});
		s.listen(0, '127.0.0.1', () => resolve({ url: `http://127.0.0.1:${s.address().port}/demo`, close: () => s.close() }));
	});
}

async function probe(page) {
	return page.evaluate(() => {
		const el = document.querySelector('nitro-docx');
		const sr = el && el.shadowRoot;
		const pageEl = sr && sr.querySelector('.dve-print-page');
		const pages = sr && sr.querySelector('.dve-print-pages');
		if (!pageEl || !pages) return null;
		const box = pageEl.getBoundingClientRect();
		const bar = sr.querySelector('.nxc');
		let caret = null;
		if (bar && bar.style.display === 'block') {
			const b = bar.getBoundingClientRect();
			if (b.height > 0) caret = { x: b.x + b.width / 2, top: b.top, h: b.height };
		}
		let hit = null;
		if (caret) {
			let best = Infinity;
			for (const s of sr.querySelectorAll('.dve-print-line > span')) {
				const node = s.firstChild;
				if (!node || node.nodeType !== 3 || !node.data) continue;
				const bid = s.closest('[data-bid]') && s.closest('[data-bid]').dataset.bid;
				const cps = Array.from(node.data);
				let u = 0;
				for (let i = 0; i <= cps.length; i++) {
					const r = document.createRange();
					let x;
					let top;
					let h;
					if (i < cps.length) {
						r.setStart(node, u);
						r.setEnd(node, u + cps[i].length);
						const b = r.getBoundingClientRect();
						x = b.left;
						top = b.top;
						h = b.height;
						u += cps[i].length;
					} else {
						const prev = cps[cps.length - 1] || '';
						r.setStart(node, Math.max(0, u - prev.length));
						r.setEnd(node, u);
						const b = r.getBoundingClientRect();
						x = b.right;
						top = b.top;
						h = b.height;
					}
					if (!h) continue;
					const d = Math.hypot(x - caret.x, top + h / 2 - (caret.top + caret.h / 2));
					if (d < best) {
						best = d;
						hit = { bid, index: i, delta: d };
					}
				}
			}
		}
		const status = document.querySelector('.sb .zl');
		return {
			width: box.width,
			height: box.height,
			zoom: getComputedStyle(pages).getPropertyValue('--dve-zoom').trim(),
			readout: ((sr.querySelector('.z') || {}).textContent || '').trim(),
			status: status ? status.textContent.trim() : '',
			hit,
		};
	});
}

async function waitProbe(page, pred, label) {
	const deadline = Date.now() + 12_000;
	let last = null;
	while (Date.now() < deadline) {
		last = await probe(page);
		if (last && pred(last)) return last;
		await page.waitForTimeout(150);
	}
	throw new Error(`${label}: ${JSON.stringify(last)}`);
}

async function findButton(page, name) {
	const loc = page.getByRole('button', { name, exact: true });
	const n = await loc.count();
	for (let i = 0; i < n; i++) {
		const b = loc.nth(i);
		if ((await b.isVisible()) && (await b.isEnabled())) return b;
	}
	return null;
}

async function clickZoom(page, name) {
	const deadline = Date.now() + 8_000;
	let triedMore = false;
	while (Date.now() < deadline) {
		const btn = await findButton(page, name);
		if (btn) {
			await btn.click();
			return;
		}
		if (!triedMore) {
			triedMore = true;
			const more = await findButton(page, 'More');
			if (more) {
				await more.click();
				await page.waitForTimeout(150);
				continue;
			}
		}
		await page.waitForTimeout(150);
	}
	throw new Error(`no enabled button: ${name}`);
}

async function clickText(page) {
	const point = await page.evaluate(() => {
		const sr = document.querySelector('nitro-docx').shadowRoot;
		const lines = [...sr.querySelectorAll('.dve-print-line')];
		const line = lines.find((n) => (n.textContent || '').includes('This sample')) || lines[0];
		if (!line) return null;
		const r = line.getBoundingClientRect();
		return { x: r.x + Math.min(80, r.width * 0.35), y: r.y + r.height / 2 };
	});
	if (!point) throw new Error('no docx text to click');
	await page.mouse.click(point.x, point.y);
}

function sameHit(a, b) {
	return a && b && a.bid === b.bid && a.index === b.index && b.delta <= 2;
}

async function shot(page, file) {
	mkdirSync(dirname(file), { recursive: true });
	await page.screenshot({ path: file, animations: 'disabled' });
}

/**
 * @returns {Promise<string|null>}
 */
async function runCase(page, base, mode, shotDir) {
	const id = `docx-${mode}`;
	await page.goto(`${base}/docx-${mode}.html`, { waitUntil: 'domcontentloaded', timeout: 60_000 });
	await page.waitForFunction(() => {
		const el = document.querySelector('nitro-docx');
		const sr = el && el.shadowRoot;
		return !!(sr && el.document && sr.querySelector('.dve-print-line'));
	}, null, { timeout: 45_000 });
	await page.waitForTimeout(400);
	const before = await waitProbe(page, (m) => m.width > 100, 'page');
	await shot(page, join(shotDir, `${id}-before.png`));
	await clickText(page);
	const placed = await waitProbe(page, (m) => m.hit && m.hit.delta <= 2, 'caret');
	const anchor = placed.hit;
	const zoomOnce = async (label) => {
		const from = (await probe(page)).width;
		await clickZoom(page, label);
		// Status-bar clicks move focus out of the page; the caret hides until the surface is focused again.
		await page.locator('nitro-docx').locator('.v').focus();
		return waitProbe(page, (m) => (label === 'Zoom in' ? m.width > from * 1.05 : m.width < from * 0.96) && sameHit(anchor, m.hit), label);
	};
	const at110 = await zoomOnce('Zoom in');
	const at120 = await zoomOnce('Zoom in');
	const ratio = at120.width / before.width;
	if (ratio < 1.15 || ratio > 1.28) throw new Error(`${id} scale ${ratio.toFixed(3)} after two Zoom in clicks (width ${before.width} -> ${at120.width})`);
	if (!at120.zoom || Number(at120.zoom) < 1.15) throw new Error(`${id} --dve-zoom stayed ${JSON.stringify(at120.zoom)}`);
	if (mode === 'full') {
		if (!/120%/.test(at120.status) && !/120%/.test(at120.readout)) {
			throw new Error(`${id} readout ${at120.status || at120.readout}`);
		}
		const slider = page.locator('input[aria-label="Zoom"]');
		await slider.focus();
		// Four steps of 10 from the 120% the buttons just set. WebKit ignores a synthetic input event on range.
		for (let i = 0; i < 4; i++) await page.keyboard.press('ArrowRight');
		await page.locator('nitro-docx').locator('.v').focus();
		await waitProbe(page, (m) => m.width > before.width * 1.5 && m.width < before.width * 1.75 && sameHit(anchor, m.hit), 'slider 160%');
		await page.locator('nitro-docx').locator('.v').focus();
		await page.keyboard.press('Control+Minus');
		await waitProbe(page, (m) => m.width < before.width * 1.55 && m.width > before.width * 1.4 && sameHit(anchor, m.hit), 'ctrl-minus');
		await page.keyboard.press('Control+Equal');
		await waitProbe(page, (m) => m.width > before.width * 1.5 && sameHit(anchor, m.hit), 'ctrl-equal');
	} else {
		if (!/120%/.test(at120.readout)) throw new Error(`${id} readout ${at120.readout}`);
		await zoomOnce('Zoom out');
		await zoomOnce('Zoom in');
	}
	await shot(page, join(shotDir, `${id}-after.png`));
	return { id, before: before.width, after: (await probe(page)).width, anchor, at110: at110.width };
}

async function main() {
	const demoDir = join(HERE, 'out/pages-demo');
	assemble(demoDir);
	const server = await startBuilt(demoDir);
	const lines = [];
	let failed = 0;
	try {
		for (const name of BROWSERS) {
			const engine = ENGINES[name];
			if (!engine) throw new Error(`unknown browser ${name}`);
			const browser = await engine.launch();
			const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
			for (const mode of ['full', 'embedded']) {
				const shotDir = join(SHOTS, name);
				try {
					const r = await runCase(page, server.url, mode, shotDir);
					lines.push(`${name} ${r.id}: ok width ${r.before.toFixed(1)} -> ${r.after.toFixed(1)} caret ${r.anchor.bid}@${r.anchor.index}`);
					console.log(lines[lines.length - 1]);
				} catch (e) {
					failed++;
					const msg = String(e && e.message ? e.message : e).split('\n')[0];
					lines.push(`${name} docx-${mode}: FAIL ${msg}`);
					console.error(lines[lines.length - 1]);
					try { await shot(page, join(shotDir, `docx-${mode}-fail.png`)); } catch { /* page may be closed */ }
				}
			}
			await browser.close();
		}
	} finally {
		server.close();
	}
	const summary = [`docx zoom: ${lines.length - failed}/${lines.length} ok`, ...lines].join('\n') + '\n';
	mkdirSync(join(HERE, 'out'), { recursive: true });
	writeFileSync(join(HERE, 'out/docx-zoom-summary.txt'), summary);
	console.log(summary);
	if (failed) process.exit(1);
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
