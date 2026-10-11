// SPDX-License-Identifier: MIT
/**
 * docx Print Layout IR (#22): LayoutResult with fake measurer vs upstream layout.
 */

import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { repoRoot } from '../corpus.ts';
import { dumpDocxViewIr } from '../oracle/docx.ts';
import { dumpDocxLayoutRust, ensureExample } from '../rust/dump.ts';
import { normalizeJson } from '../normalize.ts';

const DOCX_EXT = /\.(docx|docm|dotx|dotm)$/i;

function listDocxFiles(root: string): string[] {
	const out: string[] = [];
	const walk = (dir: string) => {
		if (!existsSync(dir)) return;
		for (const name of readdirSync(dir, { withFileTypes: true })) {
			const p = join(dir, name.name);
			if (name.isDirectory()) walk(p);
			else if (DOCX_EXT.test(name.name)) out.push(p);
		}
	};
	walk(join(root, 'fixtures/real/docx'));
	return out.filter((p) => DOCX_EXT.test(p)).sort();
}

async function runDocxLayoutCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	const notes: string[] = [];
	ensureExample('docx_layout_dump', 'nitro-xml-docx');
	spawnSync('python3', ['tools/model-compare/gates/make_opc_fixtures.py'], {
		cwd: root,
		encoding: 'utf8',
	});

	let n = 0;
	let bad = 0;
	for (const f of listDocxFiles(root)) {
		if (!existsSync(f)) continue;
		n++;
		let ref: unknown;
		try {
			ref = await dumpDocxViewIr(f);
		} catch (e) {
			bad++;
			if (notes.length < 20) notes.push(`oracle failed on ${f}: ${e}`);
			continue;
		}
		const got = dumpDocxLayoutRust(f);
		if (got.status === 101 || got.status === 124 || (got.status !== null && got.status > 128)) {
			bad++;
			if (notes.length < 20) notes.push(`panic/timeout/crash rc=${got.status} on ${f}`);
			continue;
		}
		if (!got.ok) {
			bad++;
			if (notes.length < 20) notes.push(`rust dump failed on ${f}: ${JSON.stringify(got.json)}`);
			continue;
		}
		if (normalizeJson(ref) !== normalizeJson(got.json)) {
			bad++;
			if (notes.length < 20) notes.push(`layout IR differs on ${f}`);
		}
	}
	notes.push(`docx layout differential: ${n - bad}/${n} files match upstream`);
	return { compared: n, mismatched: bad, notes };
}

export const docxLayoutComparison: Comparison = {
	id: 'docx-layout',
	label: 'docx Print Layout IR (#22)',
	ready: false,
	todo: 'Flip ready when corpus is green (20/30 real/docx match; OPC minimal excluded).',
	async oracle(fixturePath: string) {
		return dumpDocxViewIr(fixturePath);
	},
	async rust(fixturePath: string) {
		const { json } = dumpDocxLayoutRust(fixturePath);
		return json;
	},
	selectFixtures(_manifest: Manifest) {
		return listDocxFiles(repoRoot());
	},
	runCorpus: runDocxLayoutCorpus,
};
