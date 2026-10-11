// SPDX-License-Identifier: MIT
/** Chart `c:chartSpace` model IR comparison (#28). */

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
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

function dumpOracleChartXml(xml: string): unknown {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/chart_ref.ts');
	const r = spawnSync('bun', [script, 'model', xml], {
		encoding: 'utf8',
		env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
		maxBuffer: 32 * 1024 * 1024,
	});
	if (r.status !== 0) return { error: r.stderr || r.stdout };
	return JSON.parse((r.stdout || '').trim() || 'null');
}

function dumpRustChartXml(xml: string): unknown {
	const root = repoRoot();
	const bin = join(root, 'rs/target/release/examples/chart_dump');
	const r = spawnSync(bin, [xml], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
	if (r.status !== 0) return { error: r.stderr || r.stdout };
	return JSON.parse((r.stdout || '').trim() || 'null');
}

async function compareIr(ref: unknown, got: unknown): Promise<boolean> {
	const r = await normalizeChartIr(ref);
	const g = await normalizeChartIr(got);
	return normalizeJson(r) === normalizeJson(g);
}

async function runChartCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	const notes: string[] = [];
	ensureExample('chart_dump', 'nitro-xml-opc');

	let n = 0;
	let bad = 0;
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
	}
	notes.push(`chart model IR: ${n - bad}/${n} parts match upstream`);
	return { compared: n, mismatched: bad, notes };
}

export const chartComparison: Comparison = {
	id: 'chart',
	label: 'chart model + S1 SVG (#28)',
	ready: false,
	todo: 'chart model IR 108/142 corpus match; extLst xmlns + chartex + S1 SVG port (#28)',
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
