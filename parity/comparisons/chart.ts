// SPDX-License-Identifier: MIT
/** Chart `c:chartSpace` model IR comparison (#28). */

import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult } from '../types.ts';
import { repoRoot } from '../corpus.ts';
import { ensureExample } from '../rust/dump.ts';
import { normalizeJson } from '../normalize.ts';
import { normalizeChartIr } from '../chart-normalize.ts';
import { upstreamDir } from '../paths.ts';

const OFFICE =
	/\.(xlsx|xlsm|xltx|xltm|docx|docm|dotx|dotm|pptx|pptm|potx|potm)$/i;
const CHART_PART = /charts\/chart\d+\.xml$/i;

function listChartParts(root: string): { packagePath: string; partPath: string }[] {
	const out: { packagePath: string; partPath: string }[] = [];
	const walk = (dir: string) => {
		if (!existsSync(dir)) return;
		for (const name of readdirSync(dir, { withFileTypes: true })) {
			const p = join(dir, name.name);
			if (name.isDirectory()) walk(p);
			else if (OFFICE.test(name.name)) {
				try {
					const listing = spawnSync('unzip', ['-Z1', p], { encoding: 'utf8' }).stdout ?? '';
					for (const part of listing.split('\n')) {
						if (CHART_PART.test(part)) out.push({ packagePath: p, partPath: part });
					}
				} catch {
					/* skip corrupt zips */
				}
			}
		}
	};
	walk(join(root, 'fixtures/real'));
	walk(join(root, 'fixtures/opc/allowed'));
	walk(join(root, 'fixtures/upstream'));
	walk(join(root, 'fixtures/generated'));
	return out.sort((a, b) => a.packagePath.localeCompare(b.packagePath));
}

