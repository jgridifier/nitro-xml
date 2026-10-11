// SPDX-License-Identifier: MIT
/**
 * CDP Accessibility.getFullAXTree differential: roles + names (#42).
 * Compares content-surface nodes; shell chrome diffs are allow-listed (#29).
 */

import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { writeFileSync, mkdirSync } from 'node:fs';
import {
	A11Y,
	loadChromium,
	startA11yServer,
	hostUrl,
	waitReady,
	ensureVisualBundles,
} from './host.mjs';
import { readyCases } from './registry.mjs';
import { AX_ALLOW, axAllowed, PPTX_SHELL_ONLY_ROLES } from './allowlist.mjs';

/** Roles that must match between TS and rust content surfaces. */
const CONTENT_ROLES = new Set([
	'grid',
	'rowheader',
	'columnheader',
	'region',
	'status',
	'tablist',
	'tab',
]);

/**
 * @param {import('playwright').Page} page
 */
async function snapshotAx(page) {
	const client = await page.context().newCDPSession(page);
	await client.send('Accessibility.enable');
	const { nodes } = await client.send('Accessibility.getFullAXTree');
	await client.detach();
	return (nodes || [])
		.map((n) => {
			const role = n.role?.value ?? n.role ?? '';
			const name = (n.name?.value ?? n.name ?? '').trim();
			const roledescription = (
				n.roledescription?.value ??
				n.properties?.find?.((p) => p.name === 'roledescription')?.value?.value ??
				''
			).toString();
			return { role: String(role), name: String(name), roledescription: String(roledescription) };
		})
		.filter((n) => n.role && n.role !== 'none' && n.role !== 'InlineTextBox' && n.role !== 'generic');
}

/**
 * Normalize to multiset keys for content roles (+ slide region by roledescription).
 * @param {Array<{role:string,name:string,roledescription:string}>} nodes
 * @param {'ts'|'rust'} side
 * @param {import('./registry.mjs').A11yCase} c
 */
function contentKeys(nodes, side, c) {
	const keys = [];
	for (const n of nodes) {
		const isSlide =
			n.roledescription === 'slide' || (n.role === 'region' && n.name === 'Slide');
		const interesting =
			CONTENT_ROLES.has(n.role) ||
			isSlide ||
			(c.format === 'xlsx' && (n.role === 'grid' || n.role === 'status'));
		if (!interesting) continue;
		if (axAllowed(AX_ALLOW, n, side)) continue;
		// Pptx/docx TS oracles are bare reading stages — skip nitro shell tabs on rust.
		if (
			(c.format === 'pptx' || c.format === 'docx') &&
			PPTX_SHELL_ONLY_ROLES.has(n.role)
		) {
			continue;
		}
		// Header names are column letters / row numbers — keep role+name.
		// For grids/status/regions, name matters (Spreadsheet grid / Slide).
		if (n.role === 'rowheader' || n.role === 'columnheader') {
			keys.push(`${n.role}:${n.name}`);
		} else if (isSlide) {
			keys.push(`region:slide:${n.name || 'Slide'}`);
		} else if (n.role === 'tab') {
			// Sheet tab labels may differ; require presence, not cardinality.
			keys.push('tab');
		} else if (n.role === 'tablist') {
			keys.push('tablist');
		} else if (n.role === 'region' && n.name === 'Spreadsheet editor') {
			keys.push('region:Spreadsheet editor');
		} else {
			keys.push(`${n.role}:${n.name}`);
		}
	}
	return keys.sort();
}

function multisetDiff(a, b) {
	const count = (arr) => {
		const m = new Map();
		for (const k of arr) m.set(k, (m.get(k) || 0) + 1);
		return m;
	};
	const A = count(a);
	const B = count(b);
	const onlyA = [];
	const onlyB = [];
	for (const [k, n] of A) {
		const m = B.get(k) || 0;
		for (let i = 0; i < n - m; i++) onlyA.push(k);
	}
	for (const [k, n] of B) {
		const m = A.get(k) || 0;
		for (let i = 0; i < n - m; i++) onlyB.push(k);
	}
	return { onlyA, onlyB };
}

