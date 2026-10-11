// SPDX-License-Identifier: MIT
/** XML-layer comparison folded from tools/model-compare/gates/issue-12.sh. */

import { mkdtempSync, rmSync, writeFileSync, readFileSync, createWriteStream } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import type { Comparison, CorpusResult, Manifest } from '../types.ts';
import { repoRoot } from '../corpus.ts';
import { dumpXmlTree } from '../oracle/xml.ts';
import { dumpXmlFileRust, ensureExample } from '../rust/dump.ts';
import { normalizeJson } from '../normalize.ts';
import { upstreamDir } from '../paths.ts';

function runToFile(
	cmd: string,
	args: string[],
	outPath: string,
	env: NodeJS.ProcessEnv,
	timeoutMs: number,
): Promise<number> {
	return new Promise((resolve, reject) => {
		const out = createWriteStream(outPath);
		const child = spawn(cmd, args, { env, stdio: ['ignore', 'pipe', 'pipe'] });
		let stderr = '';
		child.stderr.on('data', (d) => {
			stderr += d.toString();
			if (stderr.length > 8000) stderr = stderr.slice(-8000);
		});
		child.stdout.pipe(out);
		const timer = setTimeout(() => {
			child.kill('SIGKILL');
			reject(new Error(`timeout after ${timeoutMs}ms: ${cmd}`));
		}, timeoutMs);
		child.on('error', (e) => {
			clearTimeout(timer);
			out.close();
			reject(e);
		});
		child.on('close', (code) => {
			clearTimeout(timer);
			out.end(() => resolve(code ?? 1));
		});
		void stderr;
	});
}

async function runXmlCorpus(): Promise<CorpusResult> {
	const root = repoRoot();
	ensureExample('xml_dump');
	const tmp = mkdtempSync(join(tmpdir(), 'parity-xml-'));
	const notes: string[] = [];
	try {
		const gen = spawnSync(
			'python3',
			[join(root, 'tools/model-compare/gates/make_xml_cases.py'), tmp],
			{ cwd: root, encoding: 'utf8' },
		);
		if (gen.status !== 0) {
			return { compared: 0, mismatched: 1, notes: [`make_xml_cases failed: ${gen.stderr}`] };
		}
		const listPath = join(tmp, 'list.txt');
		const refPath = join(tmp, 'ref.jsonl');
		const gotPath = join(tmp, 'got.jsonl');
		const refScript = join(root, 'tools/model-compare/gates/ref/xml_ref.ts');
		const refRc = await runToFile(
			'bun',
			[refScript, listPath],
			refPath,
			{ ...process.env, UPSTREAM_DIR: upstreamDir(root) },
			300_000,
		);
		if (refRc !== 0) {
			return { compared: 0, mismatched: 1, notes: [`xml_ref failed rc=${refRc}`] };
		}

		const bin = ensureExample('xml_dump');
		const gotRc = await runToFile(bin, [listPath], gotPath, process.env, 300_000);
		if (gotRc === 101 || gotRc === 124 || gotRc > 128) {
			return {
				compared: 0,
				mismatched: 1,
				notes: [`xml_dump panic/timeout/crash rc=${gotRc}`],
			};
		}
		if (gotRc !== 0) {
			return { compared: 0, mismatched: 1, notes: [`xml_dump failed rc=${gotRc}`] };
		}

		const refLines = readFileSync(refPath, 'utf8').split('\n').filter(Boolean);
		const gotByFile = new Map<string, { tree?: unknown; error?: string }>();
		for (const line of readFileSync(gotPath, 'utf8').split('\n')) {
			if (!line.trim()) continue;
			try {
				const o = JSON.parse(line) as { file: string; tree?: unknown; error?: string };
				gotByFile.set(o.file, o);
			} catch {
				/* skip bad line */
			}
		}
		let bad = 0;
		let compared = 0;
		for (const line of refLines) {
			const r = JSON.parse(line) as { file: string; tree?: unknown; error?: string };
			compared++;
			const g = gotByFile.get(r.file);
			if (!g) {
				bad++;
				if (notes.length < 20) notes.push(`missing output for ${r.file}`);
				continue;
			}
			const refErr = 'error' in r;
			const gotErr = 'error' in g;
			if (refErr !== gotErr) {
				bad++;
				if (notes.length < 20) notes.push(`accept/reject mismatch on ${r.file}`);
				continue;
			}
			if (r.tree !== undefined && normalizeJson(r.tree) !== normalizeJson(g.tree)) {
				bad++;
				if (notes.length < 20) notes.push(`tree differs on ${r.file}`);
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
		return [];
	},
	runCorpus: runXmlCorpus,
};
