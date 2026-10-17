// SPDX-License-Identifier: MIT
/**
 * Build per-case payloads for the visual host (TS oracle + Rust where ready).
 */

import {
	mkdirSync,
	writeFileSync,
	existsSync,
	readFileSync,
	copyFileSync,
	unlinkSync,
} from 'node:fs';
import { join, dirname } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { readyCases } from './registry.mjs';

const VISUAL = dirname(fileURLToPath(import.meta.url));
const ROOT = join(VISUAL, '../..');
const OUT = join(VISUAL, 'out/payloads');

/**
 * Excel chart series formulas often use `[0]!name` for a defined name in the
 * same workbook. Upstream calc treats any book prefix as external and returns
 * `#NAME?`, which blocks chartSummaryView's cache fallback and leaves the
 * soft viewer oracle empty (poi-dataValidationTableRange, #150). Strip the
 * self-workbook marker in the disposable submodule copy before bundling.
 */
function patchUpstreamChartSelfBookRefs() {
	const target = join(ROOT, 'upstream/src/core/xlsx/layout/ref-evaluator.ts');
	if (!existsSync(target)) {
		throw new Error(`visual prepare: missing ${target}`);
	}
	const src = readFileSync(target, 'utf8');
	// Idempotent: look for the self-workbook strip already present in the TS source.
	if (src.includes(".replace(/^\\[0]!/, '')")) {
		return false;
	}
	const from = "const text = ref.trim().replace(/^=/, '');";
	const to = "const text = ref.trim().replace(/^=/, '').replace(/^\\[0]!/, '');";
	if (!src.includes(from)) {
		throw new Error('visual prepare: ref-evaluator self-book patch anchor not found');
	}
	writeFileSync(target, src.replace(from, to));
	return true;
}

/** Liberation Sans faces used as the pinned visual-gate font family (#150). */
const PINNED_FONTS = [
	'LiberationSans-Regular.ttf',
	'LiberationSans-Bold.ttf',
	'LiberationSans-Italic.ttf',
	'LiberationSans-BoldItalic.ttf',
];

/** CJK fallback pin for pptx soft-band decks (bug64693 YaHei / 黑体). */
const PINNED_CJK_FONT = {
	src: '/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf',
	dest: 'DroidSansFallbackFull.ttf',
};

function preparePinnedFonts() {
	const destDir = join(VISUAL, 'out/fonts');
	mkdirSync(destDir, { recursive: true });
	const srcDir = '/usr/share/fonts/truetype/liberation';
	for (const name of PINNED_FONTS) {
		const src = join(srcDir, name);
		const dest = join(destDir, name);
		if (!existsSync(src)) {
			throw new Error(`visual fonts: missing ${src} (install fonts-liberation)`);
		}
		if (!existsSync(dest)) copyFileSync(src, dest);
	}
	const cjkDest = join(destDir, PINNED_CJK_FONT.dest);
	if (existsSync(PINNED_CJK_FONT.src) && !existsSync(cjkDest)) {
		copyFileSync(PINNED_CJK_FONT.src, cjkDest);
	}
	// CSS2-compatible mirror for #121 fontSource. Served at /out/fonts/css2
	// (query string ignored); every requested family maps to Liberation Sans files.
	const faces = [
		'Calibri',
		'Carlito',
		'Segoe UI',
		'Arial',
		'Liberation Sans',
		'Helvetica',
		'Verdana',
		'DejaVu Sans',
		'Lucida Grande',
		'Arial Black',
		'Informal Roman',
		'Script MT Bold',
	];
	const css2 = [
		'/* SPDX-License-Identifier: MIT — visual-gate local font mirror (#150) */',
		...PINNED_FONTS.flatMap((file) => {
			const italic = /Italic/.test(file);
			const bold = /Bold/.test(file);
			const weight = bold ? '700' : '400';
			const style = italic ? 'italic' : 'normal';
			return faces.map(
				(fam) =>
					`@font-face{font-family:"${fam}";font-style:${style};font-weight:${weight};src:url("/out/fonts/${file}") format("truetype")}`,
			);
		}),
		'',
	].join('\n');
	// Bare path matches Google CSS2 base shape: `{base}?family=...&display=swap`.
	writeFileSync(join(VISUAL, 'out/fonts/css2'), css2);
	writeFileSync(join(VISUAL, 'out/fonts/css2.css'), css2);
}

function upstreamDir() {
	return process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
}

