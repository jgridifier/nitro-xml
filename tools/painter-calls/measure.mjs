// SPDX-License-Identifier: MIT
// Counts upstream's DOM / canvas-2D calls per animation frame, per docs/painters.md row.
//
//   node tools/painter-calls/measure.mjs [--upstream DIR] [--out FILE] [xlsx|docx|pptx ...]
//
// DIR is a scratch copy of upstream at the pinned commit with `bun install` done (default
// /workspace/ooxml-measure). Vite and Playwright are loaded from DIR, so this tool has no
// dependencies of its own. It serves harness/ with Vite (upstream TypeScript sources, unbundled,
// so stack frames name upstream files), injects instrument.js, opens one fixture per format and
// records: first paint after open; a steady scroll (xlsx, docx Draft and Print Layout); a slide
// change (pptx). Test-only: nothing here ships.
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../..');
const args = process.argv.slice(2);
const opt = (name, fallback) => {
	const i = args.indexOf(name);
	if (i < 0) return fallback;
	const [value] = args.splice(i, 2).slice(1);
	return value;
};
const UP = resolve(opt('--upstream', '/workspace/ooxml-measure'));
const OUT = resolve(opt('--out', join(here, 'results.json')));
const CHROME = opt('--chrome', '/usr/bin/google-chrome');
const formats = args.length ? args : ['xlsx', 'docx', 'pptx'];

// M-tier stand-ins until the #4 manifest assigns tiers (see README.md).
const FIXTURES = {
	xlsx: 'fixtures/real/xlsx/poi-StructuredRefs-lots-with-lookups.xlsx',
	docx: 'fixtures/real/docx/poi-drawing.docx',
	pptx: 'fixtures/real/pptx/poi-60810.pptx',
};
// Generic helpers whose calls are charged to the calling painter instead.
const DEFER = [
	'src/ui/src/xlsx/grid/dom.ts',
	'src/ui/src/xlsx/grid/measure.ts',
	'src/ui/src/pptx/dom/dom.ts',
	'src/ui/src/docx/canvas-measurer.ts',
];
const QUIET_FRAMES = 60; // a scenario ends after this many frames without any recorded call
const MAX_WAIT_MS = 30_000;
const SCROLL_FRAMES = 60;
const WARMUP_FRAMES = 5; // scroll frames dropped before the steady window

if (!existsSync(join(UP, 'src/ui/src/xlsx/index.ts')))
	throw new Error(`${UP} is not an upstream checkout (no src/ui/src/xlsx/index.ts)`);
const vite = await import(pathToFileURL(join(UP, 'viewers/xlsx/node_modules/vite/dist/node/index.js')).href);
const { chromium } = createRequire(join(UP, 'e2e/xlsx/package.json'))('@playwright/test');

// `ooxml-core[/area]` and `ooxml-ui[/entry]` -> upstream TypeScript sources, as upstream's own
// OOXML_CORE_SRC aliases do.
const PACKAGES = { 'ooxml-core': 'src/core', 'ooxml-ui': 'src/ui/src' };
const coreSource = {
	name: 'ooxml-source',
	enforce: 'pre',
	resolveId(id) {
		const m = /^(ooxml-core|ooxml-ui)(?:\/(.+))?$/.exec(id);
		if (!m) return null;
		const dir = join(UP, PACKAGES[m[1]]);
		const sub = m[2];
		const candidates = sub
			? [join(dir, sub, 'index.ts'), join(dir, `${sub}.ts`)]
			: [join(dir, 'index.ts')];
		return candidates.find((file) => existsSync(file)) ?? null;
	},
};

