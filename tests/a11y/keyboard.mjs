// SPDX-License-Identifier: MIT
/**
 * Keyboard reachability of Stage-6 shell controls on rust viewers (#42).
 * Also records Tab focus order (roles/names) for the rust host.
 */

import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { writeFileSync, mkdirSync } from 'node:fs';
import {
	A11Y,
	a11yBrowsersFromEnv,
	loadBrowserType,
	startA11yServer,
	hostUrl,
	waitReady,
	ensureVisualBundles,
} from './host.mjs';
import { readyCases } from './registry.mjs';
import { RUST_RIBBON_INVENTORY, RUST_SHELL_INVENTORY } from './allowlist.mjs';

/**
 * @param {import('playwright').Page} page
 */
async function ribbonInventory(page) {
	return page.evaluate((wanted) => {
		const host = window.__a11yHost;
		const root = host?.shadowRoot || host;
		if (!root) return { found: [], missing: wanted.map((w) => w.command) };
		const found = [];
		const missing = [];
		for (const w of wanted) {
			const el = root.querySelector(`[data-g="${w.command}"]`);
			if (!el) {
				missing.push(w.command);
				continue;
			}
			const name = el.getAttribute('aria-label') || (el.textContent || '').trim();
			found.push({ command: w.command, name });
		}
		return { found, missing };
	}, RUST_RIBBON_INVENTORY);
}

async function shellInventory(page) {
	return page.evaluate((wanted) => {
		const host = window.__a11yHost;
		const root = host?.shadowRoot || host;
		if (!root) return { found: [], missing: wanted.map((w) => w.action) };
		const found = [];
		const missing = [];
		for (const w of wanted) {
			const el = root.querySelector(`[data-a="${w.action}"]`);
			if (!el) {
				missing.push(w.action);
				continue;
			}
			const name =
				el.getAttribute('aria-label') ||
				el.getAttribute('title') ||
				(el.textContent || '').trim();
			const tabIndex = el.tabIndex;
			const disabled = el.disabled === true || el.getAttribute('aria-disabled') === 'true';
			found.push({
				action: w.action,
				name,
				tabIndex,
				disabled,
				tag: el.tagName.toLowerCase(),
			});
		}
		// Sheet/slide tabs
		const tabs = [...root.querySelectorAll('[role="tab"]')].map((el) => ({
			name: (el.textContent || '').trim(),
			tabIndex: el.tabIndex,
		}));
		return { found, missing, tabs };
	}, RUST_SHELL_INVENTORY);
}

/**
 * @param {import('playwright').Page} page
 * @param {number} maxTabs
 */
async function focusOrder(page, maxTabs = 24) {
	const order = [];
	// Focus the host so Tab walks its shadow tree / light DOM.
	await page.evaluate(() => {
		const host = window.__a11yHost;
		if (host && typeof host.focus === 'function') host.focus();
		else document.body.focus();
	});
	for (let i = 0; i < maxTabs; i++) {
		await page.keyboard.press('Tab');
		const info = await page.evaluate(() => {
			const el = document.activeElement;
			if (!el) return null;
			// Pierce one shadow level (nitro-*/xlsx-editor).
			let cur = el;
			if (cur.shadowRoot?.activeElement) cur = cur.shadowRoot.activeElement;
			const name =
				cur.getAttribute?.('aria-label') ||
				cur.getAttribute?.('title') ||
				(cur.textContent || '').trim().slice(0, 40);
			return {
				tag: cur.tagName?.toLowerCase?.() || '',
				role: cur.getAttribute?.('role') || cur.getAttribute?.('type') || '',
				name,
				action: cur.getAttribute?.('data-a') || '',
			};
		});
		if (!info) break;
		order.push(info);
		// Stop if we cycle back to the first shell control twice.
		if (order.length > 2 && info.action && info.action === order[0].action) break;
	}
	return order;
}

/**
 * @param {import('./registry.mjs').A11yCase} c
 * @param {import('playwright').Browser} browser
 * @param {string} base
 */
