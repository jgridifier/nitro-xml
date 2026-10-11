// SPDX-License-Identifier: MIT
/** OPC-layer comparison folded from tools/model-compare/gates/issue-13.sh. */

import { mkdtempSync, rmSync, writeFileSync, readFileSync, readdirSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { repoRoot, selectCorpus } from '../corpus.ts';
import { dumpOpcPackageInline } from '../oracle/opc.ts';
import { dumpOpcHrefRust, dumpOpcPackageRust, ensureExample } from '../rust/dump.ts';
import { normalizeJson } from '../normalize.ts';
import { upstreamDir } from '../paths.ts';

const OFFICE_GLOB_HINT =
	/\.(xlsx|xlsm|xltx|xltm|docx|docm|dotx|dotm|pptx|pptm|potx|potm)$/i;

function listOfficeFiles(root: string): string[] {
	// Same corpus as issue-13: real office files + fixtures/opc/allowed.
	const out: string[] = [];
	const walk = (dir: string) => {
		if (!existsSync(dir)) return;
		for (const name of readdirSync(dir, { withFileTypes: true })) {
			const p = join(dir, name.name);
			if (name.isDirectory()) walk(p);
			else if (OFFICE_GLOB_HINT.test(name.name)) out.push(p);
		}
	};
	walk(join(root, 'fixtures/real'));
	walk(join(root, 'fixtures/opc/allowed'));
	return out.sort();
}

function canon(v: unknown): string {
	return normalizeJson(v);
}

async function runOpcCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	const notes: string[] = [];
	ensureExample('opc_dump');

	spawnSync('python3', [join(root, 'tools/model-compare/gates/make_opc_fixtures.py')], {
		cwd: root,
		encoding: 'utf8',
	});
	spawnSync('python3', [join(root, 'tools/model-compare/gates/make_safe_href_corpus.py')], {
		cwd: root,
		encoding: 'utf8',
	});

	let n = 0;
	let bad = 0;
	const files = listOfficeFiles(root);
	for (const f of files) {
		// Skip on-demand L/XL generated files that may be absent (same as verify --skip-on-demand).
		if (!existsSync(f)) continue;
		if (/\b(xl_|cells_1m|pages_500|slides_200)/.test(f)) continue;
		n++;
		const refScript = join(root, 'tools/model-compare/gates/ref/opc_ref.ts');
		const refProc = spawnSync('bun', [refScript, 'package', f], {
			encoding: 'utf8',
			env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
			maxBuffer: 32 * 1024 * 1024,
		});
		const got = dumpOpcPackageRust(f);
		const rcRef = refProc.status ?? 1;
		const rcGot = got.ok ? 0 : 2;
		if (rcGot === 101) {
			bad++;
			notes.push(`panic on ${f}`);
			continue;
		}
		if (rcRef !== 0 || !got.ok) {
			if (rcRef !== 0 && !got.ok) continue;
			bad++;
			notes.push(`accept/reject mismatch on ${f}`);
			continue;
		}
		let refJson: unknown;
		try {
			refJson = JSON.parse(refProc.stdout.trim());
		} catch {
			bad++;
			notes.push(`ref parse failed on ${f}`);
			continue;
		}
		if (canon(refJson) !== canon(got.json)) {
			bad++;
			notes.push(`output differs on ${f}`);
		}
	}
	notes.push(`package differential: ${n - bad}/${n} files match upstream`);

	let negBad = 0;
	let negN = 0;
	const negDir = join(root, 'fixtures/opc/negative');
	if (existsSync(negDir)) {
		for (const name of readdirSync(negDir)) {
			const f = join(negDir, name);
			negN++;
			const got = dumpOpcPackageRust(f);
			if (got.ok) {
				negBad++;
				notes.push(`negative accepted: ${f}`);
				continue;
			}
			const d = got.json as { error?: string; detected?: unknown };
			if (d?.error !== 'unsupported_format' || !d.detected) {
				negBad++;
				notes.push(`negative shape on ${f}`);
			}
		}
	}
	notes.push(`negatives: ${negN - negBad}/${negN} rejected as unsupported_format`);
	bad += negBad;

	let hrefBad = 0;
	let hrefN = 0;
	const hrefPath = join(root, 'fixtures/opc/safe-href.jsonl');
	if (existsSync(hrefPath)) {
		const hrefTmp = mkdtempSync(join(tmpdir(), 'parity-href-'));
		try {
			for (const line of readFileSync(hrefPath, 'utf8').split('\n')) {
				if (!line.trim()) continue;
				hrefN++;
				const o = JSON.parse(line) as { id: string; href: string | null };
				let arg: string;
				if (o.href === null) arg = '__NULL__';
				else {
					const p = join(hrefTmp, 'h');
					writeFileSync(p, o.href, 'utf8');
					arg = `@${p}`;
				}
				const refScript = join(root, 'tools/model-compare/gates/ref/opc_ref.ts');
				const refProc = spawnSync('bun', [refScript, 'href', arg], {
					encoding: 'utf8',
					env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
				});
				const got = dumpOpcHrefRust(arg);
				let refJson: unknown = null;
				try {
					refJson = JSON.parse(refProc.stdout.trim());
				} catch {
					/* ignore */
				}
				if (
					refProc.status !== 0 ||
					!got.ok ||
					canon(refJson) !== canon(got.json)
				) {
					hrefBad++;
					notes.push(`href mismatch on ${o.id}`);
				}
			}
		} finally {
			rmSync(hrefTmp, { recursive: true, force: true });
		}
	}
	notes.push(`safe-href differential: ${hrefN - hrefBad}/${hrefN} match upstream`);
	bad += hrefBad;

	return { compared: n + negN + hrefN, mismatched: bad, notes };
}

export const opcComparison: Comparison = {
	id: 'opc',
	label: 'OPC package / safe-href (issue #13)',
	ready: true,
	async oracle(fixturePath: string) {
		return dumpOpcPackageInline(fixturePath);
	},
	async rust(fixturePath: string) {
		const { json } = dumpOpcPackageRust(fixturePath);
		return json;
	},
	selectFixtures(manifest: Manifest) {
		return selectCorpus(manifest, { skipOnDemand: true, tiers: ['S', 'M'] });
	},
	runCorpus: runOpcCorpus,
};
