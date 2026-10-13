// SPDX-License-Identifier: MIT
/**
 * TS oracle for docx.
 * - Styles/numbering/theme/settings slice (#20 / ready).
 * - View IR (LayoutResult + fake measurer): kept for when Print Layout lands.
 */

import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { repoRoot } from '../corpus.ts';
import { upstreamDir } from '../paths.ts';

/** Stage-4 styles dump (same shape as tools/model-compare/gates/ref/docx_styles_ref.ts). */
export async function dumpDocxStylesOracle(fixturePath: string): Promise<unknown> {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/docx_styles_ref.ts');
	const r = spawnSync('bun', [script, fixturePath], {
		encoding: 'utf8',
		env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
		maxBuffer: 32 * 1024 * 1024,
	});
	try {
		return JSON.parse((r.stdout || '').trim() || 'null');
	} catch {
		return { error: 'oracle_parse', status: r.status };
	}
}

/** Stage-6 shell dump (zoom / page tab / find / copy / links / diagnostics). */
export function dumpDocxShellOracle(fixturePath: string): {
	ok: boolean;
	json: unknown;
} {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/docx_shell_ref.ts');
	const r = spawnSync('bun', [script, fixturePath], {
		encoding: 'utf8',
		env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
		maxBuffer: 32 * 1024 * 1024,
	});
	let json: unknown = null;
	try {
		json = JSON.parse((r.stdout || '').trim() || 'null');
	} catch {
		json = { error: 'oracle_parse', status: r.status };
	}
	return { ok: r.status === 0, json };
}

/** LayoutResult view IR — not yet the CI docx comparison. */
export async function dumpDocxViewIr(fixturePath: string): Promise<unknown> {
	const up = upstreamDir();
	const { loadDocx } = await import(`${up}/src/core/docx/parse.ts`);
	const { layoutDocumentModel } = await import(`${up}/src/core/docx/layout/layout.ts`);
	const { createFakeMeasurer } = await import(`${up}/src/core/docx/layout/measure.ts`);

	const bytes = readFileSync(fixturePath);
	const loaded = await loadDocx(bytes);
	const result = layoutDocumentModel(loaded.model, createFakeMeasurer());
	return {
		format: 'docx',
		pages: result.pages,
		approximations: result.approximations ?? [],
	};
}