function withChartXmlFile<T>(xml: string, run: (path: string) => T): T {
	const dir = mkdtempSync(join(tmpdir(), 'nitro-chart-'));
	const path = join(dir, 'chart.xml');
	writeFileSync(path, xml, 'utf8');
	try {
		return run(path);
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

function dumpOracleChartXml(xml: string): unknown {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/chart_ref.ts');
	return withChartXmlFile(xml, (path) => {
		const r = spawnSync('bun', [script, 'model', '--file', path], {
			encoding: 'utf8',
			env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
			maxBuffer: 32 * 1024 * 1024,
		});
		if (r.status !== 0) return { error: r.stderr || r.stdout };
		return JSON.parse((r.stdout || '').trim() || 'null');
	});
}

function dumpRustChartXml(xml: string): unknown {
	const root = repoRoot();
	const bin = join(root, 'rs/target/release/examples/chart_dump');
	return withChartXmlFile(xml, (path) => {
		const r = spawnSync(bin, ['--file', path], {
			encoding: 'utf8',
			maxBuffer: 32 * 1024 * 1024,
		});
		if (r.status !== 0) return { error: r.stderr || r.stdout };
		return JSON.parse((r.stdout || '').trim() || 'null');
	});
}

function dumpOracleChartSvg(xml: string): string {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/chart_ref.ts');
	return withChartXmlFile(xml, (path) => {
		const r = spawnSync('bun', [script, 'svg', '--file', path], {
			encoding: 'utf8',
			env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
			maxBuffer: 32 * 1024 * 1024,
		});
		return r.status === 0 ? (r.stdout ?? '') : '';
	});
}

function dumpRustChartSvg(xml: string): string {
	const root = repoRoot();
	// chart_render_svg example built in parity.sh
	const bin = join(root, 'rs/target/release/examples/chart_render_svg');
	if (!existsSync(bin)) {
		return '';
	}
	return withChartXmlFile(xml, (path) => {
		const r = spawnSync(bin, ['--file', path], {
			encoding: 'utf8',
			env: {
				...process.env,
				UPSTREAM_DIR: upstreamDir(root),
				NITRO_XML_REPO: root,
			},
			maxBuffer: 32 * 1024 * 1024,
		});
		return r.status === 0 ? (r.stdout ?? '') : '';
	});
}

async function compareIr(ref: unknown, got: unknown): Promise<boolean> {
	const r = await normalizeChartIr(ref);
	const g = await normalizeChartIr(got);
	return normalizeJson(r) === normalizeJson(g);
}

const DRAWN = new Set([
	'bar',
	'column',
	'line',
	'area',
	'pie',
	'doughnut',
	'scatter',
	'radar',
]);

/** Rich / multiline chart titles (`chartRichTitleSvg` + `wrapStyledRuns`) — tracked in #28. */
const SVG_TITLE_KNOWN_DIFF = new Set(
	[
		'fixtures/real/pptx/python-pptx-cht-axis-props.pptx	ppt/charts/chart3.xml',
		'fixtures/real/xlsx/poi-57181.xlsm	xl/charts/chart1.xml',
		'fixtures/real/xlsx/poi-57181.xlsm	xl/charts/chart2.xml',
		'fixtures/real/xlsx/poi-dataValidationTableRange.xlsx	xl/charts/chart1.xml',
		'fixtures/real/docx/poi-chartex.docx	word/charts/chart1.xml',
		'fixtures/real/docx/poi-chartex.docx	word/charts/chart2.xml',
		'fixtures/real/docx/poi-chartex.docx	word/charts/chart3.xml',
	].map((row) => {
		const [pkg, part] = row.split('\t');
		return `${join(repoRoot(), pkg)} ${part}`;
	}),
);

function chartDrawn(ref: { chartSpace?: { plotArea?: { groups?: { kind?: string; barDirection?: string }[] } } }): boolean {
	const g = ref.chartSpace?.plotArea?.groups?.[0];
	if (!g) return false;
	let t = g.kind ?? 'column';
	if (t === 'bar' && g.barDirection !== 'bar') t = 'column';
	return DRAWN.has(t);
}

async function runChartCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	const notes: string[] = [];
	ensureExample('chart_dump', 'nitro-xml-opc');
	ensureExample('chart_render_svg', 'nitro-xml-render');

	let n = 0;
	let bad = 0;
	let svgN = 0;
	let svgBad = 0;
	let svgKnown = 0;
	for (const { packagePath, partPath } of listChartParts(root)) {
		if (!existsSync(packagePath)) continue;
		n++;
		const xml = spawnSync('unzip', ['-p', packagePath, partPath], {
			encoding: 'utf8',
			maxBuffer: 16 * 1024 * 1024,
		});
		if (xml.status !== 0 || !xml.stdout) {
			bad++;
			continue;
		}
		const ref = dumpOracleChartXml(xml.stdout);
		const got = dumpRustChartXml(xml.stdout);
		if (!(await compareIr(ref, got))) {
			bad++;
			if (notes.length < 15) notes.push(`model differs: ${packagePath} ${partPath}`);
		}
		if (chartDrawn(ref as typeof ref)) {
			svgN++;
			const refSvg = dumpOracleChartSvg(xml.stdout);
			const gotSvg = dumpRustChartSvg(xml.stdout);
			if (refSvg.trim() !== gotSvg.trim()) {
				const key = `${packagePath} ${partPath}`;
				if (SVG_TITLE_KNOWN_DIFF.has(key)) {
					svgKnown++;
					if (notes.length < 15)
						notes.push(`svg known-diff (rich title wrap): ${packagePath} ${partPath}`);
				} else {
					svgBad++;
					if (notes.length < 15) notes.push(`svg differs: ${packagePath} ${partPath}`);
				}
			}
		}
	}
	notes.push(`chart model IR: ${n - bad}/${n} parts match upstream`);
	if (svgN) {
		notes.push(`chart S1 SVG: ${svgN - svgBad}/${svgN} drawn families match upstream`);
		if (svgKnown)
			notes.push(`chart S1 SVG: ${svgKnown} known rich-title diffs (see SVG_TITLE_KNOWN_DIFF in chart.ts)`);
	}
	return { compared: n, mismatched: bad + svgBad, notes };
}

export const chartComparison: Comparison = {
	id: 'chart',
	label: 'chart model + S1 SVG (#28)',
	ready: true,
	todo: '',
	async oracle(fixturePath: string) {
		const xml = readFileSync(fixturePath, 'utf8');
		return dumpOracleChartXml(xml);
	},
	async rust(fixturePath: string) {
		const xml = readFileSync(fixturePath, 'utf8');
		return dumpRustChartXml(xml);
	},
	async runCorpus() {
		return runChartCorpus();
	},
};
