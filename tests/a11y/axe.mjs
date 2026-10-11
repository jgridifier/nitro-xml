// SPDX-License-Identifier: MIT
/**
 * Differential axe-core: zero *new* serious/critical violations on rust vs TS (#42).
 */

import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { writeFileSync, mkdirSync } from 'node:fs';
import {
	A11Y,
	VISUAL,
	loadChromium,
	startA11yServer,
	hostUrl,
	waitReady,
	ensureVisualBundles,
} from './host.mjs';
import { readyCases } from './registry.mjs';
import { AXE_ALLOW, axeAllowed } from './allowlist.mjs';

const AXE_SRC = join(A11Y, 'node_modules/axe-core/axe.min.js');

/**
 * @param {import('playwright').Page} page
 */
async function runAxe(page) {
	await page.addScriptTag({ path: AXE_SRC });
	return page.evaluate(async () => {
		// Pierce open shadow roots used by nitro-* / xlsx-editor.
		const results = await axe.run(document, {
			iframes: false,
			resultTypes: ['violations'],
			runOnly: {
				type: 'tag',
				values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'best-practice'],
			},
		});
		return (results.violations || []).map((v) => ({
			id: v.id,
			impact: v.impact,
			description: v.description,
			nodes: (v.nodes || []).length,
			targets: (v.nodes || []).slice(0, 5).map((n) => (n.target || []).join(' ')),
		}));
	});
}

function seriousCritical(list) {
	return list.filter((v) => v.impact === 'serious' || v.impact === 'critical');
}

function keyOf(v) {
	return v.id;
}

/**
 * @param {import('./registry.mjs').A11yCase} c
 * @param {import('playwright').Browser} browser
 * @param {string} base
 */
async function checkCase(c, browser, base) {
	const sides = /** @type {const} */ (['ts', 'rust']);
	/** @type {Record<string, ReturnType<typeof seriousCritical>>} */
	const bySide = {};
	for (const side of sides) {
		const context = await browser.newContext({ deviceScaleFactor: 1 });
		const page = await context.newPage();
		await page.goto(hostUrl(base, c, side), { waitUntil: 'domcontentloaded' });
		await waitReady(page);
		const all = await runAxe(page);
		bySide[side] = seriousCritical(all).filter((v) => !axeAllowed(AXE_ALLOW, v.id, side));
		await context.close();
	}
	const tsKeys = new Set(bySide.ts.map(keyOf));
	const newOnRust = bySide.rust.filter((v) => !tsKeys.has(keyOf(v)));
	const ok = newOnRust.length === 0;
	return {
		ok,
		id: c.id,
		format: c.format,
		ts: bySide.ts,
		rust: bySide.rust,
		newOnRust,
		summary: ok
			? `OK ${c.id}: axe new-vs-upstream 0 (ts=${bySide.ts.length} rust=${bySide.rust.length} serious/critical after allow-list)`
			: `FAIL ${c.id}: ${newOnRust.length} new serious/critical on rust: ${newOnRust.map((v) => v.id).join(', ')}`,
	};
}

export async function runAxeChecks() {
	ensureVisualBundles();
	mkdirSync(join(A11Y, 'out'), { recursive: true });
	const chromium = await loadChromium();
	const server = await startA11yServer();
	const browser = await chromium.launch({ headless: true });
	const results = [];
	try {
		for (const c of readyCases()) {
			results.push(await checkCase(c, browser, server.url));
		}
	} finally {
		await browser.close();
		await server.close();
	}
	writeFileSync(join(A11Y, 'out/axe.json'), JSON.stringify(results, null, 2));
	const ok = results.every((r) => r.ok);
	const lines = results.map((r) => r.summary);
	return {
		ok,
		results,
		summary: [`axe: ${ok ? 'PASS' : 'FAIL'} (${results.filter((r) => r.ok).length}/${results.length})`, ...lines].join('\n'),
	};
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
	const r = await runAxeChecks();
	console.log(r.summary);
	process.exit(r.ok ? 0 : 1);
}