function ensureReleaseExample(name, crate) {
	const bin = join(ROOT, 'rs/target/release/examples', name);
	if (existsSync(bin)) return bin;
	const r = spawnSync(
		'cargo',
		['build', '--release', '-q', '-p', crate, '--example', name],
		{ cwd: join(ROOT, 'rs'), encoding: 'utf8' },
	);
	if (r.status !== 0) {
		throw new Error(`build ${name}: ${r.stderr || r.stdout}`);
	}
	return bin;
}

function unzipPart(pkg, part) {
	const r = spawnSync('unzip', ['-p', pkg, part], {
		encoding: 'buffer',
		maxBuffer: 32 * 1024 * 1024,
	});
	if (r.status !== 0) throw new Error(`unzip ${pkg} ${part}`);
	return r.stdout;
}

function prepareChart(c) {
	const pkg = join(ROOT, c.fixture);
	const xml = unzipPart(pkg, c.part);
	const tmp = join(OUT, `${c.id}.xml`);
	writeFileSync(tmp, xml);

	const env = { ...process.env, UPSTREAM_DIR: upstreamDir() };
	const ts = spawnSync(
		'bun',
		[join(ROOT, 'tools/model-compare/gates/ref/chart_ref.ts'), 'svg', '--file', tmp],
		{ encoding: 'utf8', env, maxBuffer: 32 * 1024 * 1024 },
	);
	if (ts.status !== 0) throw new Error(`chart TS svg: ${ts.stderr || ts.stdout}`);
	writeFileSync(join(OUT, `${c.id}-ts.svg`), ts.stdout);

	if (c.rust) {
		const bin = ensureReleaseExample('chart_render_svg', 'nitro-xml-render');
		const rust = spawnSync(bin, [], { input: xml, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
		if (rust.status !== 0) throw new Error(`chart rust svg: ${rust.stderr}`);
		writeFileSync(join(OUT, `${c.id}-rust.svg`), rust.stdout);
	}
}

async function prepareXlsxViewIr(c) {
	const file = join(ROOT, c.fixture);
	const { dumpXlsxViewIr } = await import('../oracle/xlsx.ts');
	const tsIr = await dumpXlsxViewIr(file);
	writeFileSync(join(OUT, `${c.id}-ts.json`), JSON.stringify(tsIr));

	if (c.rust) {
		const bin = ensureReleaseExample('xlsx_view_ir_dump', 'nitro-xml-xlsx');
		const rust = spawnSync(bin, [file], {
			encoding: 'utf8',
			maxBuffer: 64 * 1024 * 1024,
		});
		if (rust.status !== 0) throw new Error(`xlsx view-IR rust: ${rust.stderr || rust.stdout}`);
		// Normalize by re-parsing so key order matches host JSON.parse.
		const ir = JSON.parse(rust.stdout);
		writeFileSync(join(OUT, `${c.id}-rust.json`), JSON.stringify(ir));
	}
}

function ensureUpstreamBuilt() {
	const up = upstreamDir();
	const coreXlsx = join(up, 'src/core/dist/xlsx');
	const corePptx = join(up, 'src/core/dist/pptx');
	const uiXlsx = join(up, 'src/ui/dist/xlsx.js');
	const uiPptxDom = join(up, 'src/ui/dist/pptx/dom/index.mjs');
	if (!existsSync(coreXlsx) || !existsSync(corePptx)) {
		console.log('visual prepare: building upstream ooxml-core');
		const r = spawnSync('bun', ['run', '--cwd', 'src/core', 'build'], {
			cwd: up,
			encoding: 'utf8',
			maxBuffer: 16 * 1024 * 1024,
		});
		if (r.status !== 0) throw new Error(`ooxml-core build: ${r.stderr || r.stdout}`);
	}
	if (!existsSync(uiXlsx) || !existsSync(uiPptxDom)) {
		console.log('visual prepare: building upstream ooxml-ui');
		const r = spawnSync('bun', ['run', '--cwd', 'src/ui', 'build'], {
			cwd: up,
			encoding: 'utf8',
			maxBuffer: 16 * 1024 * 1024,
		});
		if (r.status !== 0) throw new Error(`ooxml-ui build: ${r.stderr || r.stdout}`);
	}
}

function prepareXlsxViewerBundle() {
	ensureUpstreamBuilt();
	const bundleDir = join(VISUAL, 'out/bundles');
	mkdirSync(bundleDir, { recursive: true });
	const entry = join(VISUAL, 'hosts/xlsx-editor-entry.mjs');
	const outFile = join(bundleDir, 'xlsx-editor.js');
	const stamp = join(bundleDir, '.chart-self-book-v1');
	const patched = patchUpstreamChartSelfBookRefs();
	if (patched || !existsSync(stamp)) {
		try {
			unlinkSync(outFile);
		} catch {
			/* missing is fine */
		}
	}
	// Re-bundle when missing or entry newer than bundle.
	if (existsSync(outFile)) {
		const entryStat = spawnSync('stat', ['-c', '%Y', entry], { encoding: 'utf8' });
		const outStat = spawnSync('stat', ['-c', '%Y', outFile], { encoding: 'utf8' });
		if (
			entryStat.status === 0 &&
			outStat.status === 0 &&
			Number(outStat.stdout.trim()) >= Number(entryStat.stdout.trim())
		) {
			writeFileSync(stamp, '1\n');
			return;
		}
	}
	console.log('visual prepare: bundling upstream xlsx-editor for browser host');
	const r = spawnSync(
		'bun',
		['build', entry, '--outfile', outFile, '--format', 'esm', '--target', 'browser'],
		{ cwd: ROOT, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
	);
	if (r.status !== 0) throw new Error(`xlsx-editor bundle: ${r.stderr || r.stdout}`);
	writeFileSync(stamp, '1\n');
}

async function prepareDocxLayoutIr(c) {
	const file = join(ROOT, c.fixture);
	const { dumpDocxViewIr } = await import('../oracle/docx.ts');
	const tsIr = await dumpDocxViewIr(file);
	writeFileSync(join(OUT, `${c.id}-ts.json`), JSON.stringify(tsIr));

	if (c.rust) {
		const bin = ensureReleaseExample('docx_layout_dump', 'nitro-xml-docx');
		const rust = spawnSync(bin, [file], {
			encoding: 'utf8',
			maxBuffer: 64 * 1024 * 1024,
		});
		if (rust.status !== 0) throw new Error(`docx layout rust: ${rust.stderr || rust.stdout}`);
		const ir = JSON.parse(rust.stdout);
		writeFileSync(join(OUT, `${c.id}-rust.json`), JSON.stringify(ir));
	}
}

function prepareXlsxViewer(c) {
	const file = join(ROOT, c.fixture);
	if (!existsSync(file)) throw new Error(`missing fixture ${c.fixture}`);
	prepareXlsxViewerBundle();
	// Fixture is served live via /repo/; no IR payload.
}

function prepareXlsxEditChrome(c) {
	const file = join(ROOT, c.fixture);
	if (!existsSync(file)) throw new Error(`missing fixture ${c.fixture}`);
	prepareXlsxViewerBundle();
}

function prepareDocxEditorBundle() {
	ensureUpstreamBuilt();
	const bundleDir = join(VISUAL, 'out/bundles');
	mkdirSync(bundleDir, { recursive: true });
	const entry = join(VISUAL, 'hosts/docx-editor-entry.mjs');
	const outFile = join(bundleDir, 'docx-editor.js');
	if (existsSync(outFile)) {
		const entryStat = spawnSync('stat', ['-c', '%Y', entry], { encoding: 'utf8' });
		const outStat = spawnSync('stat', ['-c', '%Y', outFile], { encoding: 'utf8' });
		if (
			entryStat.status === 0 &&
			outStat.status === 0 &&
			Number(outStat.stdout.trim()) >= Number(entryStat.stdout.trim())
		) {
			return;
		}
	}
	console.log('visual prepare: bundling upstream docx-editor for browser host');
	const r = spawnSync(
		'bun',
		['build', entry, '--outfile', outFile, '--format', 'esm', '--target', 'browser'],
		{ cwd: ROOT, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 },
	);
	if (r.status !== 0) throw new Error(`docx-editor bundle: ${r.stderr || r.stdout}`);
}

function prepareDocxEditChrome(c) {
	const file = join(ROOT, c.fixture);
	if (!existsSync(file)) throw new Error(`missing fixture ${c.fixture}`);
	prepareDocxEditorBundle();
}

function preparePptxSlideBundle() {
	ensureUpstreamBuilt();
	const bundleDir = join(VISUAL, 'out/bundles');
	mkdirSync(bundleDir, { recursive: true });
	const entry = join(VISUAL, 'hosts/pptx-slide-entry.mjs');
	const outFile = join(bundleDir, 'pptx-slide.js');
	if (existsSync(outFile)) {
		const entryStat = spawnSync('stat', ['-c', '%Y', entry], { encoding: 'utf8' });
		const outStat = spawnSync('stat', ['-c', '%Y', outFile], { encoding: 'utf8' });
		if (
			entryStat.status === 0 &&
			outStat.status === 0 &&
			Number(outStat.stdout.trim()) >= Number(entryStat.stdout.trim())
		) {
			return;
		}
	}
	console.log('visual prepare: bundling upstream pptx slide stage for browser host');
	const r = spawnSync(
		'bun',
		['build', entry, '--outfile', outFile, '--format', 'esm', '--target', 'browser'],
		{ cwd: ROOT, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 },
	);
	if (r.status !== 0) throw new Error(`pptx-slide bundle: ${r.stderr || r.stdout}`);
}

function preparePptxViewer(c) {
	const file = join(ROOT, c.fixture);
	if (!existsSync(file)) throw new Error(`missing fixture ${c.fixture}`);
	preparePptxSlideBundle();
}

function writeDocxPrintCssModule() {
	const cssPath = join(upstreamDir(), 'src/ui/src/docx/styles/print.css');
	if (!existsSync(cssPath)) throw new Error(`missing upstream print.css at ${cssPath}`);
	const css = readFileSync(cssPath, 'utf8');
	const genDir = join(VISUAL, 'generated');
	mkdirSync(genDir, { recursive: true });
	const mod = join(genDir, 'docx-print-css.mjs');
	writeFileSync(
		mod,
		`// SPDX-License-Identifier: MIT\n/** Generated by prepare.mjs — upstream print.css for #23 visual host. */\nexport const printCss = ${JSON.stringify(css)};\n`,
	);
}

function prepareDocxViewerBundle() {
	writeDocxPrintCssModule();
	const bundleDir = join(VISUAL, 'out/bundles');
	mkdirSync(bundleDir, { recursive: true });
	const entry = join(VISUAL, 'hosts/docx-print-entry.mjs');
	const outFile = join(bundleDir, 'docx-print.js');
	if (existsSync(outFile)) {
		const entryStat = spawnSync('stat', ['-c', '%Y', entry], { encoding: 'utf8' });
		const outStat = spawnSync('stat', ['-c', '%Y', outFile], { encoding: 'utf8' });
		if (
			entryStat.status === 0 &&
			outStat.status === 0 &&
			Number(outStat.stdout.trim()) >= Number(entryStat.stdout.trim())
		) {
			return;
		}
	}
	console.log('visual prepare: bundling upstream docx Print Layout for browser host');
	const r = spawnSync(
		'bun',
		['build', entry, '--outfile', outFile, '--format', 'esm', '--target', 'browser'],
		{ cwd: ROOT, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 },
	);
	if (r.status !== 0) throw new Error(`docx-print bundle: ${r.stderr || r.stdout}`);
}

function prepareDocxViewer(c) {
	const file = join(ROOT, c.fixture);
	if (!existsSync(file)) throw new Error(`missing fixture ${c.fixture}`);
	prepareDocxViewerBundle();
}

export async function prepareAll() {
	mkdirSync(OUT, { recursive: true });
	preparePinnedFonts();
	let pptxBundleDone = false;
	let docxBundleDone = false;
	let docxEditorBundleDone = false;
	for (const c of readyCases()) {
		if (c.kind === 'chart-svg') prepareChart(c);
		else if (c.kind === 'xlsx-view-ir') await prepareXlsxViewIr(c);
		else if (c.kind === 'docx-layout-ir') await prepareDocxLayoutIr(c);
		else if (c.kind === 'xlsx-viewer') prepareXlsxViewer(c);
		else if (c.kind === 'xlsx-edit-chrome') prepareXlsxEditChrome(c);
		else if (c.kind === 'pptx-viewer') {
			if (!pptxBundleDone) {
				preparePptxSlideBundle();
				pptxBundleDone = true;
			}
			preparePptxViewer(c);
		} else if (c.kind === 'docx-viewer') {
			if (!docxBundleDone) {
				prepareDocxViewerBundle();
				docxBundleDone = true;
			}
			prepareDocxViewer(c);
		} else if (c.kind === 'docx-edit-chrome') {
			if (!docxEditorBundleDone) {
				prepareDocxEditorBundle();
				docxEditorBundleDone = true;
			}
			prepareDocxEditChrome(c);
		} else throw new Error(`prepare: unknown kind for ${c.id}`);
		console.log(`visual prepare: ${c.id}`);
	}
}

if (import.meta.main) {
	await prepareAll();
}
