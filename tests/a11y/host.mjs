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

/** Playwright engines used by the a11y gate (#42 / #29 cross-browser AC). */
export const A11Y_BROWSER_NAMES = /** @type {const} */ (['chromium', 'webkit', 'firefox']);

/**
 * Browsers to run. Default: all three. Override with `A11Y_BROWSERS=chromium,webkit`.
 * AX-tree stays Chromium-only (CDP Accessibility.getFullAXTree).
 */
export function a11yBrowsersFromEnv() {
	const raw = (process.env.A11Y_BROWSERS || A11Y_BROWSER_NAMES.join(','))
		.split(',')
		.map((s) => s.trim().toLowerCase())
		.filter(Boolean);
	const out = [];
	for (const name of raw) {
		if (!A11Y_BROWSER_NAMES.includes(/** @type {*} */ (name))) {
			throw new Error(`a11y: unknown browser ${name} (want ${A11Y_BROWSER_NAMES.join('|')})`);
		}
		if (!out.includes(name)) out.push(name);
	}
	return out.length ? out : [...A11Y_BROWSER_NAMES];
}

export async function loadPlaywright() {
	return import(pathToFileURL(join(VISUAL, 'node_modules/playwright/index.mjs')).href);
}

export async function loadChromium() {
	const mod = await loadPlaywright();
	return mod.chromium;
}

/**
 * @param {'chromium'|'webkit'|'firefox'} name
 */
export async function loadBrowserType(name) {
	const mod = await loadPlaywright();
	const t = mod[name];
	if (!t) throw new Error(`a11y: playwright has no engine ${name}`);
	return t;
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
	const page =
		c.format === 'pptx'
			? 'pptx-a11y.html'
			: c.format === 'docx'
				? 'docx-a11y.html'
				: 'xlsx-a11y.html';
	const q = new URLSearchParams({
		side,
		id: c.id,
		fixture: `/repo/${c.fixture}`,
		width: String(c.width || 800),
		height: String(c.height || 480),
	});
	if (c.format === 'pptx') q.set('slide', String(c.slide || 0));
	if (c.edit) q.set('edit', '1');
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
	const docxBundle = join(VISUAL, 'out/bundles/docx-print.js');
	const fontMirror = join(VISUAL, 'out/fonts/css2');
	if (
		existsSync(xlsxBundle) &&
		existsSync(pptxBundle) &&
		existsSync(docxBundle) &&
		existsSync(fontMirror)
	) {
		return;
	}
	const r = spawnSync('bun', [prepare], {
		cwd: VISUAL,
		encoding: 'utf8',
		env: { ...process.env, UPSTREAM_DIR: join(ROOT, 'upstream') },
		maxBuffer: 64 * 1024 * 1024,
	});
	if (r.status !== 0) throw new Error(`visual prepare: ${r.stderr || r.stdout}`);
}