const server = await vite.createServer({
	configFile: false,
	root: join(here, 'harness'),
	logLevel: 'warn',
	plugins: [coreSource],
	resolve: { alias: [{ find: /^@up\//, replacement: `${UP}/` }] },
	server: { host: '127.0.0.1', port: 0, fs: { allow: [UP, here] } },
	optimizeDeps: { entries: [join(here, 'harness/index.html')] },
});
await server.listen();
const base = server.resolvedUrls.local[0];

const rows = JSON.parse(readFileSync(join(here, 'rows.json'), 'utf8')).files;
const init = readFileSync(join(here, 'instrument.js'), 'utf8')
	.replace('__ROWS__', JSON.stringify(rows))
	.replace('__DEFER__', JSON.stringify(DEFER));

const browser = await chromium.launch({ headless: true, executablePath: CHROME });
const results = { upstream: UP, fixtures: {}, scenarios: [] };

const median = (values) => {
	if (!values.length) return 0;
	const s = [...values].sort((a, b) => a - b);
	const mid = s.length >> 1;
	return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
};

/** Collapses raw counts into per-row stats for frames in [from, to]. */
function summarise(raw, from, to, includeZeros) {
	const perRow = new Map(); // row -> Map(frame -> n)
	const categories = new Map(); // row -> {category: n}
	for (const { frame, row, category, n } of raw.rows) {
		if (frame < from || frame > to) continue;
		const frames = perRow.get(row) ?? new Map();
		frames.set(frame, (frames.get(frame) ?? 0) + n);
		perRow.set(row, frames);
		const c = categories.get(row) ?? {};
		c[category] = (c[category] ?? 0) + n;
		categories.set(row, c);
	}
	const out = {};
	for (const [row, frames] of perRow) {
		const active = [...frames.values()];
		const all = includeZeros
			? Array.from({ length: to - from + 1 }, (_, i) => frames.get(from + i) ?? 0)
			: active;
		out[row] = {
			total: active.reduce((a, b) => a + b, 0),
			activeFrames: active.length,
			windowFrames: to - from + 1,
			median: median(all),
			max: Math.max(...active),
			categories: categories.get(row),
		};
	}
	return out;
}

async function waitQuiet(page, startFrame) {
	const t0 = Date.now();
	while (Date.now() - t0 < MAX_WAIT_MS) {
		const quiet = await page.evaluate((s) => window.__painterCalls.quietSince(s), startFrame);
		if (quiet >= QUIET_FRAMES) return true;
		await page.waitForTimeout(100);
	}
	return false;
}

/** Runs `action` under the recorder and summarises the frames it produced. */
async function scenario(page, format, name, action, { includeZeros = false, window } = {}) {
	const start = await page.evaluate(() => window.__painterCalls.start());
	const extra = (await action()) ?? {};
	const settled = await waitQuiet(page, start);
	const raw = await page.evaluate(() => window.__painterCalls.stop());
	const from = window ? extra.from + window[0] : start;
	const to = window ? extra.to : raw.end;
	const entry = {
		format,
		name,
		settled,
		frames: [from, to],
		rows: summarise(raw, from, to, includeZeros),
		otherFiles: raw.other,
	};
	results.scenarios.push(entry);
	const brief = Object.entries(entry.rows)
		.sort((a, b) => b[1].total - a[1].total)
		.map(([r, s]) => `${r}=${s.median}/${s.total}`)
		.join(' ');
	console.log(`${format} ${name}${settled ? '' : ' (not settled)'}: ${brief}`);
	return entry;
}

/** Scrolls the largest scrollable box (deep, through shadow roots) by `dy` px per frame. */
const scrollInPage = async ([frames, dy]) => {
	const all = [];
	const walk = (root) => {
		for (const el of root.querySelectorAll('*')) {
			all.push(el);
			if (el.shadowRoot) walk(el.shadowRoot);
		}
	};
	walk(document);
	const scrollers = all.filter((el) => {
		const s = getComputedStyle(el);
		return /(auto|scroll)/.test(s.overflowY) && el.scrollHeight > el.clientHeight + 100;
	});
	scrollers.sort((a, b) => b.clientWidth * b.clientHeight - a.clientWidth * a.clientHeight);
	const el = scrollers[0];
	if (!el) throw new Error('no scrollable element');
	const nextFrame = () => new Promise((r) => requestAnimationFrame(r));
	await nextFrame();
	const from = window.__painterCalls.frame();
	for (let i = 0; i < frames; i++) {
		el.scrollTop += dy;
		await nextFrame();
	}
	return { from, to: window.__painterCalls.frame(), scroller: el.className, top: el.scrollTop };
};

async function openPage() {
	const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
	page.on('pageerror', (e) => console.warn(`  pageerror: ${e.message}`));
	page.on('console', (m) => m.type() === 'error' && console.warn(`  console: ${m.text().slice(0, 300)}`));
	await page.addInitScript({ content: init });
	await page.goto(base);
	await page.waitForFunction(() => document.documentElement.dataset.ready === '1', null, {
		timeout: 120_000,
	});
	return page;
}

const bytesOf = (format) => {
	const file = join(repo, FIXTURES[format]);
	results.fixtures[format] = { file: FIXTURES[format], bytes: readFileSync(file).length };
	return [...readFileSync(file)];
};

for (const format of formats) {
	const page = await openPage();
	const bytes = bytesOf(format);
	if (format === 'xlsx') {
		await scenario(page, format, 'first paint', () =>
			page.evaluate(([b, n]) => window.harness.openXlsx(new Uint8Array(b), n), [bytes, FIXTURES.xlsx]),
		);
		await scenario(page, format, 'steady scroll', () => page.evaluate(scrollInPage, [SCROLL_FRAMES, 60]), {
			includeZeros: true,
			window: [WARMUP_FRAMES],
		});
	} else if (format === 'docx') {
		await scenario(page, format, 'first paint (Draft)', () =>
			page.evaluate((b) => window.harness.openDocx(new Uint8Array(b)), bytes),
		);
		await scenario(page, format, 'steady scroll (Draft)', () => page.evaluate(scrollInPage, [SCROLL_FRAMES, 40]), {
			includeZeros: true,
			window: [WARMUP_FRAMES],
		});
		await scenario(page, format, 'first paint (Print Layout)', async () => {
			await page.getByRole('button', { name: 'Print Layout', exact: true }).first().click();
		});
		await scenario(page, format, 'steady scroll (Print Layout)', () => page.evaluate(scrollInPage, [SCROLL_FRAMES, 40]), {
			includeZeros: true,
			window: [WARMUP_FRAMES],
		});
	} else if (format === 'pptx') {
		const count = await page.evaluate((b) => window.harness.loadPptx(new Uint8Array(b)), bytes);
		results.fixtures.pptx.slides = count;
		await scenario(page, format, 'first paint (slide 1)', () => page.evaluate(() => window.harness.showSlide(0)));
		for (let i = 1; i < Math.min(count, 11); i++)
			await scenario(page, format, `slide change to ${i + 1}`, () =>
				page.evaluate((n) => window.harness.showSlide(n), i),
			);
	}
	await page.close();
}

await browser.close();
await server.close();
writeFileSync(OUT, `${JSON.stringify(results, null, 1)}\n`);
console.log(`wrote ${OUT}`);
