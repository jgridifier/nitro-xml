// SPDX-License-Identifier: MIT
/**
 * Pixel check after geometry edits (#84).
 *
 * Applies pptx-move-s0 and pptx-geometry-rotate with the TS editor, saves a
 * temp pptx, then screenshots slide 0 at DPR 1: upstream paint vs the Rust
 * viewer. Mismatch must stay within 0.5% (post-edit bound).
 *
 * Skips (exit 0) when Playwright or the visual host is not available.
 *
 *   bun parity/edit/pptx_pixel_after_edit.mjs
 */

import { mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { applyAndSave } from './apply.mjs';
import { fixtureAbs, resolveFixture } from './scripts.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, '../..');
const OUT = join(HERE, 'out/pixel');
/** Post-edit bound from the issue brief (0.5% of pixels). */
const MISMATCH_LIMIT = 0.005;
const SCRIPTS = ['pptx-move-s0', 'pptx-geometry-rotate'];

function skip(reason) {
	console.log(`pixel-after-edit: SKIP (${reason})`);
	process.exit(0);
}

async function paintPair(browser, base, fixtureRel, id) {
	const c = {
		id,
		kind: 'pptx-viewer',
		format: 'pptx',
		fixture: fixtureRel,
		width: 1280,
		height: 720,
		slide: 0,
	};
	const { comparePngBuffers } = await import('../visual/compare.mjs');
	async function shot(side) {
		const context = await browser.newContext({
			viewport: { width: c.width, height: c.height },
			deviceScaleFactor: 1,
		});
		try {
			const page = await context.newPage();
			const q = new URLSearchParams({
				side,
				id: c.id,
				fixture: `/repo/${c.fixture}`,
				width: String(c.width),
				height: String(c.height),
				slide: '0',
			});
			await page.goto(`${base}/pages/pptx-viewer.html?${q}`, { waitUntil: 'networkidle' });
			await page.waitForFunction(
				() => {
					const el = document.getElementById('stage');
					return el && (el.dataset.ready === '1' || el.dataset.ready === 'err');
				},
				null,
				{ timeout: 90_000 },
			);
			const ready = await page.evaluate(() => document.getElementById('stage')?.dataset.ready);
			if (ready !== '1') {
				const body = await page.evaluate(() =>
					document.getElementById('stage')?.textContent?.slice(0, 300),
				);
				throw new Error(`host not ready (${side}): ${body || ready}`);
			}
			return await page.locator('#stage').screenshot({ type: 'png' });
		} finally {
			await context.close();
		}
	}
	const ts = await shot('ts');
	const rust = await shot('rust');
	const cmp = comparePngBuffers(ts, rust, { format: 'pptx', threshold: MISMATCH_LIMIT });
	return { ratio: cmp.ratio, mismatched: cmp.mismatched, pixels: cmp.total };
}

async function main() {
	let chromium;
	try {
		const require = createRequire(join(ROOT, 'parity/visual/package.json'));
		({ chromium } = require('playwright'));
	} catch (e) {
		skip(`playwright not installed (${e && e.message ? e.message : e})`);
	}
	if (!existsSync(join(ROOT, 'parity/visual/out/bundles/pptx-slide.js'))) {
		skip('pptx slide bundle not prepared');
	}

	mkdirSync(OUT, { recursive: true });
	const { startVisualServer } = await import('../visual/server.mjs');
	const server = await startVisualServer();
	const browser = await chromium.launch({
		headless: true,
		args: ['--no-sandbox', '--disable-dev-shm-usage'],
	});
	try {
		for (const id of SCRIPTS) {
			const script = JSON.parse(readFileSync(join(ROOT, 'edits/pptx', `${id}.json`), 'utf8'));
			const row = resolveFixture(script);
			const input = new Uint8Array(readFileSync(fixtureAbs(row)));
			const saved = await applyAndSave(input, script);
			const rel = `parity/edit/out/pixel/${id}.pptx`;
			writeFileSync(join(ROOT, rel), saved);
			const result = await paintPair(browser, server.url, rel, id);
			const pct = (result.ratio * 100).toFixed(3);
			console.log(
				`pixel-after-edit: ${id} mismatch ${pct}% (${result.mismatched}/${result.pixels})`,
			);
			if (result.ratio > MISMATCH_LIMIT) {
				console.error(`pixel-after-edit: FAIL ${id} ${(result.ratio * 100).toFixed(3)}% > 0.5%`);
				process.exitCode = 1;
			}
		}
	} finally {
		await browser.close();
		await server.close();
	}
	if (process.exitCode) process.exit(process.exitCode);
	console.log('pixel-after-edit: PASS');
}

main().catch((e) => {
	const msg = String(e && e.message ? e.message : e);
	if (/browserType\.launch|Executable doesn't exist|playwright/i.test(msg)) {
		skip(msg.slice(0, 180));
	}
	console.error(e);
	process.exit(1);
});
