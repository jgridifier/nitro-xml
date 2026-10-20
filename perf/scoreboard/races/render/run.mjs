// SPDX-License-Identifier: MIT
// First-paint race runner (#41). Headless Chromium; COOP/COEP; load+init+first paint
// per format×tier. Pixel-comparable vs upstream-ts within PRODUCT_SPEC §5 limits.
//
// Options: --runs=7 --warmup=2 --seed=20261011 --format=xlsx --tier=S,M
//          --entrants=upstream-ts,nitro-xml --chrome=/usr/bin/google-chrome
import { execFileSync } from 'node:child_process';
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	readFileSync,
	statSync,
	writeFileSync,
} from 'node:fs';
import { createServer } from 'node:http';
import os from 'node:os';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';
import { PIXEL_LIMIT, TIERS } from './fixtures.mjs';
import { comparePng } from './pixel.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const scoreboard = join(here, '..', '..');
const repo = join(scoreboard, '..', '..');
const visualOut = join(repo, 'parity', 'visual', 'out');
const args = Object.fromEntries(
	process.argv.slice(2).map((a) => {
		const [k, v] = a.replace(/^--/, '').split('=');
		return [k, v ?? 'true'];
	}),
);
const RUNS = Number(args.runs ?? 7);
const WARMUP = Number(args.warmup ?? 2);
const SEED = Number(args.seed ?? 20261011);
const CHROME = args.chrome ?? process.env.CHROME_PATH ?? '/usr/bin/google-chrome';
const TIMEOUT_MS = Number(args.timeout ?? 60000);
const FORMATS = args.format ? args.format.split(',') : ['xlsx'];
const TIERS_F = args.tier ? args.tier.split(',') : ['S', 'M', 'L', 'XL'];
const CHROME_FLAGS = [
	'--enable-precise-memory-info',
	'--js-flags=--expose-gc',
	'--disable-renderer-backgrounding',
	'--disable-background-timer-throttling',
];

const REFERENCE = 'upstream-ts';
const ENTRANTS = [
	{
		id: 'upstream-ts',
		name: 'upstream ooxml TS',
		formats: ['xlsx'],
		reference: true,
		status: 'entered',
	},
	{
		id: 'nitro-xml',
		name: 'Nitro XML (Rust/WASM)',
		formats: ['xlsx'],
		status: 'entered',
	},
];

function median(xs) {
	if (!xs.length) return NaN;
	const a = [...xs].sort((x, y) => x - y);
	const m = Math.floor(a.length / 2);
	return a.length % 2 ? a[m] : (a[m - 1] + a[m]) / 2;
}