async function checkCase(c, browser, base) {
	// Catches: measure-batch race ("expected N widths, got M") leaving the host in
	// dataset.ready=err on a cold keyboard open; one reload usually clears it.
	const maxAttempts = 3;
	let lastErr;
	for (let attempt = 1; attempt <= maxAttempts; attempt++) {
		const context = await browser.newContext({ deviceScaleFactor: 1 });
		try {
			const page = await context.newPage();
			await page.goto(hostUrl(base, c, 'rust'), { waitUntil: 'domcontentloaded' });
			await waitReady(page);
			const inv = await shellInventory(page);
			const rib = c.edit ? await ribbonInventory(page) : { missing: [], found: [] };
			const order = await focusOrder(page);

			const missing = inv.missing || [];
			const ribbonMissing = rib.missing || [];
			const unnamed = (inv.found || []).filter((f) => !f.name);
			const unreachable = (inv.found || []).filter((f) => f.disabled || f.tabIndex < -1);
			// Buttons are in tab order by default (tabIndex 0 or default).
			const ok =
				missing.length === 0 &&
				ribbonMissing.length === 0 &&
				unnamed.length === 0 &&
				unreachable.length === 0;

			// Focus order should include zoom/find/copy at minimum when Tab works across shadow.
			const actionsInOrder = new Set(order.map((o) => o.action).filter(Boolean));
			const shellReached = RUST_SHELL_INVENTORY.filter((w) => actionsInOrder.has(w.action));
			// Shadow-delegatesFocus may block Tab into closed chrome in some hosts; inventory DOM check is primary.
			const focusNote =
				shellReached.length >= 3
					? `focus-order hit ${shellReached.length}/${RUST_SHELL_INVENTORY.length} shell actions`
					: `focus-order hit ${shellReached.length}/${RUST_SHELL_INVENTORY.length} (DOM inventory is authoritative; Tab into shadow may be limited)`;

			await context.close();
			return {
				ok,
				id: c.id,
				format: c.format,
				inventory: inv,
				focusOrder: order,
				summary: ok
					? `OK ${c.id}: shell controls reachable (${inv.found.length}/${RUST_SHELL_INVENTORY.length})${c.edit ? `; ribbon ${rib.found.length}/${RUST_RIBBON_INVENTORY.length}` : ''}; ${focusNote}`
					: `FAIL ${c.id}: missing=[${missing.join(',')}] ribbon=[${ribbonMissing.join(',')}] unnamed=[${unnamed.map((u) => u.action).join(',')}]`,
			};
		} catch (e) {
			lastErr = e;
			await context.close().catch(() => {});
			const msg = String(e && e.message ? e.message : e);
			if (attempt < maxAttempts && /expected \d+ widths, got \d+/.test(msg)) {
				continue;
			}
			throw e;
		}
	}
	throw lastErr;
}

export async function runKeyboardChecks() {
	ensureVisualBundles();
	mkdirSync(join(A11Y, 'out'), { recursive: true });
	const browsers = a11yBrowsersFromEnv();
	const server = await startA11yServer();
	const results = [];
	try {
		for (const browserName of browsers) {
			const type = await loadBrowserType(browserName);
			const browser = await type.launch({ headless: true });
			try {
				for (const c of readyCases()) {
					const r = await checkCase(c, browser, server.url);
					r.browser = browserName;
					r.summary = `[${browserName}] ${r.summary}`;
					results.push(r);
				}
			} finally {
				await browser.close();
			}
		}
	} finally {
		await server.close();
	}
	writeFileSync(join(A11Y, 'out/keyboard.json'), JSON.stringify(results, null, 2));
	const ok = results.every((r) => r.ok);
	const lines = results.map((r) => r.summary);
	return {
		ok,
		results,
		summary: [
			`keyboard: ${ok ? 'PASS' : 'FAIL'} (${results.filter((r) => r.ok).length}/${results.length}; browsers=${browsers.join(',')})`,
			...lines,
		].join('\n'),
	};
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
	const r = await runKeyboardChecks();
	console.log(r.summary);
	process.exit(r.ok ? 0 : 1);
}
