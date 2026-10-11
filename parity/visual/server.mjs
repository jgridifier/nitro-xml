// SPDX-License-Identifier: MIT
/**
 * Minimal static server for the visual gate (#45).
 * Borrowed from the headless-host half of #6 (no COOP/COEP, no perf metrics).
 */

import { createServer } from 'node:http';
import { readFileSync, existsSync, statSync } from 'node:fs';
import { join, extname, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../..', import.meta.url));
const VISUAL = fileURLToPath(new URL('.', import.meta.url));

const TYPES = {
	'.html': 'text/html; charset=utf-8',
	'.js': 'text/javascript; charset=utf-8',
	'.mjs': 'text/javascript; charset=utf-8',
	'.css': 'text/css; charset=utf-8',
	'.json': 'application/json',
	'.svg': 'image/svg+xml',
	'.png': 'image/png',
	'.pdf': 'application/pdf',
	'.wasm': 'application/wasm',
	'.xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
	'.docx': 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
	'.xlsm': 'application/vnd.ms-excel.sheet.macroEnabled.12',
	'.ttf': 'font/ttf',
	'.woff': 'font/woff',
	'.woff2': 'font/woff2',
	'.pptx': 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
	'.pptm': 'application/vnd.ms-powerpoint.presentation.macroEnabled.12',
	'.potx': 'application/vnd.openxmlformats-officedocument.presentationml.template',
	'.potm': 'application/vnd.ms-powerpoint.template.macroEnabled.12',
};

function resolveSafe(base, reqPath) {
	const decoded = decodeURIComponent(reqPath.split('?')[0]);
	const cleaned = normalize(decoded).replace(/^(\.\.(\/|\\|$))+/, '');
	const full = join(base, cleaned);
	if (!full.startsWith(base)) return null;
	return full;
}

/**
 * @param {{ port?: number, root?: string }} [opts]
 * @returns {Promise<{ url: string, port: number, close: () => Promise<void> }>}
 */
export function startVisualServer(opts = {}) {
	const port = opts.port ?? 0;
	const docRoot = opts.root ?? VISUAL;

	const server = createServer((req, res) => {
		const url = new URL(req.url || '/', 'http://127.0.0.1');
		let filePath = null;
		if (url.pathname.startsWith('/repo/')) {
			filePath = resolveSafe(ROOT, url.pathname.slice('/repo/'.length));
		} else if (url.pathname.startsWith('/out/')) {
			filePath = resolveSafe(join(VISUAL, 'out'), url.pathname.slice('/out/'.length));
		} else {
			filePath = resolveSafe(docRoot, url.pathname === '/' ? 'pages/host.html' : url.pathname);
		}
		if (!filePath || !existsSync(filePath) || !statSync(filePath).isFile()) {
			res.writeHead(404);
			res.end('not found');
			return;
		}
		// #121 CSS2 mirror is written as out/fonts/css2 (no extension).
		const ext = extname(filePath);
		const type =
			TYPES[ext] ||
			(/[/\\]fonts[/\\]css2$/.test(filePath) ? 'text/css; charset=utf-8' : 'application/octet-stream');
		res.writeHead(200, {
			'Content-Type': type,
			'Cache-Control': 'no-store',
		});
		res.end(readFileSync(filePath));
	});

	return new Promise((resolve, reject) => {
		server.once('error', reject);
		server.listen(port, '127.0.0.1', () => {
			const addr = server.address();
			const p = typeof addr === 'object' && addr ? addr.port : port;
			resolve({
				url: `http://127.0.0.1:${p}`,
				port: p,
				close: () =>
					new Promise((res, rej) => server.close((err) => (err ? rej(err) : res()))),
			});
		});
	});
}

if (import.meta.main) {
	const s = await startVisualServer({ port: Number(process.env.PORT || 4173) });
	console.log(`visual server ${s.url}`);
}
