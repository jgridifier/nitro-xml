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
	{ format: 'pptx', page: 'pptx-a11y.html', fixture: 'fixtures/generated/pptx/slides_10.pptx', next: 'Next slide' },
];

const url = (base, f, w, mode) =>
	`${base}/repo/tests/a11y/pages/${f.page}?${new URLSearchParams({
		side: 'rust', id: `embedded-${f.format}`, fixture: `/repo/${f.fixture}`, width: String(w), height: '400',
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
	const axe = await page.evaluate(async () => {
		const r = await axe.run(document, { resultTypes: ['violations'], runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'best-practice'] } });
		return r.violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id);
	});
	const axeNew = axe.filter((id) => !['landmark-one-main', 'region'].includes(id)); // same allow-list as default mode (#29)
	if (axeNew.length) errs.push(`axe: ${axeNew.join(',')}`);

	const tabStops = await page.evaluate(() => [...window.__a11yHost.shadowRoot.querySelectorAll('.nx .nx-b')].filter((b) => b.tabIndex === 0).length);
	if (tabStops !== 1) errs.push(`roving tabindex: ${tabStops} tab stops`);

	// Roving focus + state mirrored from the re-rendered Rust chrome.
	const nxBtn = (name) => page.locator(`css=nitro-${f.format} >> .nx-b[aria-label="${name}"]`);
	await nxBtn('Copy').focus();
	await page.keyboard.press('ArrowRight');
	const afterArrow = await page.evaluate(active);
	if (afterArrow !== 'Next slide') errs.push(`ArrowRight skipped the disabled Previous button: got ${afterArrow}`);
	const prevDisabled = await nxBtn('Previous slide').isDisabled();
	await page.keyboard.press('Enter');
	await page.waitForFunction(() => window.__a11yHost.shadowRoot.querySelector('.nx-r[data-r=nav]').textContent.startsWith('2 /'), null, { timeout: 10_000 }).catch(() => errs.push('Next did not advance / readout not mirrored'));
	const prevAfter = await nxBtn('Previous slide').isDisabled();
	if (!prevDisabled || prevAfter) errs.push(`Previous disabled state not mirrored (first=${prevDisabled}, after Next=${prevAfter})`);
	await page.keyboard.press('End');
	if ((await page.evaluate(active)) !== 'Zoom in') errs.push('End did not reach last button');

	// Find toggle mirrors the find bar.
	await nxBtn('Find').click();
	await page.waitForFunction(() => window.__a11yHost.shadowRoot.querySelector('.nx-b[aria-label=Find]').getAttribute('aria-pressed') === 'true', null, { timeout: 5_000 }).catch(() => {});
	if ((await nxBtn('Find').getAttribute('aria-pressed')) !== 'true') errs.push('Find not aria-pressed while the find bar is open');

	// Narrow: groups collapse into More; Escape closes and returns focus to it.
	await open(240, 'embedded');
	const more = page.locator(`css=nitro-${f.format} >> .nx-mb`);
	if (!(await more.isVisible())) errs.push('no More button at 240px');
	else {
		await more.click();
		if ((await more.getAttribute('aria-expanded')) !== 'true') errs.push('More did not expand');
		await page.keyboard.press('Escape');
		if ((await more.getAttribute('aria-expanded')) !== 'false' || (await page.evaluate(active)) !== 'More') errs.push('Escape did not close More and refocus it');
	}
	const clipped = await page.evaluate(() => { const b = window.__a11yHost.shadowRoot.querySelector('.nx'); return b.scrollWidth > b.clientWidth + 1; });
	if (clipped) errs.push('toolbar still overflows at 240px');
	await ctx.close();
	return { ok: !errs.length, summary: `[${browserName}] ${errs.length ? 'FAIL' : 'OK'} embedded-${f.format}${errs.length ? ': ' + errs.join('; ') : ''}` };
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
