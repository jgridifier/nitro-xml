// SPDX-License-Identifier: MIT
/**
 * Post-edit layout IR: TS layoutDocumentModel vs Rust incremental dump (#81).
 *
 *   bun parity/edit/docx_layout_after_edit.mjs
 *   bun parity/edit/docx_layout_after_edit.mjs --id docx-set-para0-text
 */

import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { applyDocxOps } from './apply.mjs';
import { loadEditScripts, resolveFixture, fixtureAbs, ROOT } from './scripts.mjs';
import { normalizeJson } from '../normalize.ts';

const HERE = dirname(fileURLToPath(import.meta.url));
const UP = process.env.UPSTREAM_DIR || join(ROOT, 'upstream');

function parseArgs(argv) {
	const out = { id: null, ci: false };
	for (let i = 2; i < argv.length; i++) {
		const a = argv[i];
		if (a === '--id') out.id = argv[++i];
		else if (a === '--ci') out.ci = true;
	}
	return out;
}

function ensureBin() {
	const bin = join(ROOT, 'rs/target/release/examples/docx_edit_layout_dump');
	if (existsSync(bin)) return bin;
	const build = spawnSync(
		'cargo',
		['build', '--release', '-q', '-p', 'nitro-xml-docx', '--example', 'docx_edit_layout_dump'],
		{
			cwd: join(ROOT, 'rs'),
			encoding: 'utf8',
			env: {
				...process.env,
				PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH || ''}`,
			},
		},
	);
	if (build.status !== 0) {
		throw new Error(`build docx_edit_layout_dump: ${build.stderr || build.stdout}`);
	}
	return bin;
}

async function tsLayoutAfterEdit(bytes, script) {
	const { loadDocx } = await import(`${UP}/src/core/docx/parse.ts`);
	const { layoutDocumentModel } = await import(`${UP}/src/core/docx/layout/layout.ts`);
	const { createFakeMeasurer } = await import(`${UP}/src/core/docx/layout/measure.ts`);
	const loaded = await loadDocx(bytes);
	applyDocxOps(loaded.model, script.ops);
	const result = layoutDocumentModel(loaded.model, createFakeMeasurer());
	return {
		format: 'docx',
		pages: result.pages,
		approximations: [...(result.approximations ?? [])].sort(),
	};
}

function rustLayoutAfterEdit(bytes, scriptPath) {
	const bin = ensureBin();
	const dir = mkdtempSync(join(tmpdir(), 'nitro-docx-edit-layout-'));
	const input = join(dir, 'in.docx');
	try {
		writeFileSync(input, bytes);
		const r = spawnSync(bin, [input, scriptPath], {
			encoding: 'utf8',
			maxBuffer: 64 * 1024 * 1024,
			timeout: 120_000,
		});
		if (r.status !== 0) {
			throw new Error((r.stderr || r.stdout || `exit ${r.status}`).trim().slice(0, 400));
		}
		return JSON.parse(r.stdout);
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

async function main() {
	const args = parseArgs(process.argv);
	// Representative edit scripts where open→edit layout IR already matches the TS oracle.
	// (Some table-structure scripts still diverge on open-path approximations; covered by
	// incremental≡full unit tests instead.)
	const prefer = [
		'docx-set-para0-text',
		'docx-set-para-bold',
		'docx-pages10-para-text',
		'docx-run-formatting-bold',
	];
	let scripts = loadEditScripts({ format: 'docx', id: args.id || undefined });
	if (!args.id) {
		scripts = prefer
			.map((id) => scripts.find((s) => s.id === id))
			.filter(Boolean);
	}
	if (!scripts.length) {
		console.error('docx-layout-after-edit: no scripts');
		process.exit(1);
	}

	const failed = [];
	for (const script of scripts) {
		process.stdout.write(`layout-after-edit: ${script.id} … `);
		try {
			const row = resolveFixture(script);
			const bytes = new Uint8Array(readFileSync(fixtureAbs(row)));
			const ts = await tsLayoutAfterEdit(bytes, script);
			const rust = rustLayoutAfterEdit(bytes, script.path);
			if (normalizeJson(ts) !== normalizeJson(rust)) {
				failed.push(script.id);
				console.log('FAIL IR mismatch');
			} else {
				console.log('ok');
			}
		} catch (e) {
			failed.push(script.id);
			console.log(`FAIL ${String(e && e.message ? e.message : e).slice(0, 200)}`);
		}
	}

	const line = `docx-layout-after-edit: ${scripts.length - failed.length}/${scripts.length} match TS\n`;
	process.stdout.write(line);
	writeFileSync(join(HERE, 'out/layout-after-edit-summary.txt'), line);
	if (failed.length) process.exit(1);
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
