#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Static server for the perf harness (#6). Localhost only, no outbound requests.
 *
 * Sends Cross-Origin-Opener-Policy: same-origin and Cross-Origin-Embedder-Policy: require-corp
 * (cross-origin isolation, required for performance.measureUserAgentSpecificMemory) and
 * Cache-Control: no-store by default (--cache switches to max-age for warm runs; the runner's
 * cold mode also disables the HTTP cache over CDP).
 *
 * Serves only /perf/ (minus node_modules), /fixtures/ and /packages/nitro-xml/. The upstream submodule is
 * never served; TS-engine bundles are built into /perf/dist/ts/ by build-ts.mjs.
 *
 *   node perf/server.mjs [--port=8123] [--host=127.0.0.1] [--cache]
 */
import { createServer } from 'node:http';
import { createReadStream, existsSync, statSync } from 'node:fs';
import { dirname, extname, join, normalize, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

export const REPO = join(dirname(fileURLToPath(import.meta.url)), '..');
const ALLOWED = ['perf', 'fixtures', 'packages/nitro-xml'];
const BLOCKED = ['perf/node_modules']; // never serve dependency trees

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.wasm': 'application/wasm',
  '.ttf': 'font/ttf',
  '.woff2': 'font/woff2',
  '.xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  '.xlsm': 'application/vnd.ms-excel.sheet.macroEnabled.12',
  '.docx': 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  '.pptx': 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
};

const ISOLATION = {
  'cross-origin-opener-policy': 'same-origin',
  'cross-origin-embedder-policy': 'require-corp',
  'cross-origin-resource-policy': 'same-origin',
};
/** `no-store` for cold runs; `cache` lets warm runs reuse HTTP and V8 code caches like a returning visitor. */
const CACHE_CONTROL = { 'no-store': 'no-store', cache: 'public, max-age=3600' };

/** Map a URL path to a file under REPO, or null when outside the allow-list / traversal. */
export function resolve(urlPath) {
  let p;
  try {
    p = normalize(decodeURIComponent(urlPath)).replace(/\\/g, '/');
  } catch {
    return null;
  }
  const rel = p.replace(/^\/+/, '');
  if (rel.split('/').includes('..')) return null;
  if (!ALLOWED.some((a) => rel === a || rel.startsWith(a + '/'))) return null;
  if (BLOCKED.some((b) => rel === b || rel.startsWith(b + '/'))) return null;
  const file = join(REPO, rel);
  if (!file.startsWith(REPO + sep)) return null;
  return file;
}

export function startServer({ port = 0, host = '127.0.0.1', cache = 'no-store' } = {}) {
  const HEADERS = { ...ISOLATION, 'cache-control': CACHE_CONTROL[cache] };
  const server = createServer((req, res) => {
    if (req.method !== 'GET' && req.method !== 'HEAD') {
      res.writeHead(405, HEADERS).end();
      return;
    }
    const { pathname } = new URL(req.url, 'http://localhost');
    const file = resolve(pathname === '/' ? '/perf/pages/index.html' : pathname);
    if (!file || !existsSync(file) || !statSync(file).isFile()) {
      res.writeHead(404, { ...HEADERS, 'content-type': 'text/plain' }).end(`not found: ${pathname}`);
      return;
    }
    res.writeHead(200, {
      ...HEADERS,
      'content-type': MIME[extname(file)] ?? 'application/octet-stream',
      'content-length': statSync(file).size,
    });
    if (req.method === 'HEAD') res.end();
    else createReadStream(file).pipe(res);
  });
  return new Promise((ok, fail) => {
    server.once('error', fail);
    server.listen(port, host, () => {
      const { port: p } = server.address();
      ok({ server, origin: `http://${host}:${p}`, close: () => new Promise((r) => server.close(r)) });
    });
  });
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const a = Object.fromEntries(process.argv.slice(2).map((s) => s.replace(/^--/, '').split('=')));
  const { origin } = await startServer({ port: Number(a.port ?? 8123), host: a.host ?? '127.0.0.1', cache: a.cache ? 'cache' : 'no-store' });
  console.log(`perf server: ${origin}/perf/pages/xlsx.html?engine=ts&fixture=xlsx-S`);
}
