#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Perf runner (#6): Playwright Chromium + one CDP session per page, against the localhost hosts in
 * perf/pages. The single documented entrypoint (see perf/README.md).
 *
 *   node perf/run.mjs [--engine ts|wasm|ts,wasm] [--mode cold|warm] [--iterations N]
 *                     [--cpu-throttle 1|4] [--fixtures xlsx-S,docx,M,...] [--memory]
 *                     [--out file.json] [--chrome /path/to/chrome] [--timeout ms]
 *
 *   cold  A new browser context per iteration; HTTP cache disabled over CDP and by the server
 *         (no-store), which also removes the V8 script and WASM compiled-code caches (both live in the
 *         HTTP cache); the browser process is shared but contexts do not share cache or storage.
 *   warm  One context. One discarded warm-up load, then N loads in the same context with the HTTP
 *         cache on, so V8/WASM code caches work as for a returning visitor.
 *   Defaults: 10 iterations cold, 20 warm.
 */
import { mkdirSync, mkdtempSync, existsSync, rmSync, writeFileSync, readFileSync } from 'node:fs';
import { cpus, platform, release, tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';
import { startServer } from './server.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..');

// --flag value | --flag=value | --flag (boolean)
function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const m = /^--([^=]+)(?:=(.*))?$/.exec(argv[i]);
    if (!m) throw new Error(`unexpected argument: ${argv[i]}`);
    if (m[2] !== undefined) out[m[1]] = m[2];
    else if (argv[i + 1] !== undefined && !argv[i + 1].startsWith('--')) out[m[1]] = argv[++i];
    else out[m[1]] = true;
  }
  return out;
}
const args = parseArgs(process.argv.slice(2));
const KNOWN = ['engine', 'mode', 'iterations', 'cpu-throttle', 'fixtures', 'memory', 'out', 'chrome', 'timeout', 'help'];
for (const k of Object.keys(args)) if (!KNOWN.includes(k)) throw new Error(`unknown flag --${k} (see perf/README.md)`);
if (args.help) {
  console.log(readFileSync(fileURLToPath(import.meta.url), 'utf8').split('/**\n')[1].split('*/')[0].replace(/^ \* ?/gm, ''));
  process.exit(0);
}

const MODE = args.mode ?? 'cold';
if (!['cold', 'warm'].includes(MODE)) throw new Error('--mode must be cold or warm');
const ITERATIONS = Number(args.iterations ?? (MODE === 'cold' ? 10 : 20));
if (!(ITERATIONS >= 1)) throw new Error('--iterations must be >= 1');
const THROTTLE = Number(args['cpu-throttle'] ?? 1);
if (![1, 4].includes(THROTTLE)) throw new Error('--cpu-throttle must be 1 or 4');
const ENGINES = String(args.engine ?? 'ts').split(',');
for (const e of ENGINES) if (!['ts', 'wasm'].includes(e)) throw new Error(`--engine: unknown engine ${e}`);
const MEMORY = args.memory === true || args.memory === 'true';
const TIMEOUT = Number(args.timeout ?? 120000);
const VIEWPORT = { width: 1280, height: 800 };

// ---- fixtures -------------------------------------------------------------------------------
const corpus = JSON.parse(readFileSync(join(here, 'fixtures.json'), 'utf8')).fixtures;
// Selector tokens: fixture id (xlsx-S), format (docx), or tier (M).
const tokens = args.fixtures ? String(args.fixtures).split(',') : null;
const selected = corpus.filter(
  (f) => !tokens || tokens.some((t) => t === f.id || t === f.format || t === f.tier),
);
if (!selected.length) throw new Error(`--fixtures ${args.fixtures} matched nothing in perf/fixtures.json`);
const fixtures = selected.filter((f) => {
  const ok = existsSync(join(repo, f.path));
  if (!ok) console.warn(`skip ${f.id}: ${f.path} not generated (fixtures/gen)`);
  return ok;
});
if (!fixtures.length) throw new Error('no selected fixture exists on disk');

// ---- engine prerequisites -------------------------------------------------------------------
for (const e of ENGINES) {
  if (e === 'ts') {
    for (const f of new Set(fixtures.map((x) => x.format))) {
      if (!existsSync(join(here, 'dist/ts', `${f}.js`))) throw new Error(`missing perf/dist/ts/${f}.js; run: node perf/build-ts.mjs`);
    }
  } else if (!existsSync(join(repo, 'packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm'))) {
    throw new Error('engine wasm needs packages/nitro-xml/pkg; run: bash scripts/build-wasm.sh');
  }
}