function rng(seed) {
	let a = seed >>> 0;
	return () => {
		a = (a + 0x6d2b79f5) >>> 0;
		let t = a;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}
const rand = rng(SEED);
const shuffle = (xs) => {
	const a = [...xs];
	for (let i = a.length - 1; i > 0; i--) {
		const j = Math.floor(rand() * (i + 1));
		[a[i], a[j]] = [a[j], a[i]];
	}
	return a;
};

const MIME = {
	'.js': 'text/javascript',
	'.mjs': 'text/javascript',
	'.html': 'text/html',
	'.wasm': 'application/wasm',
	'.json': 'application/json',
	'.css': 'text/css',
	'.xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
	'.pptx': 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
	'.docx': 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
};

function resolvePath(urlPath) {
	if (urlPath.startsWith('/fixtures/')) return join(repo, urlPath.slice(1));
	if (urlPath.startsWith('/packages/')) return join(repo, urlPath.slice(1));
	if (urlPath.startsWith('/out/')) return join(visualOut, urlPath.slice('/out/'.length));
	if (urlPath.startsWith('/fonts/')) return join(visualOut, 'fonts', urlPath.slice('/fonts/'.length));
	if (urlPath.startsWith('/parity/visual/')) return join(repo, urlPath.slice(1));
	if (urlPath.startsWith('/races/render/')) return join(here, urlPath.slice('/races/render/'.length));
	if (urlPath === '/' || urlPath === '/page/' || urlPath === '/page/index.html') {
		return join(here, 'page', 'index.html');
	}
	if (urlPath.startsWith('/page/')) return join(here, 'page', urlPath.slice('/page/'.length));
	return null;
}

const server = createServer((req, res) => {
	const url = new URL(req.url, 'http://x');
	let path = decodeURIComponent(url.pathname);
	if (path === '/') path = '/page/index.html';
	const file = resolvePath(path);
	if (!file || !existsSync(file) || !statSync(file).isFile()) {
		res.writeHead(404).end(`missing ${path}`);
		return;
	}
	res.writeHead(200, {
		'content-type': MIME[extname(file)] ?? 'application/octet-stream',
		'cross-origin-opener-policy': 'same-origin',
		'cross-origin-embedder-policy': 'require-corp',
		'cross-origin-resource-policy': 'same-origin',
		'cache-control': 'no-store',
	});
	res.end(readFileSync(file));
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const origin = `http://127.0.0.1:${server.address().port}`;

const git = (...a) => {
	try {
		return execFileSync('git', a, { cwd: repo, encoding: 'utf8' }).trim();
	} catch {
		return null;
	}
};

if (!existsSync(join(repo, 'packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm'))) {
	console.error('missing packages/nitro-xml/pkg — run bash scripts/build-wasm.sh');
	process.exit(1);
}
if (!existsSync(join(visualOut, 'bundles/xlsx-editor.js'))) {
	console.error('missing parity/visual/out/bundles/xlsx-editor.js — run: cd parity/visual && bun ./prepare.mjs');
	process.exit(1);
}

let active = ENTRANTS.filter((e) => e.status === 'entered');
if (args.entrants) {
	const want = new Set(args.entrants.split(','));
	active = active.filter((e) => want.has(e.id) || e.reference);
}

const fixtures = TIERS.filter(
	(f) => FORMATS.includes(f.format) && TIERS_F.includes(f.tier) && existsSync(join(repo, f.rel)),
);
if (!fixtures.length) {
	console.error('no fixtures matched; generate with fixtures/gen/generate.py');
	process.exit(1);
}

const browser = await chromium.launch({
	executablePath: CHROME,
	headless: true,
	args: CHROME_FLAGS,
});
const chromeVersion = browser.version();
const started = new Date();
const files = [];
let isolated = null;

/** One cold sample: fresh context/page (load+init+first paint). */
async function runOneSample(entrant, fx, { screenshot }) {
	const context = await browser.newContext({
		viewport: { width: 800, height: 480 },
		deviceScaleFactor: 1,
	});
	const page = await context.newPage();
	const consoleErrors = [];
	page.on('pageerror', (e) => consoleErrors.push(String(e.message).slice(0, 300)));
	let timer;
	try {
		const work = (async () => {
			await page.goto(origin + '/page/index.html');
			await page.waitForFunction(() => window.__fpReady === true);
			const setup = await page.evaluate(
				(a) => window.__fp.setup(a),
				{
					entrant: entrant.id,
					format: fx.format,
					fixturePath: `/${fx.rel}`,
				},
			);
			const r = await page.evaluate(() => window.__fp.paintOnce());
			await page.waitForFunction(() => document.getElementById('stage')?.dataset?.ready === '1');
			const shot = screenshot ? await page.locator('#stage').screenshot({ type: 'png' }) : null;
			return { ms: r.ms, shot, crossOriginIsolated: setup.crossOriginIsolated };
		})();
		const timeout = new Promise((_, rej) => {
			timer = setTimeout(() => rej(new Error(`timeout after ${TIMEOUT_MS / 1000} s`)), TIMEOUT_MS);
		});
		return await Promise.race([work, timeout]);
	} catch (e) {
		const msg = String(e.message ?? e).replace(/^page\.evaluate: /, '').split('\n')[0].slice(0, 300);
		return { error: msg + (consoleErrors.length ? ` (page error: ${consoleErrors[0]})` : '') };
	} finally {
		clearTimeout(timer);
		await context.close().catch(() => {});
	}
}

async function runEntrant(entrant, fx) {
	const times = [];
	let shot = null;
	let iso = null;
	for (let i = 0; i < WARMUP + RUNS; i++) {
		const last = i === WARMUP + RUNS - 1;
		const r = await runOneSample(entrant, fx, { screenshot: last });
		if (r.error) return { error: r.error };
		iso ??= r.crossOriginIsolated;
		if (i >= WARMUP) times.push(r.ms);
		if (last) shot = r.shot;
	}
	return { times, shot, crossOriginIsolated: iso };
}

for (const fx of fixtures) {
	const order = shuffle(active.filter((e) => e.formats.includes(fx.format)));
	const results = {};
	const shots = {};
	for (const entrant of order) {
		process.stdout.write(`${fx.format}/${fx.tier} ${entrant.id} … `);
		const r = await runEntrant(entrant, fx);
		if (r.error) {
			results[entrant.id] = { status: 'error', error: r.error };
			process.stdout.write(`ERROR ${r.error}\n`);
			continue;
		}
		isolated ??= r.crossOriginIsolated;
		shots[entrant.id] = r.shot;
		results[entrant.id] = {
			status: 'ok',
			timesMs: r.times.map((t) => Math.round(t * 1000) / 1000),
			medianMs: median(r.times),
			minMs: Math.min(...r.times),
			maxMs: Math.max(...r.times),
		};
		process.stdout.write(`${results[entrant.id].medianMs.toFixed(2)} ms\n`);
	}
	const refShot = shots[REFERENCE];
	const limit = PIXEL_LIMIT[fx.format];
	for (const [id, r] of Object.entries(results)) {
		if (r.status !== 'ok') continue;
		if (id === REFERENCE) {
			r.comparable = true;
			r.pixelRatio = 0;
			continue;
		}
		if (!refShot || !shots[id]) {
			r.comparable = false;
			r.pixel = { reason: 'missing screenshot' };
			continue;
		}
		const diffPath = join(
			scoreboard,
			'results',
			'render',
			'diffs',
			`${fx.format}-${fx.tier}-${id}.png`,
		);
		const cmp = comparePng(refShot, shots[id], { limit, diffPath });
		r.comparable = cmp.ok;
		r.pixelRatio = cmp.ratio;
		r.pixel = {
			ok: cmp.ok,
			mismatched: cmp.mismatched,
			total: cmp.total,
			ratio: cmp.ratio,
			limit,
			reason: cmp.reason,
		};
		if (!cmp.ok) r.firstDifference = { kind: 'pixels', ...r.pixel };
	}
	files.push({
		format: fx.format,
		tier: fx.tier,
		file: fx.rel,
		bytes: statSync(join(repo, fx.rel)).size,
		entrantOrder: order.map((e) => e.id),
		results,
	});
}

await browser.close();
server.close();

const sha = git('rev-parse', 'HEAD');
const short = git('rev-parse', '--short', 'HEAD');
const result = {
	race: 'first-paint',
	schema: 1,
	generatedAt: new Date().toISOString(),
	startedAt: started.toISOString(),
	git: { sha, short, branch: git('rev-parse', '--abbrev-ref', 'HEAD') },
	machine: {
		cpuModel: os.cpus()[0]?.model ?? null,
		logicalCores: os.cpus().length,
		ramBytes: os.totalmem(),
		platform: `${os.type()} ${os.release()} ${os.arch()}`,
		node: process.version,
	},
	browser: {
		name: 'Google Chrome (headless)',
		executable: CHROME,
		version: chromeVersion,
		flags: CHROME_FLAGS,
		crossOriginIsolated: isolated,
	},
	method: {
		warmupRuns: WARMUP,
		timedRuns: RUNS,
		timer:
			'performance.now() around module/wasm load + open + first painted grid frame; fixture bytes fetched in setup outside the timed span; fresh browser context and page per sample (cold load each time)',
		viewport: '800x480 CSS px, DPR 1',
		pixelLimits: PIXEL_LIMIT,
		reference: REFERENCE,
		order: `entrants in a seeded random order per fixture (mulberry32, seed ${SEED})`,
		isolation: 'fresh browser context and page per timed sample',
	},
	entrants: active,
	files,
};

const date = new Date().toISOString().slice(0, 10);
const outDir = join(scoreboard, 'results', 'render');
mkdirSync(outDir, { recursive: true });
const outFile = args.out ?? join(outDir, `${date}-${short ?? 'nogit'}.json`);
writeFileSync(outFile, JSON.stringify(result, null, 1) + '\n');
const latest = join(repo, 'docs', 'scoreboard', 'data', 'first-paint-latest.json');
mkdirSync(dirname(latest), { recursive: true });
copyFileSync(outFile, latest);
console.log(`wrote ${outFile}`);
for (const f of files) {
	for (const [id, r] of Object.entries(f.results)) {
		if (r.status !== 'ok') console.log(`${f.format} ${f.tier} ${id}: ${r.status}`);
		else {
			console.log(
				`${f.format} ${f.tier} ${id}: median ${r.medianMs.toFixed(2)} ms` +
					(r.comparable ? '' : ' (nc)') +
					(r.pixelRatio != null ? ` pixels=${(r.pixelRatio * 100).toFixed(3)}%` : ''),
			);
		}
	}
}
