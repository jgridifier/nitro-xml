// SPDX-License-Identifier: MIT
// Parse race runner. Headless system Chrome via playwright-core; a local static server (with
// COOP/COEP so the page is cross-origin isolated and performance.now() has its fine resolution);
// for every fixture in fixtures/real/{xlsx,docx,pptx}: entrants in a seeded random order, each in a
// fresh browser context and page, bytes fetched into memory first, 2 warm-up runs, then N timed
// runs of parse() only. Writes results/parse/<UTC date>-<git short sha>.json and
// docs/scoreboard/data/parse-latest.json.
//
// Options: --runs=7 --warmup=2 --seed=20261011 --format=xlsx --entrants=a,b --files=substr
//          --chrome=/usr/bin/google-chrome --out=<file>
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import os from 'node:os';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';
import { firstDifference } from './lib/normalise.mjs';
import { median, summarise } from './lib/summary.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..', '..');
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
const TIMEOUT_MS = Number(args.timeout ?? 180000);
const FORMATS = args.format ? args.format.split(',') : ['xlsx', 'docx', 'pptx'];
const CHROME_FLAGS = ['--enable-precise-memory-info', '--js-flags=--expose-gc', '--disable-renderer-backgrounding', '--disable-background-timer-throttling'];

const competitors = JSON.parse(readFileSync(join(here, 'competitors.json'), 'utf8'));
const bundleStatus = existsSync(join(here, 'dist/bundles/status.json')) ? JSON.parse(readFileSync(join(here, 'dist/bundles/status.json'), 'utf8')) : {};
const rustStatus = existsSync(join(here, 'dist/rust/status.json')) ? JSON.parse(readFileSync(join(here, 'dist/rust/status.json'), 'utf8')) : {};
const REFERENCE = competitors.entrants.find((e) => e.reference).id;

// Entrants: entered ones whose bundles built; anything that failed to build is "not entered".
const entrants = competitors.entrants.map((e) => {
	const base = { id: e.id, name: e.name, formats: e.formats, version: e.version, licence: e.licence, language: e.language, timingScope: e.timingScope ?? 'parse', fastestConfig: e.fastestConfig ?? null, status: e.status, reason: e.reason ?? null };
	if (e.status !== 'entered' || !e.races.some((r) => r.startsWith('parse'))) return base;
	const problems = [];
	for (const [fmt, bundle] of Object.entries(e.adapters)) {
		const st = bundleStatus[bundle];
		if (!st || !st.built) problems.push(`${fmt} adapter did not build: ${st?.error ?? 'not built'}`);
		if (bundle.startsWith('rust-')) {
			const rs = rustStatus[bundle.slice(5)];
			if (!rs || !rs.built) problems.push(`does not build for wasm32 (${rs?.error ?? 'not built'})`);
			else base.wasmBytes = rs.wasmBytes;
		}
		if (st?.bundleBytes) (base.bundleBytes ??= {})[fmt] = st.bundleBytes;
	}
	if (e.scripts && !bundleStatus['pptxjs-vendor']?.built) problems.push(`vendor scripts unavailable: ${bundleStatus['pptxjs-vendor']?.error}`);
	if (problems.length) return { ...base, status: 'not entered', reason: `not entered: ${problems.join('; ')}` };
	return { ...base, adapters: e.adapters, scripts: e.scripts ?? [], refuses: e.refuses ?? {} };
});
let active = entrants.filter((e) => e.status === 'entered');
if (args.entrants) active = active.filter((e) => args.entrants.split(',').includes(e.id) || e.id === REFERENCE);

// Seeded PRNG (mulberry32) for the per-file entrant order.
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