// ---- stats ----------------------------------------------------------------------------------
const sorted = (xs) => [...xs].sort((a, b) => a - b);
const pct = (xs, p) => {
  const a = sorted(xs);
  const i = (a.length - 1) * p;
  const lo = Math.floor(i);
  return a[lo] + (a[Math.ceil(i)] - a[lo]) * (i - lo);
};
const summarize = (xs) =>
  xs.length
    ? { n: xs.length, min: Math.min(...xs), median: pct(xs, 0.5), p95: pct(xs, 0.95), max: Math.max(...xs), mean: xs.reduce((s, x) => s + x, 0) / xs.length }
    : null;

// ---- one load -------------------------------------------------------------------------------
const HEAP = ['JSHeapUsedSize', 'JSHeapTotalSize'];

async function loadOnce(context, origin, fx, engine, { cacheDisabled }) {
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  const net = { requests: 0, fromCache: 0, blocked: 0 };
  try {
    await cdp.send('Network.enable');
    await cdp.send('Network.setCacheDisabled', { cacheDisabled });
    await cdp.send('Performance.enable');
    if (THROTTLE !== 1) await cdp.send('Emulation.setCPUThrottlingRate', { rate: THROTTLE });
    cdp.on('Network.responseReceived', (e) => {
      net.requests++;
      if (e.response.fromDiskCache || e.response.fromServiceWorker) net.fromCache++;
    });
    page.on('requestfailed', (r) => r.failure()?.errorText === 'net::ERR_NAME_NOT_RESOLVED' && net.blocked++);
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e.message).slice(0, 300)));

    const url = `${origin}/perf/pages/${fx.format}.html?engine=${engine}&fixture=${fx.id}`;
    await page.goto(url, { waitUntil: 'commit', timeout: TIMEOUT });
    await page.waitForFunction(() => window.__perf?.done === true, null, { timeout: TIMEOUT, polling: 50 });
    const res = await page.evaluate(() => window.__perf);
    if (res.error) throw new Error(`${fx.id}/${engine}: ${res.error}${errors.length ? ` | pageerror: ${errors[0]}` : ''}`);

    const m = res.marks;
    const sample = {
      engine_ms: m.engine_ready,
      fetch_ms: m.fixture_ready - m.engine_ready,
      render_ms: m.first_paint - m.render_start,
      first_paint_ms: m.first_paint,
    };
    if (m.all_slides !== undefined) {
      sample.all_slides_ms = m.all_slides;
      sample.all_slides_render_ms = m.all_slides - m.render_start;
    }
    const metrics = Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map((x) => [x.name, x.value]));
    for (const k of HEAP) sample[k] = metrics[k];
    if (MEMORY) {
      sample.uamem_bytes = await page.evaluate(async () => {
        if (!crossOriginIsolated || !performance.measureUserAgentSpecificMemory) return null;
        try {
          return (await Promise.race([performance.measureUserAgentSpecificMemory(), new Promise((_, r) => setTimeout(() => r(), 20000))])).bytes;
        } catch {
          return null;
        }
      });
    }
    return { sample, detail: res.detail, net };
  } finally {
    await page.close().catch(() => {});
  }
}

// ---- main -----------------------------------------------------------------------------------
const server = await startServer({ cache: MODE === 'warm' ? 'cache' : 'no-store' });
const origin = server.origin;
const launchOpts = {
  headless: true,
  ...(args.chrome ? { executablePath: String(args.chrome) } : { channel: 'chromium' }),
  viewport: VIEWPORT,
  deviceScaleFactor: 1,
  serviceWorkers: 'block',
  args: [
    '--enable-precise-memory-info',
    '--disable-renderer-backgrounding',
    '--disable-background-timer-throttling',
    '--disable-background-networking',
    '--disable-component-update',
    '--no-default-browser-check',
    '--disk-cache-size=1073741824',
    // Offline guarantee: every host except localhost fails to resolve (counted as a blocked request).
    // Playwright's context.route() would do it too but switches the HTTP cache off, defeating warm mode.
    '--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1',
  ],
};
// Cold: shared browser, a new in-memory context per iteration. Warm: one persistent context (disk-backed
// HTTP cache, so the 12 MiB upstream bundles are cacheable and V8 can keep code caches), in a temp profile.
const profile = MODE === 'warm' ? mkdtempSync(join(tmpdir(), 'nitro-perf-')) : null;
const browser = MODE === 'cold' ? await chromium.launch(launchOpts) : null;
const warmContext = MODE === 'warm' ? await chromium.launchPersistentContext(profile, launchOpts) : null;
const chromeVersion = browser ? browser.version() : warmContext.browser().version();
const newContext = () => browser.newContext({ viewport: VIEWPORT, deviceScaleFactor: 1, serviceWorkers: 'block' });

