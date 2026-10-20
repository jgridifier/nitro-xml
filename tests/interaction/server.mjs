// SPDX-License-Identifier: MIT
// Static server for the local demo build: mirrors what scripts/ci/pages.sh vendors into _site/demo
// (demo/ + packages/nitro-xml/loader + packages/nitro-xml/pkg) without copying anything.
import http from 'node:http';
import { createReadStream, statSync } from 'node:fs';
import { join, extname, normalize, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const MIME = {
	'.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css',
	'.wasm': 'application/wasm', '.json': 'application/json', '.svg': 'image/svg+xml',
	'.png': 'image/png', '.xlsx': 'application/octet-stream', '.docx': 'application/octet-stream',
	'.pptx': 'application/octet-stream',
};

function resolve(urlPath) {
	const p = normalize(decodeURIComponent(urlPath)).replace(/^(\.\.[/\\])+/, '');
	const maps = [
		['/demo/nitro-xml/loader/', join(ROOT, 'packages/nitro-xml/loader/')],
		['/demo/nitro-xml/pkg/', join(ROOT, 'packages/nitro-xml/pkg/')],
		['/demo/', join(ROOT, 'demo/')],
	];
	for (const [prefix, dir] of maps) if (p.startsWith(prefix)) return join(dir, p.slice(prefix.length));
	return null;
}

export function startServer() {
	return new Promise((res) => {
		const s = http.createServer((req, rsp) => {
			const f = resolve(new URL(req.url, 'http://x').pathname);
			try {
				if (!f || !statSync(f).isFile()) throw new Error('404');
				rsp.writeHead(200, { 'content-type': MIME[extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
				createReadStream(f).pipe(rsp);
			} catch {
				rsp.writeHead(404).end('not found');
			}
		});
		s.listen(0, '127.0.0.1', () => res({ url: `http://127.0.0.1:${s.address().port}/demo`, close: () => s.close() }));
	});
}