// Static server: /dist, /page, /adapters from perf/scoreboard; /fixtures from the repo.
const MIME = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json' };
const server = createServer((req, res) => {
	const url = new URL(req.url, 'http://x');
	let path = decodeURIComponent(url.pathname);
	if (path === '/') path = '/page/index.html';
	const root = path.startsWith('/fixtures/') ? repo : here;
	const file = normalize(join(root, path));
	if (!file.startsWith(root) || !existsSync(file) || !statSync(file).isFile()) {
		res.writeHead(404).end();
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

const browser = await chromium.launch({ executablePath: CHROME, headless: true, args: CHROME_FLAGS });
const chromeVersion = browser.version();

const fixtures = [];
for (const format of FORMATS) {
	const dir = join(repo, 'fixtures', 'real', format);
	for (const f of readdirSync(dir).sort()) {
		if (!/\.(xls[xm]|xltx|doc[xm]|dotx|ppt[xm]|potx)$/.test(f)) continue;
		if (args.files && !f.includes(args.files)) continue;
		fixtures.push({ format, file: f, path: `/fixtures/real/${format}/${f}`, bytes: statSync(join(dir, f)).size });
	}
}

async function runOne(entrant, fx) {
	const context = await browser.newContext();
	const page = await context.newPage();
	const consoleErrors = [];
	page.on('pageerror', (e) => consoleErrors.push(String(e.message).slice(0, 300)));
	let timer;
	try {
		const work = (async () => {
			await page.goto(origin + '/');
			await page.waitForFunction(() => window.__sbReady === true);
			const setup = await page.evaluate((a) => window.__sb.setup(a), {
				bundle: `/dist/bundles/${entrant.adapters[fx.format]}.js`,
				scripts: entrant.scripts,
				fixtureUrl: fx.path,
			});
			const r = await page.evaluate((a) => window.__sb.run(a), { warmup: WARMUP, runs: RUNS, format: fx.format });
			return { ...r, crossOriginIsolated: setup.crossOriginIsolated };
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

const files = [];
const started = new Date();
let isolated = null;
for (const fx of fixtures) {
	const order = shuffle(active.filter((e) => e.formats.includes(fx.format)));
	const results = {};
	const outputs = {};
	for (const entrant of order) {
		const ext = extname(fx.file).slice(1);
		if (entrant.refuses?.[ext]) {
			results[entrant.id] = { status: 'skipped', reason: entrant.refuses[ext] };
			continue;
		}
		const r = await runOne(entrant, fx);
		if (r.error) {
			results[entrant.id] = { status: 'error', error: r.error };
			process.stdout.write(`${fx.format}/${fx.file} ${entrant.id}: ERROR ${r.error}\n`);
			continue;
		}
		isolated ??= r.crossOriginIsolated;
		outputs[entrant.id] = r.output;
		results[entrant.id] = {
			status: 'ok',
			timesMs: r.times.map((t) => Math.round(t * 1000) / 1000),
			medianMs: median(r.times),
			minMs: Math.min(...r.times),
			maxMs: Math.max(...r.times),
			approxHeapBeforeBytes: r.heapBefore,
			approxHeapAfterBytes: r.heapAfter,
			wasmMemoryBeforeBytes: r.wasmBefore,
			wasmMemoryAfterBytes: r.wasmAfter,
			outputSize: r.output.length,
		};
	}
	const ref = outputs[REFERENCE];
	for (const [id, r] of Object.entries(results)) {
		if (r.status !== 'ok') continue;
		if (id === REFERENCE) {
			r.comparable = true;
			continue;
		}
		if (!ref) {
			r.comparable = false;
			r.firstDifference = { reason: 'reference (upstream TS) failed on this file' };
			continue;
		}
		const diff = firstDifference(fx.format, ref, outputs[id]);
		r.comparable = diff === null;
		if (diff) {
			r.firstDifference = diff;
			// Flags likely reference gaps: other entrants whose output is identical to this one.
			const same = Object.keys(outputs).filter((o) => o !== id && o !== REFERENCE && firstDifference(fx.format, outputs[o], outputs[id]) === null);
			if (same.length) r.sameOutputAs = same;
		}
	}
	files.push({ format: fx.format, file: fx.file, bytes: fx.bytes, entrantOrder: order.map((e) => e.id), results });
	const line = order.map((e) => {
		const r = results[e.id];
		return `${e.id}=${r.status === 'ok' ? `${r.medianMs.toFixed(2)}ms${r.comparable ? '' : '(nc)'}` : r.status}`;
	});
	process.stdout.write(`${fx.format}/${fx.file}: ${line.join(' ')}\n`);
}
await browser.close();
server.close();

const sha = git('rev-parse', 'HEAD');
const short = git('rev-parse', '--short', 'HEAD');
let memInfo = null;
try {
	memInfo = readFileSync('/proc/meminfo', 'utf8').split('\n')[0];
} catch {}
const result = {
	race: 'parse',
	schema: 1,
	generatedAt: new Date().toISOString(),
	startedAt: started.toISOString(),
	git: { sha, short, branch: git('rev-parse', '--abbrev-ref', 'HEAD'), upstream: git('-C', 'upstream', 'rev-parse', 'HEAD') },
	machine: {
		cpuModel: os.cpus()[0]?.model ?? null,
		logicalCores: os.cpus().length,
		ramBytes: os.totalmem(),
		memInfo,
		platform: `${os.type()} ${os.release()} ${os.arch()}`,
		node: process.version,
		ci: process.env.GITHUB_ACTIONS === 'true' ? `GitHub Actions ${process.env.RUNNER_OS ?? ''} ${process.env.ImageOS ?? ''}`.trim() : null,
	},
	browser: { name: 'Google Chrome (headless)', executable: CHROME, version: chromeVersion, flags: CHROME_FLAGS, crossOriginIsolated: isolated },
	method: {
		warmupRuns: WARMUP,
		timedRuns: RUNS,
		timer: 'performance.now() around parse() only; bytes already in memory; a fresh copy of the bytes per run, made outside the timed span',
		gcBetweenRuns: 'gc() (--expose-gc) before every run, outside the timed span',
		isolation: 'fresh browser context and page per (file, entrant)',
		order: `entrants in a seeded random order per file (mulberry32, seed ${SEED})`,
		memory: 'approximate: performance.memory.usedJSHeapSize just before and just after the last timed parse (model retained), plus wasm linear memory size for wasm entrants; not a peak measurement',
		reference: REFERENCE,
		equivalence: 'see docs/scoreboard-method.md#equivalence',
	},
	entrants,
	evaluated: competitors.evaluated,
	files,
	summary: summarise(files, entrants, REFERENCE),
};

const date = new Date().toISOString().slice(0, 10);
const outFile = args.out ?? join(here, 'results', 'parse', `${date}-${short ?? 'nogit'}.json`);
mkdirSync(dirname(outFile), { recursive: true });
writeFileSync(outFile, JSON.stringify(result, null, 1) + '\n');
if (!args.out) {
	const latest = join(repo, 'docs', 'scoreboard', 'data', 'parse-latest.json');
	mkdirSync(dirname(latest), { recursive: true });
	copyFileSync(outFile, latest);
}
console.log(`wrote ${outFile}`);
for (const [format, rows] of Object.entries(result.summary)) {
	for (const [id, r] of Object.entries(rows)) {
		if (r.status !== 'entered') console.log(`${format} ${id}: ${r.status}`);
		else console.log(`${format} ${id}: comparable ${r.comparable}/${r.files}, median-of-medians ${r.medianOfMediansMs?.toFixed(2)} ms, geomean vs TS ${r.geomeanRatioVsReference?.toFixed(3)} on ${r.sharedWithReference}, W/L ${r.winsVsReference}/${r.lossesVsReference}, errors ${r.errors}`);
	}
}