const results = [];
const failures = [];
const summary = {
  chrome: chromeVersion,
  playwright: JSON.parse(readFileSync(join(here, 'node_modules/playwright-core/package.json'), 'utf8')).version,
  node: process.version,
  platform: `${platform()} ${release()}`,
  cpu: `${cpus()[0]?.model ?? 'unknown'} x${cpus().length}`,
  mode: MODE,
  iterations: ITERATIONS,
  cpuThrottle: THROTTLE,
  viewport: VIEWPORT,
  upstreamPin: 'ChristopherVR/ooxml@e66aaec24adc0b68472eae1fc42aa4598e09c352',
  date: new Date().toISOString(),
};
console.log(`chrome ${summary.chrome} · playwright ${summary.playwright} · ${MODE} x${ITERATIONS} · cpu-throttle ${THROTTLE}x`);

let blockedTotal = 0;
for (const fx of fixtures) {
  for (const engine of ENGINES) {
    const samples = [];
    let detail = null;
    let cacheHits = 0;
    try {
      if (MODE === 'cold') {
        for (let i = 0; i < ITERATIONS; i++) {
          const context = await newContext();
          try {
            const r = await loadOnce(context, origin, fx, engine, { cacheDisabled: true });
            samples.push(r.sample);
            detail = r.detail;
            cacheHits += r.net.fromCache;
            blockedTotal += r.net.blocked;
          } finally {
            await context.close();
          }
        }
      } else {
        const context = warmContext;
        try {
          await loadOnce(context, origin, fx, engine, { cacheDisabled: false }); // discarded warm-up
          for (let i = 0; i < ITERATIONS; i++) {
            const r = await loadOnce(context, origin, fx, engine, { cacheDisabled: false });
            samples.push(r.sample);
            detail = r.detail;
            blockedTotal += r.net.blocked;
          }
        } finally {
          // the persistent context is shared across fixtures; only its pages are closed (in loadOnce)
        }
      }
    } catch (e) {
      failures.push({ fixture: fx.id, engine, error: String(e.message ?? e) });
      console.error(`FAIL ${fx.id} ${engine}: ${e.message ?? e}`);
      continue;
    }
    if (MODE === 'cold' && cacheHits) failures.push({ fixture: fx.id, engine, error: `cold run served ${cacheHits} responses from cache` });
    const keys = Object.keys(samples[0]);
    const stats = Object.fromEntries(keys.map((k) => [k, summarize(samples.map((s) => s[k]).filter((v) => typeof v === 'number'))]));
    results.push({ fixture: fx.id, format: fx.format, tier: fx.tier, engine, detail, stats, samples });
  }
}
await (browser ?? warmContext).close();
if (profile) rmSync(profile, { recursive: true, force: true });
await server.close();

// ---- report ---------------------------------------------------------------------------------
const f1 = (x) => (x == null ? '-' : x.toFixed(1));
console.log(`\nfixture    engine  first_paint ms (median / p95)   render ms (median)   all_slides ms (median)   heap MiB`);
for (const r of results) {
  const s = r.stats;
  console.log(
    `${r.fixture.padEnd(10)} ${r.engine.padEnd(7)} ${`${f1(s.first_paint_ms.median)} / ${f1(s.first_paint_ms.p95)}`.padEnd(30)} ${f1(s.render_ms.median).padEnd(20)} ${f1(s.all_slides_ms?.median).padEnd(24)} ${f1(s.JSHeapUsedSize.median / 1048576)}`,
  );
}
if (blockedTotal) console.warn(`\nWARNING: ${blockedTotal} non-localhost request(s) were blocked; a host tried to reach the network.`);

const outFile = args.out ? String(args.out) : join(here, 'results', `${summary.date.replace(/[:.]/g, '-')}-${MODE}-${ENGINES.join('+')}.json`);
mkdirSync(dirname(outFile), { recursive: true });
writeFileSync(outFile, JSON.stringify({ ...summary, blockedRequests: blockedTotal, results, failures }, null, 2));
console.log(`\nwrote ${outFile.replace(repo + '/', '')}`);
if (failures.length) {
  console.error(`${failures.length} failure(s)`);
  process.exit(1);
}
