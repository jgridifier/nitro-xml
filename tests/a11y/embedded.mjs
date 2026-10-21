// SPDX-License-Identifier: MIT
/**
 * Embedded-mode toolbar (Stream F): axe (zero serious/critical, absolute: there is no TS twin),
 * one roving tab stop, arrow/Home/End, state mirrored from the Rust chrome after it re-renders,
 * overflow menu at narrow width, and a default-mode negative control.
 */
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import {
	A11Y,
	a11yBrowsersFromEnv,
	loadBrowserType,
	startA11yServer,
	waitReady,
} from './host.mjs';

const AXE_SRC = join(A11Y, 'node_modules/axe-core/axe.min.js');

/** One entry per format whose embedded toolbar has shipped. */
const FORMATS = [
	{ format: 'docx', page: 'docx-a11y.html', fixture: 'fixtures/generated/docx/pages_10.docx' }, // one shell page: Pages group hides itself
	{ format: 'xlsx', page: 'xlsx-a11y.html', fixture: 'fixtures/real/xlsx/poi-45544.xlsx', id: 'xlsx-readonly' },
	{ format: 'xlsx', page: 'xlsx-a11y.html', fixture: 'fixtures/real/xlsx/poi-45544.xlsx', id: 'xlsx-edit', edit: true, after: 'Bold' },
	{ format: 'pptx', page: 'pptx-a11y.html', fixture: 'fixtures/generated/pptx/slides_10.pptx', next: 'Next slide', prev: 'Previous slide' },
];

const url = (base, f, w, mode) =>
	`${base}/repo/tests/a11y/pages/${f.page}?${new URLSearchParams({
		side: 'rust', id: `embedded-${f.id || f.format}`, ...(f.edit ? { edit: '1' } : {}), fixture: `/repo/${f.fixture}`, width: String(w), height: '400',
		...(mode ? { mode } : {}),
	})}`;

// Runs in the page. Returns the active element's aria-label inside the host shadow root.
const active = () => window.__a11yHost.shadowRoot.activeElement?.getAttribute('aria-label') || null;

