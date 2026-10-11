// SPDX-License-Identifier: MIT
/** XML-layer comparison folded from tools/model-compare/gates/issue-12.sh. */

import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { repoRoot } from '../corpus.ts';
import { dumpXmlTree } from '../oracle/xml.ts';
import { dumpXmlFileRust, dumpXmlListRust, ensureExample } from '../rust/dump.ts';
import { normalizeJson } from '../normalize.ts';
import { upstreamDir } from '../paths.ts';

async function runXmlCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	ensureExample('xml_dump');
	const tmp = mkdtempSync(join(tmpdir(), 'parity-xml-'));
	const notes: string[] = [];
	try {
		const gen = spawnSync('python3', [join(root, 'tools/model-compare/gates/make_xml_cases.py'), tmp], {
			cwd: root,
			encoding: 'utf8',
		});
		if (gen.status !== 0) {
			return { compared: 0, mismatched: 1, notes: [`make_xml_cases failed: ${gen.stderr}`] };
		}
		const listPath = join(tmp, 'list.txt');
		const refScript = join(root, 'tools/model-compare/gates/ref/xml_ref.ts');
		const ref = spawnSync('bun', [refScript, listPath], {
			cwd: root,
			encoding: 'utf8',
			env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
			maxBuffer: 64 * 1024 * 1024,
		});
		if (ref.status !== 0) {
			return { compared: 0, mismatched: 1, notes: [`xml_ref failed rc=${ref.status}`] };
		}
		writeFileSync(join(tmp, 'ref.jsonl'), ref.stdout);
		const gotRows = dumpXmlListRust(listPath);
		writeFileSync(
			join(tmp, 'got.jsonl'),
			gotRows.map((r) => JSON.stringify(r)).join('\n') + '\n',
		);

		const refLines = ref.stdout.split('\n').filter(Boolean);
		const gotByFile = new Map<string, unknown>();
		for (const row of gotRows) {
			const o = row as { file?: string };
			if (o.file) gotByFile.set(o.file, row);
		}
		let bad = 0;
		let compared = 0;
		for (const line of refLines) {
			const r = JSON.parse(line) as { file: string; tree?: unknown; error?: string };
			compared++;
			const g = gotByFile.get(r.file) as { tree?: unknown; error?: string } | undefined;
			if (!g) {
				bad++;
				notes.push(`missing output for ${r.file}`);
				continue;
			}
			const refErr = 'error' in r;
			const gotErr = 'error' in g;
			if (refErr !== gotErr) {
				bad++;
				notes.push(`accept/reject mismatch on ${r.file}`);
				continue;
			}
			if (r.tree !== undefined && normalizeJson(r.tree) !== normalizeJson(g.tree)) {
				bad++;
				notes.push(`tree differs on ${r.file}`);
			}
		}
		notes.push(`differential: ${compared - bad}/${compared} XML inputs match upstream`);
		return { compared, mismatched: bad, notes };
	} finally {
		rmSync(tmp, { recursive: true, force: true });
	}
}

export const xmlComparison: Comparison = {
	id: 'xml',
	label: 'XML streamer (issue #12)',
	ready: true,
	async oracle(fixturePath: string) {
		return dumpXmlTree(fixturePath);
	},
	async rust(fixturePath: string) {
		return dumpXmlFileRust(fixturePath);
	},
	selectFixtures(_manifest: Manifest) {
		// Corpus is generated (real zip XML parts + crafted + fixtures/xml).
		return [];
	},
	runCorpus: runXmlCorpus,
};