/**
 * @param {import('./registry.mjs').A11yCase} c
 * @param {import('playwright').Browser} browser
 * @param {string} base
 */
async function checkCase(c, browser, base) {
	/** @type {Record<string, string[]>} */
	const keys = {};
	/** @type {Record<string, unknown>} */
	const raw = {};
	for (const side of /** @type {const} */ (['ts', 'rust'])) {
		const context = await browser.newContext({ deviceScaleFactor: 1 });
		const page = await context.newPage();
		await page.goto(hostUrl(base, c, side), { waitUntil: 'domcontentloaded' });
		await waitReady(page);
		const nodes = await snapshotAx(page);
		raw[side] = nodes;
		keys[side] = contentKeys(nodes, side, c);
		await context.close();
	}

	// xlsx: require shared grid + status; headers may differ in count (viewport) — compare role presence sets.
	// pptx: require slide region.
	const { onlyA: onlyTs, onlyB: onlyRust } = multisetDiff(keys.ts, keys.rust);

	// Viewport-dependent header counts: allow header-only cardinality diffs when both sides have ≥1.
	const filterHeaders = (list) =>
		list.filter((k) => !k.startsWith('rowheader:') && !k.startsWith('columnheader:'));
	const tsHasHdr =
		keys.ts.some((k) => k.startsWith('rowheader:')) &&
		keys.ts.some((k) => k.startsWith('columnheader:'));
	const rustHasHdr =
		keys.rust.some((k) => k.startsWith('rowheader:')) &&
		keys.rust.some((k) => k.startsWith('columnheader:'));
	let failTs = onlyTs;
	let failRust = onlyRust;
	if (c.format === 'xlsx' && tsHasHdr && rustHasHdr) {
		failTs = filterHeaders(onlyTs);
		failRust = filterHeaders(onlyRust);
	}
	// Sheet tab count can differ (hidden sheets / naming); require both sides have tablist + ≥1 tab.
	if (c.format === 'xlsx') {
		const tsTabs = keys.ts.filter((k) => k === 'tab').length;
		const rustTabs = keys.rust.filter((k) => k === 'tab').length;
		const tsList = keys.ts.includes('tablist');
		const rustList = keys.rust.includes('tablist');
		if (tsTabs >= 1 && rustTabs >= 1 && tsList && rustList) {
			failTs = failTs.filter((k) => k !== 'tab' && k !== 'tablist');
			failRust = failRust.filter((k) => k !== 'tab' && k !== 'tablist');
		}
	}

	const ok = failTs.length === 0 && failRust.length === 0;
	return {
		ok,
		id: c.id,
		format: c.format,
		tsKeys: keys.ts,
		rustKeys: keys.rust,
		onlyTs: failTs,
		onlyRust: failRust,
		raw,
		summary: ok
			? `OK ${c.id}: AX content roles match (ts=${keys.ts.length} rust=${keys.rust.length} keys)`
			: `FAIL ${c.id}: AX diff onlyTs=[${failTs.slice(0, 8).join('; ')}] onlyRust=[${failRust.slice(0, 8).join('; ')}]`,
	};
}

export async function runAxTreeChecks() {
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
	writeFileSync(
		join(A11Y, 'out/ax-tree.json'),
		JSON.stringify(
			results.map(({ raw, ...rest }) => rest),
			null,
			2,
		),
	);
	const ok = results.every((r) => r.ok);
	const lines = results.map((r) => r.summary);
	return {
		ok,
		results,
		summary: [`ax-tree: ${ok ? 'PASS' : 'FAIL'} (${results.filter((r) => r.ok).length}/${results.length})`, ...lines].join('\n'),
	};
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
	const r = await runAxTreeChecks();
	console.log(r.summary);
	process.exit(r.ok ? 0 : 1);
}