async function check(f, browser, base, browserName) {
	const errs = [];
	const ctx = await browser.newContext({ deviceScaleFactor: 1 });
	const page = await ctx.newPage();
	const open = async (w, mode) => {
		await page.goto(url(base, f, w, mode), { waitUntil: 'domcontentloaded' });
		await waitReady(page);
		if (mode) await page.waitForSelector('css=nitro-' + f.format + ' >> .nx', { timeout: 15_000 });
	};

	// Negative control: default mode must keep the old toolbar and mount nothing.
	await open(960, '');
	const dflt = await page.evaluate(() => {
		const r = window.__a11yHost.shadowRoot;
		return { nx: !!r.querySelector('.nx'), old: getComputedStyle(r.querySelector('.chrome .b')).display };
	});
	if (dflt.nx || dflt.old === 'none') errs.push(`default mode changed: ${JSON.stringify(dflt)}`);

	await open(960, 'embedded');
	await page.addScriptTag({ path: AXE_SRC });
	const axe = await page.evaluate(async (fmt) => {
		const r = await axe.run(document, { resultTypes: ['violations'], runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'best-practice'] } });
		// Toolbar nodes only: the grid / Rust chrome are covered (differentially) by the default-mode cases.
		const mine = (n) => /class="nx[ "-]/.test(n.html);
		return r.violations.filter((v) => (v.impact === 'serious' || v.impact === 'critical') && v.nodes.some(mine)).map((v) => v.id + ':' + v.nodes.filter(mine).slice(0, 2).map((n) => n.target.join(' ') + ' ' + n.html.slice(0, 90) + ' ' + (n.failureSummary || '').slice(0, 160)).join(' | '));
	}, f.format);
	if (axe.length) errs.push(`axe: ${axe.join(',')}`);

	const tabStops = await page.evaluate(() => [...window.__a11yHost.shadowRoot.querySelectorAll('.nx .nx-b')].filter((b) => b.tabIndex === 0).length);
	if (tabStops !== 1) errs.push(`roving tabindex: ${tabStops} tab stops`);

	// Roving focus + state mirrored from the re-rendered Rust chrome.
	// Cut/Copy/Paste stay in the DOM but hidden until the editor emits them; the view-only shell Copy shares the name.
	const nxBtn = (name) => page.locator(`css=nitro-${f.format} >> .nx-b[aria-label="${name}"]:not([hidden])`);
	await nxBtn('Copy').focus();
	await page.keyboard.press('ArrowRight');
	if (f.next) {
		const afterArrow = await page.evaluate(active);
		if (afterArrow !== f.next) errs.push(`ArrowRight skipped the disabled Previous button (expected ${f.next}): got ${afterArrow}`);
		const prevDisabled = await nxBtn(f.prev).isDisabled();
		await page.keyboard.press('Enter');
		await page.waitForFunction(() => window.__a11yHost.shadowRoot.querySelector('.nx-r[data-r=nav]').textContent.startsWith('2 /'), null, { timeout: 10_000 }).catch(() => errs.push('Next did not advance / readout not mirrored'));
		const prevAfter = await nxBtn(f.prev).isDisabled();
		if (!prevDisabled || prevAfter) errs.push(`Previous disabled state not mirrored (first=${prevDisabled}, after Next=${prevAfter})`);
	} else if ((await page.evaluate(active)) !== (f.after || 'Find')) errs.push(`expected ${f.after || 'Find'} after Copy (a group that must be hidden is showing?)`);
	await page.keyboard.press('End');
	if ((await page.evaluate(active)) !== 'Zoom in') errs.push('End did not reach last button');

	if (f.edit) {
		// A narrow CI font/width may have moved the group into More: open it first.
		const press = async (name) => {
			if (!(await nxBtn(name).isVisible())) await page.locator(`css=nitro-${f.format} >> .nx-mb`).click();
			await nxBtn(name).click();
		};
		// Wiring + mirroring through the real engine: Insert sheet adds a tab, Undo removes it.
		const tabs = () => page.evaluate(() => window.__a11yHost.shadowRoot.querySelectorAll('.chrome .t button').length);
		const n0 = await tabs();
		await press('Insert sheet');
		await page.waitForFunction((n) => window.__a11yHost.shadowRoot.querySelectorAll('.chrome .t button').length === n + 1, n0, { timeout: 10_000 }).catch(() => errs.push('Insert sheet did not add a tab'));
		await press('Undo');
		await page.waitForFunction((n) => window.__a11yHost.shadowRoot.querySelectorAll('.chrome .t button').length === n, n0, { timeout: 10_000 }).catch(() => errs.push('Undo did not remove the inserted sheet'));
	}

	// Find toggle mirrors the find bar.
	await nxBtn('Find').click();
	await page.waitForFunction(() => window.__a11yHost.shadowRoot.querySelector('.nx-b[aria-label=Find]').getAttribute('aria-pressed') === 'true', null, { timeout: 5_000 }).catch(() => {});
	if ((await nxBtn('Find').getAttribute('aria-pressed')) !== 'true') errs.push('Find not aria-pressed while the find bar is open');

	// Narrow: groups collapse into More; Escape closes and returns focus to it.
	await open(240, 'embedded');
	const more = page.locator(`css=nitro-${f.format} >> .nx-mb`);
	if (await more.isVisible()) {
		await more.click();
		if ((await more.getAttribute('aria-expanded')) !== 'true') errs.push('More did not expand');
		await page.keyboard.press('Escape');
		if ((await more.getAttribute('aria-expanded')) !== 'false' || (await page.evaluate(active)) !== 'More') errs.push('Escape did not close More and refocus it');
	}
	const clipped = await page.evaluate(() => { const b = window.__a11yHost.shadowRoot.querySelector('.nx'); return b.scrollWidth > b.clientWidth + 1; });
	if (clipped) errs.push('toolbar still overflows at 240px');
	await ctx.close();
	return { ok: !errs.length, summary: `[${browserName}] ${errs.length ? 'FAIL' : 'OK'} embedded-${f.id || f.format}${errs.length ? ': ' + errs.join('; ') : ''}` };
}

export async function runEmbeddedChecks() {
	const server = await startA11yServer();
	const results = [];
	try {
		for (const name of a11yBrowsersFromEnv()) {
			const browser = await (await loadBrowserType(name)).launch({ headless: true });
			try { for (const f of FORMATS) results.push(await check(f, browser, server.url, name)); }
			finally { await browser.close(); }
		}
	} finally { await server.close(); }
	const ok = results.every((r) => r.ok);
	return { ok, summary: [`embedded: ${ok ? 'PASS' : 'FAIL'}`, ...results.map((r) => r.summary)].join('\n') };
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
	const r = await runEmbeddedChecks();
	console.log(r.summary);
	process.exit(r.ok ? 0 : 1);
}
