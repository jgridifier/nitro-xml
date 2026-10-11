// SPDX-License-Identifier: MIT
/**
 * Shared Playwright helpers for the a11y gate (#42).
 * Reuses the visual-gate static server + upstream bundles from #45.
 */

import { join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { startVisualServer } from '../../parity/visual/server.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const VISUAL = join(ROOT, 'parity/visual');
const A11Y = dirname(fileURLToPath(import.meta.url));

export { ROOT, VISUAL, A11Y };

export async function loadChromium() {
	const mod = await import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);
	return mod.chromium;
}

export async function startA11yServer() {
	// Serve visual root so /out/bundles, /fonts, /repo work; a11y pages via /repo/tests/a11y/pages.
	return startVisualServer({ root: VISUAL });
}

/**
 * @param {string} base
 * @param {import('./registry.mjs').A11yCase} c
 * @param {'ts'|'rust'} side
 */
export function hostUrl(base, c, side) {
	const page = c.format === 'pptx' ? 'pptx-a11y.html' : 'xlsx-a11y.html';
	const q = new URLSearchParams({
		side,
		id: c.id,
		fixture: `/repo/${c.fixture}`,
		width: String(c.width || 800),
		height: String(c.height || 480),
	});
	if (c.format === 'pptx') q.set('slide', String(c.slide || 0));
	return `${base}/repo/tests/a11y/pages/${page}?${q}`;
}

export async function waitReady(page, timeoutMs = 45_000) {
	await page.waitForFunction(() => {
		const el = document.getElementById('stage');
		return el && (el.dataset.ready === '1' || el.dataset.ready === 'err');
	}, null, { timeout: timeoutMs });
	const ready = await page.evaluate(() => document.getElementById('stage')?.dataset.ready);
	if (ready !== '1') {
		const body = await page.evaluate(() => document.getElementById('stage')?.textContent?.slice(0, 600));
		throw new Error(`a11y host not ready: ${body || ''}`);
	}
}

/** Ensure visual-gate upstream bundles + font mirror exist (same as #45 prepare). */
export function ensureVisualBundles() {
	const prepare = join(VISUAL, 'prepare.mjs');
	const xlsxBundle = join(VISUAL, 'out/bundles/xlsx-editor.js');
	const pptxBundle = join(VISUAL, 'out/bundles/pptx-slide.js');
	const fontMirror = join(VISUAL, 'out/fonts/css2');
	if (existsSync(xlsxBundle) && existsSync(pptxBundle) && existsSync(fontMirror)) return;
	const r = spawnSync('bun', [prepare], {
		cwd: VISUAL,
		encoding: 'utf8',
		env: { ...process.env, UPSTREAM_DIR: join(ROOT, 'upstream') },
		maxBuffer: 64 * 1024 * 1024,
	});
	if (r.status !== 0) throw new Error(`visual prepare: ${r.stderr || r.stdout}`);
}
