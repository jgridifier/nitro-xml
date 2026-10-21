// SPDX-License-Identifier: MIT
/**
 * Interaction audit (slice 1: measure, don't fix).
 *
 *   node tests/interaction/audit.mjs [--targets local,live] [--browsers chromium,webkit]
 *        [--only <substring>] [--formats docx,xlsx,pptx] [--skip-latency] [--concurrency 3]
 *
 * Drives real mouse/keyboard/touch input in headless Chromium and WebKit against the live GitHub
 * Pages demos and a local build (demo/ + packages/nitro-xml/{loader,pkg}; run scripts/build-wasm.sh
 * first, and `bun install` in parity/visual for playwright + pngjs). Writes tests/interaction/out/results.json and failure screenshots to out/shots/.
 * Exit code is always 0: this is an audit, not a gate.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { startServer } from './server.mjs';
import { openDemo, inEl, pct, TAGS } from './lib.mjs';
import { SCENARIOS } from './scenarios.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = join(HERE, 'out');
const SHOTS = join(OUT, 'shots');
const LIVE = 'https://jgridifier.github.io/nitro-xml/demo';

const arg = (k, d) => { const i = process.argv.indexOf('--' + k); return i < 0 ? d : (process.argv[i + 1] ?? true); };
const targets = String(arg('targets', 'local,live')).split(',');
const browsers = String(arg('browsers', 'chromium,webkit')).split(',');
const only = arg('only', '');
const formats = String(arg('formats', 'docx,xlsx,pptx')).split(',');
const concurrency = +arg('concurrency', 2);
const skipLatency = process.argv.includes('--skip-latency');
const latencyOnly = process.argv.includes('--latency-only');
const reps = +arg('reps', 1);

const { chromium, webkit } = await import(pathToFileURL(join(HERE, '../../parity/visual/node_modules/playwright/index.mjs')).href);
const ENGINES = { chromium, webkit };
mkdirSync(SHOTS, { recursive: true });

const withTimeout = (p, ms, what) => Promise.race([p, new Promise((_, rej) => setTimeout(() => rej(new Error(`timeout ${ms}ms: ${what}`)), ms))]);

async function runScenario(browser, base, target, browserName, sc, fmt) {
	const touch = !!sc.touch;
	const ctxOpts = { viewport: { width: 1280, height: 800 }, hasTouch: touch };
	const context = await browser.newContext(ctxOpts);
	if (browserName === 'chromium' && sc.clipboard) await context.grantPermissions(['clipboard-read', 'clipboard-write']).catch(() => {});
	const page = await context.newPage();
	// Humanise: a person needs >100 ms between releasing the mouse and the next key. The event-ordering
	// scenario uses the raw functions saved on ctx.raw.
	const raw = { click: page.mouse.click.bind(page.mouse), up: page.mouse.up.bind(page.mouse), dblclick: page.mouse.dblclick.bind(page.mouse), tap: page.touchscreen.tap.bind(page.touchscreen) };
	for (const [obj, k] of [[page.mouse, 'click'], [page.mouse, 'up'], [page.mouse, 'dblclick'], [page.touchscreen, 'tap']]) {
		const orig = obj[k].bind(obj);
		obj[k] = async (...a) => { const r = await orig(...a); await page.waitForTimeout(150); return r; };
	}
	const errors = [];
	page.on('pageerror', (e) => errors.push(String(e).slice(0, 160)));
	let cdp = null;
	const rec = { id: sc.id, fmt, browser: browserName, target, sev: sc.sev, title: sc.title, status: 'error', detail: '', shot: null };
	const ctx = {
		page, fmt, browser: browserName, target, raw,
		cdp: async () => (cdp ||= await context.newCDPSession(page)),
		shot: async (name) => {
			const f = `${target}-${browserName}-${name}.png`;
			await page.screenshot({ path: join(SHOTS, f) }).catch(() => {});
			rec.shot = f;
		},
	};
	try {
		await withTimeout(openDemo(page, base, fmt), 60_000, 'open');
		const r = await withTimeout(sc.fn(ctx), 60_000, sc.id);
		Object.assign(rec, r);
		if (r.status === 'fail' && !rec.shot) await ctx.shot(sc.id.replace(/\W/g, '-'));
	} catch (e) {
		rec.status = 'error';
		rec.detail = String(e && e.message || e).slice(0, 300);
		await ctx.shot('error-' + sc.id.replace(/\W/g, '-'));
	}
	if (errors.length) rec.pageErrors = errors.slice(0, 3);
	await context.close().catch(() => {});
	return rec;
}

// ---------- latency: input event -> first DOM mutation -> next frame ----------
async function latencyRun(browser, base, browserName, target, fmt, kind) {
	const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
	const page = await context.newPage();
	try {
		await openDemo(page, base, fmt);
		const hook = () => inEl(page, fmt, (el, sr) => {
			const w = window;
			w.__lat = { s: [], t0: 0, armed: false };
			const root = sr.querySelector('.v');
			const arm = () => { if (w.__lat.armed) return; w.__lat.armed = true; };
			addEventListener('keydown', (e) => { w.__lat.t0 = e.timeStamp; w.__lat.armed = true; w.__lat.dom = null; }, true);
			addEventListener('pointerdown', (e) => { w.__lat.t0 = e.timeStamp; w.__lat.armed = true; w.__lat.dom = null; }, true);
			new MutationObserver(() => {
				const L = w.__lat;
				if (!L.armed || L.dom != null) return;
				L.dom = performance.now() - L.t0;
				requestAnimationFrame(() => requestAnimationFrame(() => { L.s.push({ dom: L.dom, frame: performance.now() - L.t0 }); L.armed = false; L.dom = null; }));
			}).observe(root, { subtree: true, childList: true, attributes: true, characterData: true });
		});
		await hook();
		const n = fmt === 'docx' ? 1 : 14;
		if (fmt === 'xlsx') {
			const g = await inEl(page, 'xlsx', (el, sr) => { const m = (s, a, b) => [...sr.querySelectorAll(s)].map((e) => { const r = e.getBoundingClientRect(); return [r[a], r[b]]; }); return { cols: m('.xg-hdr-col .xg-hd', 'x', 'width'), rows: m('.xg-hdr-row .xg-hd', 'y', 'height') }; });
			if (kind === 'click') {
				for (let i = 0; i < n; i++) {
					await page.mouse.click(g.cols[1 + (i % 6)][0] + 10, g.rows[2 + (i % 9)][0] + 6);
					await page.waitForTimeout(220);
				}
			} else {
				await page.mouse.click(g.cols[8][0] + 10, g.rows[3][0] + 6);
				await page.waitForTimeout(300);
				for (let i = 0; i < n; i++) { await page.keyboard.press(kind === 'nav' ? (i % 2 ? 'ArrowUp' : 'ArrowDown') : 'x'); await page.waitForTimeout(160); }
			}
		} else if (fmt === 'pptx') {
			const r = await inEl(page, 'pptx', (el, sr) => { const b = sr.querySelector('.pptxv-text').getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 2 }; });
			if (kind === 'click') for (let i = 0; i < n; i++) { await page.mouse.click(r.x + (i % 5) * 7, r.y); await page.waitForTimeout(220); }
			else { await page.mouse.dblclick(r.x, r.y); await page.waitForTimeout(300); for (let i = 0; i < n; i++) { await page.keyboard.press(kind === 'nav' ? 'PageDown' : 'x'); await page.waitForTimeout(200); } }
		} else {
			const r = await inEl(page, 'docx', (el, sr) => { const b = sr.querySelectorAll('.dve-print-line')[3].getBoundingClientRect(); return { x: b.x + 100, y: b.y + b.height / 2 }; });
			await page.mouse.click(r.x, r.y); await page.waitForTimeout(300);
			await page.keyboard.press(kind === 'nav' ? 'ArrowRight' : 'x'); await page.waitForTimeout(1200);
		}
		const samples = await page.evaluate(() => window.__lat?.s || []);
		return samples;
	} finally {
		await context.close().catch(() => {});
	}
}

async function latency(browser, base, browserName, target) {
	const rows = [];
	const plan = [['xlsx', 'type'], ['xlsx', 'nav'], ['xlsx', 'click'], ['pptx', 'type'], ['pptx', 'click'], ['docx', 'type'], ['docx', 'nav']];
	for (const [fmt, kind] of plan) {
		if (!formats.includes(fmt)) continue;
		let samples = [];
		const nrun = (fmt === 'docx' ? 5 : 1) * reps; // docx stops painting after the first keystroke, so use fresh pages
		for (let k = 0; k < nrun; k++) samples = samples.concat(await latencyRun(browser, base, browserName, target, fmt, kind).catch((e) => []));
		const dom = samples.map((s) => s.dom), frame = samples.map((s) => s.frame);
		rows.push({ target, browser: browserName, fmt, kind, n: samples.length, domP50: pct(dom, 50), domP95: pct(dom, 95), frameP50: pct(frame, 50), frameP95: pct(frame, 95) });
		console.log(`latency ${target}/${browserName} ${fmt}.${kind}: n=${samples.length} dom p50/p95=${pct(dom, 50)}/${pct(dom, 95)}ms frame p50/p95=${pct(frame, 50)}/${pct(frame, 95)}ms`);
	}
	return rows;
}

async function pool(items, n, fn) {
	const out = []; let i = 0;
	await Promise.all(Array.from({ length: n }, async () => { while (i < items.length) { const k = i++; out[k] = await fn(items[k]); } }));
	return out;
}

const server = targets.includes('local') ? await startServer() : null;
const results = [], lat = [];
for (const target of targets) {
	const base = target === 'local' ? server.url : LIVE;
	for (const browserName of browsers) {
		const browser = await ENGINES[browserName].launch();
		const jobs = [];
		for (const sc of SCENARIOS) {
			if (latencyOnly || (only && !sc.id.includes(only))) continue;
			for (const fmt of sc.fmts) if (formats.includes(fmt)) jobs.push([sc, fmt]);
		}
		const t0 = Date.now();
		// The system clipboard is shared by every context in a browser: clipboard scenarios run one at a time.
		const par = jobs.filter(([sc]) => !sc.clipboard), ser = jobs.filter(([sc]) => sc.clipboard);
		const run = ([sc, fmt]) => runScenario(browser, base, target, browserName, sc, fmt);
		const rs = [...(await pool(par, concurrency, run)), ...(await pool(ser, 1, run))];
		for (const r of rs) { results.push(r); console.log(`${r.status.toUpperCase().padEnd(5)} ${target}/${browserName} ${r.id}${r.fmt !== r.id.split('.')[0] ? ' [' + r.fmt + ']' : ''}: ${r.detail}`.slice(0, 300)); }
		console.log(`-- ${target}/${browserName}: ${rs.length} scenarios in ${((Date.now() - t0) / 1000).toFixed(0)}s`);
		if (!skipLatency && (latencyOnly || !only)) lat.push(...(await latency(browser, base, browserName, target)));
		await browser.close();
	}
}
server?.close();
writeFileSync(join(OUT, latencyOnly ? 'latency.json' : 'results.json'), JSON.stringify({ when: new Date().toISOString(), results, latency: lat }, null, 1));
const tally = {};
for (const r of results) { const k = `${r.target}/${r.browser}`; (tally[k] ||= { pass: 0, fail: 0, na: 0, error: 0 })[r.status]++; }
console.log(JSON.stringify(tally));
