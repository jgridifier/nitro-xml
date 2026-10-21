// SPDX-License-Identifier: MIT
/**
 * End-to-end edit of the built GitHub Pages demos (pptx-full, pptx-embedded,
 * docx-full, docx-embedded) on Chromium and WebKit. Rides the selection-parity
 * job: scripts/ci/selection_parity.sh. No new workflow.
 *
 * Catches: a Pages demo whose ribbon edit (type, bold, undo, redo, new slide or
 * table, save) does not survive reopening the downloaded bytes.
 */
import { createServer } from 'node:http';
import { cpSync, createReadStream, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { zipEntries } from './lib.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, '../..');
const MARK = 'NxE2E';
const MIME = {
	'.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css',
	'.wasm': 'application/wasm', '.json': 'application/json', '.svg': 'image/svg+xml',
	'.png': 'image/png', '.xlsx': 'application/octet-stream', '.docx': 'application/octet-stream',
	'.pptx': 'application/octet-stream',
};

const DEMOS = [
	{ format: 'pptx', mode: 'full', structure: 'slide' },
	{ format: 'pptx', mode: 'embedded', structure: 'slide' },
	{ format: 'docx', mode: 'full', structure: 'table' },
	{ format: 'docx', mode: 'embedded', structure: 'table' },
];

function assemble(dir) {
	rmSync(dir, { recursive: true, force: true });
	mkdirSync(dir, { recursive: true });
	cpSync(join(ROOT, 'demo'), dir, { recursive: true });
	rmSync(join(dir, 'nitro-xml'), { recursive: true, force: true });
	const loader = join(dir, 'nitro-xml/loader');
	cpSync(join(ROOT, 'packages/nitro-xml/loader'), loader, {
		recursive: true,
		filter: (src) => !src.split(/[/\\]/).includes('test'),
	});
	const pkg = join(ROOT, 'packages/nitro-xml/pkg');
	if (!statSync(join(pkg, 'nitro_xml_wasm_bg.wasm'), { throwIfNoEntry: false })) {
		throw new Error('demo-edit: missing wasm pkg; run scripts/build-wasm.sh');
	}
	cpSync(pkg, join(dir, 'nitro-xml/pkg'), { recursive: true });
}

function startBuilt(demoDir) {
	return new Promise((resolve) => {
		const s = createServer((req, rsp) => {
			const pathname = decodeURIComponent(new URL(req.url, 'http://x').pathname);
			const rel = pathname.replace(/^\/demo\/?/, '');
			const f = normalize(join(demoDir, rel));
			if (!f.startsWith(demoDir)) {
				rsp.writeHead(404).end('not found');
				return;
			}
			try {
				if (!statSync(f).isFile()) throw new Error('404');
				rsp.writeHead(200, { 'content-type': MIME[extname(f)] || 'application/octet-stream', 'cache-control': 'no-store' });
				createReadStream(f).pipe(rsp);
			} catch {
				rsp.writeHead(404).end('not found');
			}
		});
		s.listen(0, '127.0.0.1', () => resolve({ url: `http://127.0.0.1:${s.address().port}/demo`, close: () => s.close() }));
	});
}

function docxState(buf) {
	const xml = zipEntries(buf)['word/document.xml']().toString('utf8');
	const hit = xml.split('</w:r>').find((r) => r.includes(MARK));
	const bold = !!(hit && /<w:b(?:\s|\/|>)/.test(hit) && !/w:val="0"/.test(hit));
	const tables = (xml.match(/<w:tbl[ >]/g) || []).length;
	return { present: !!hit, bold, count: tables };
}

function pptxState(buf) {
	const z = zipEntries(buf);
	const slides = Object.keys(z).filter((n) => /^ppt\/slides\/slide\d+\.xml$/.test(n));
	let present = false;
	let bold = false;
	for (const name of slides) {
		const xml = z[name]().toString('utf8');
		if (!xml.includes(MARK)) continue;
		present = true;
		const hit = xml.split('</a:r>').find((r) => r.includes(MARK));
		bold = !!(hit && /\bb="1"/.test(hit));
		break;
	}
	return { present, bold, count: slides.length };
}

const stateOf = (format, buf) => (format === 'docx' ? docxState(buf) : pptxState(buf));

async function packageBytes(page, format) {
	const tag = format === 'docx' ? 'nitro-docx' : 'nitro-pptx';
	const arr = await page.evaluate(async (tag) => Array.from(await document.querySelector(tag).save()), tag);
	return Buffer.from(arr);
}

async function placeCaret(page, format) {
	const point = await page.evaluate((format) => {
		const tag = format === 'docx' ? 'nitro-docx' : 'nitro-pptx';
		const sr = document.querySelector(tag).shadowRoot;
		if (format === 'docx') {
			const lines = [...sr.querySelectorAll('.dve-print-line')];
			const line = lines.find((n) => (n.textContent || '').includes('This sample')) || lines[3] || lines[0];
			if (!line) return null;
			const r = line.getBoundingClientRect();
			return { x: r.x + Math.min(120, r.width / 2), y: r.y + r.height / 2, dbl: false };
		}
		const nodes = [...sr.querySelectorAll('.pptxv-text')];
		const node = nodes.find((n) => (n.textContent || '').includes('Open Office')) || nodes[1] || nodes[0];
		if (!node) return null;
		const r = node.getBoundingClientRect();
		return { x: r.x + r.width / 2, y: r.y + r.height / 2, dbl: true };
	}, format);
	if (!point) throw new Error('no text target');
	if (point.dbl) await page.mouse.dblclick(point.x, point.y);
	else await page.mouse.click(point.x, point.y);
	await page.waitForTimeout(350);
}

async function findButton(page, names) {
	for (const name of names) {
		const loc = page.getByRole('button', { name, exact: true });
		const n = await loc.count();
		for (let i = 0; i < n; i++) {
			const b = loc.nth(i);
			if ((await b.isVisible()) && (await b.isEnabled())) return b;
		}
	}
	return null;
}

async function clickButton(page, names, tab) {
	if (tab) {
		const t = page.getByRole('tab', { name: tab, exact: true });
		if ((await t.count()) && (await t.isVisible())) await t.click();
	}
	const deadline = Date.now() + 10_000;
	let triedMore = false;
	while (Date.now() < deadline) {
		const btn = await findButton(page, names);
		if (btn) {
			await btn.click();
			return;
		}
		if (!triedMore) {
			triedMore = true;
			const more = await findButton(page, ['More']);
			if (more) {
				await more.click();
				await page.waitForTimeout(150);
				continue;
			}
		}
		await page.waitForTimeout(200);
	}
	throw new Error(`no enabled button: ${names.join(' / ')}`);
}

async function waitState(page, format, pred, label) {
	const deadline = Date.now() + 12_000;
	let last = null;
	while (Date.now() < deadline) {
		try {
			last = stateOf(format, await packageBytes(page, format));
			if (pred(last)) return last;
		} catch (e) {
			last = { error: String(e.message || e).split('\n')[0] };
		}
		await page.waitForTimeout(200);
	}
	throw new Error(`${label}: ${JSON.stringify(last)}`);
}

async function shot(page, file) {
	mkdirSync(dirname(file), { recursive: true });
	await page.screenshot({ path: file, animations: 'disabled' });
}

/**
 * @returns {Promise<string|null>} failure message
 */
async function editDemo(page, base, demo, shotDir) {
	const id = `${demo.format}-${demo.mode}`;
	await page.goto(`${base}/${demo.format}-${demo.mode}.html`, { waitUntil: 'domcontentloaded', timeout: 60_000 });
	const tag = demo.format === 'docx' ? 'nitro-docx' : 'nitro-pptx';
	await page.waitForFunction(
		(tag) => {
			const el = document.querySelector(tag);
			const sr = el && el.shadowRoot;
			return !!(sr && el.document && sr.querySelector('.dve-print-line, [aria-roledescription=slide]'));
		},
		tag,
		{ timeout: 60_000 },
	);
	await page.waitForTimeout(500);
	await shot(page, join(shotDir, `${id}-before.png`));
	const before = stateOf(demo.format, await packageBytes(page, demo.format));
	if (before.present) return `${MARK} already in the sample`;

	await placeCaret(page, demo.format);
	await page.keyboard.type(MARK, { delay: 40 });
	await waitState(page, demo.format, (s) => s.present, 'typed text missing from save()');
	await clickButton(page, ['Bold']);
	const styled = await waitState(page, demo.format, (s) => s.present && s.bold, 'bold did not mark the typed run');

	await clickButton(page, ['Undo']);
	const undone = await waitState(page, demo.format, (s) => s.present !== styled.present || s.bold !== styled.bold, 'undo left the package unchanged');
	if (undone.present && undone.bold) return `undo did not revert bold (${JSON.stringify(undone)})`;

	await clickButton(page, ['Redo']);
	await waitState(page, demo.format, (s) => s.present && s.bold, 'redo did not restore typed bold text');

	const mid = stateOf(demo.format, await packageBytes(page, demo.format));
	if (demo.structure === 'slide') await clickButton(page, ['New Slide', 'New slide'], 'Insert');
	else await clickButton(page, ['Insert a table', 'Table'], 'Insert');
	const structured = await waitState(
		page,
		demo.format,
		(s) => s.present && s.bold && s.count === mid.count + 1,
		demo.structure === 'slide' ? 'insert slide did not add a slide part' : 'insert table did not add w:tbl',
	);

	const downloadPromise = page.waitForEvent('download', { timeout: 20_000 });
	await clickButton(page, ['Save']);
	const download = await downloadPromise;
	const savedPath = await download.path();
	if (!savedPath) return 'save did not produce a download';
	const saved = readFileSync(savedPath);
	const onDisk = stateOf(demo.format, saved);
	if (!onDisk.present || !onDisk.bold || onDisk.count !== structured.count) {
		return `download bytes lost the edit (${JSON.stringify(onDisk)} want count ${structured.count})`;
	}

	await page.waitForFunction(() => !document.querySelector('.ox')?.hasAttribute('data-dirty'), null, { timeout: 8_000 }).catch(() => {});
	await page.locator('input[type=file]').setInputFiles({
		name: download.suggestedFilename() || `edited.${demo.format}`,
		mimeType: 'application/octet-stream',
		buffer: saved,
	});
	await page.waitForTimeout(400);
	const reopened = await waitState(
		page,
		demo.format,
		(s) => s.present && s.bold && s.count === structured.count,
		'reopen did not round-trip the saved edit',
	);
	await shot(page, join(shotDir, `${id}-after.png`));
	if (!reopened.present) return 'reopen dropped the typed text';
	return null;
}

export async function runDemoEdit(browserNames, shotDir) {
	const site = join(HERE, 'out/pages-demo');
	assemble(site);
	const server = await startBuilt(site);
	const pw = await import(pathToFileURL(join(ROOT, 'parity/visual/node_modules/playwright/index.mjs')).href);
	const rows = [];
	try {
		for (const name of browserNames) {
			const engine = pw[name];
			if (!engine) throw new Error(`playwright has no ${name}`);
			const browser = await engine.launch();
			try {
				for (const demo of DEMOS) {
					const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, acceptDownloads: true });
					const page = await context.newPage();
					page.on('dialog', (d) => d.accept());
					const caseId = `${demo.format}-${demo.mode}@${name}`;
					let msg = null;
					try {
						msg = await editDemo(page, server.url, demo, join(shotDir, name));
					} catch (e) {
						msg = String(e.message || e).split('\n')[0];
						await shot(page, join(shotDir, name, `${demo.format}-${demo.mode}-fail.png`)).catch(() => {});
					}
					rows.push({ case: caseId, ok: !msg, detail: msg || '' });
					console.log(`${msg ? 'FAIL' : 'ok'} ${caseId}${msg ? ': ' + msg : ''}`);
					await context.close();
				}
			} finally {
				await browser.close();
			}
		}
	} finally {
		server.close();
	}
	const bad = rows.filter((r) => !r.ok);
	const summary = `pages demo edit: ${rows.length - bad.length}/${rows.length} ok` + bad.map((r) => `\n  FAIL ${r.case}: ${r.detail}`).join('');
	return { ok: bad.length === 0, rows, summary };
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
	const names = (process.env.DEMO_EDIT_BROWSERS || 'chromium,webkit').split(',').map((s) => s.trim()).filter(Boolean);
	const shotDir = process.env.DEMO_EDIT_SHOTS || '/opt/cursor/artifacts/demo-edit';
	const res = await runDemoEdit(names, shotDir);
	mkdirSync(join(HERE, 'out'), { recursive: true });
	writeFileSync(join(HERE, 'out/demo-edit-summary.txt'), res.summary + '\n');
	console.log(res.summary);
	process.exit(res.ok ? 0 : 1);
}
