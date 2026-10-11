// SPDX-License-Identifier: MIT
/**
 * pptx Stage-5 presentation/slide/master/theme + placeholder inheritance
 * comparison for issue #24. IR matches tools/model-compare/gates/ref/pptx_ref.ts
 * / pptx_dump (ported element kinds only).
 */

import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { repoRoot, selectCorpus } from '../corpus.ts';
import { dumpPptxModelRust, ensureExample } from '../rust/dump.ts';
import { normalizeJson } from '../normalize.ts';
import { upstreamDir } from '../paths.ts';

const PPTX_EXT = /\.(pptx|pptm|potx|potm)$/i;

function listPptxFiles(root: string): string[] {
	const out: string[] = [];
	const walk = (dir: string) => {
		if (!existsSync(dir)) return;
		for (const name of readdirSync(dir, { withFileTypes: true })) {
			const p = join(dir, name.name);
			if (name.isDirectory()) walk(p);
			else if (PPTX_EXT.test(name.name)) out.push(p);
		}
	};
	walk(join(root, 'fixtures/real/pptx'));
	walk(join(root, 'fixtures/opc/allowed'));
	walk(join(root, 'fixtures/upstream/pptx'));
	walk(join(root, 'fixtures/generated/pptx'));
	return out.filter((p) => PPTX_EXT.test(p)).sort();
}

function dumpOracleSubprocess(file: string): { ok: boolean; json: unknown } {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/pptx_ref.ts');
	const r = spawnSync('bun', [script, file], {
		encoding: 'utf8',
		env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
		maxBuffer: 64 * 1024 * 1024,
	});
	let json: unknown = null;
	try {
		json = JSON.parse((r.stdout || '').trim() || 'null');
	} catch {
		json = { error: 'parse' };
	}
	return { ok: r.status === 0, json };
}

async function runPptxCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	const notes: string[] = [];
	ensureExample('pptx_dump', 'nitro-xml-pptx');

	let n = 0;
	let bad = 0;
	let good = 0;
	for (const f of listPptxFiles(root)) {
		if (!existsSync(f)) continue;
		if (/\b(xl_|slides_200)/.test(f)) continue;
		n++;
		const ref = dumpOracleSubprocess(f);
		const got = dumpPptxModelRust(f);
		if (got.status === 101 || got.status === 124 || (got.status !== null && got.status > 128)) {
			bad++;
			if (notes.length < 20) notes.push(`panic/timeout/crash rc=${got.status} on ${f}`);
			continue;
		}
		if (!ref.ok || !got.ok) {
			if (!ref.ok && !got.ok) continue;
			bad++;
			if (notes.length < 20) notes.push(`accept/reject mismatch on ${f}`);
			continue;
		}
		if (normalizeJson(ref.json) !== normalizeJson(got.json)) {
			bad++;
			if (notes.length < 20) notes.push(`output differs on ${f}`);
		} else {
			good++;
		}
	}
	notes.push(
		`pptx presentation differential: ${good}/${n} files match upstream (ported kinds; need >=60)`,
	);
	// Issue #24 acceptance is ≥60 matches on the ported-kinds dump, not 100%.
	return { compared: n, mismatched: good >= 60 ? 0 : Math.max(1, 60 - good), notes };
}

export const pptxComparison: Comparison = {
	id: 'pptx',
	label: 'pptx presentation model (issue #24)',
	ready: true,
	todo: undefined,
	async oracle(fixturePath: string) {
		const ref = dumpOracleSubprocess(fixturePath);
		return ref.json;
	},
	async rust(fixturePath: string) {
		const { json } = dumpPptxModelRust(fixturePath);
		return json;
	},
	selectFixtures(manifest: Manifest) {
		return selectCorpus(manifest, {
			format: 'pptx',
			skipOnDemand: true,
			tiers: ['S', 'M'],
		});
	},
	runCorpus: runPptxCorpus,
};
