// SPDX-License-Identifier: MIT
/**
 * Build per-case payloads for the visual host (TS oracle + Rust where ready).
 */

import { mkdirSync, writeFileSync, existsSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { readyCases } from './registry.mjs';

const VISUAL = dirname(fileURLToPath(import.meta.url));
const ROOT = join(VISUAL, '../..');
const OUT = join(VISUAL, 'out/payloads');

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

export async function prepareAll() {
	mkdirSync(OUT, { recursive: true });
	for (const c of readyCases()) {
		if (c.kind === 'chart-svg') prepareChart(c);
		else if (c.kind === 'xlsx-view-ir') await prepareXlsxViewIr(c);
		else throw new Error(`prepare: unknown kind for ${c.id}`);
		console.log(`visual prepare: ${c.id}`);
	}
}

if (import.meta.main) {
	await prepareAll();
}
