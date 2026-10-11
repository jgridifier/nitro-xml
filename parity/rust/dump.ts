// SPDX-License-Identifier: MIT
/** Spawn Rust dump examples (xml_dump, opc_dump, future view_ir_dump). */

import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { repoRoot } from '../corpus.ts';
import { rustExample } from '../paths.ts';

function runBin(
	bin: string,
	args: string[],
	opts: { timeoutMs?: number } = {},
): { status: number | null; stdout: string; stderr: string } {
	const r = spawnSync(bin, args, {
		encoding: 'utf8',
		timeout: opts.timeoutMs ?? 120_000,
		maxBuffer: 64 * 1024 * 1024,
		env: process.env,
	});
	return {
		status: r.status,
		stdout: r.stdout || '',
		stderr: r.stderr || '',
	};
}

export function ensureExample(name: string, crate = 'nitro-xml-opc'): string {
	const root = repoRoot();
	const bin = rustExample(name, root);
	if (existsSync(bin)) return bin;
	const build = spawnSync(
		'cargo',
		['build', '--release', '-q', '-p', crate, '--example', name],
		{
			cwd: join(root, 'rs'),
			encoding: 'utf8',
			env: { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` },
		},
	);
	if (build.status !== 0) {
		throw new Error(`failed to build example ${name} (-p ${crate}): ${build.stderr}`);
	}
	return rustExample(name, root);
}

export function dumpXlsxWorkbookRust(
	fixturePath: string,
): { ok: boolean; json: unknown; status: number | null } {
	const bin = ensureExample('xlsx_dump', 'nitro-xml-xlsx');
	const r = runBin(bin, [fixturePath], { timeoutMs: 60_000 });
	let json: unknown = null;
	try {
		json = JSON.parse(r.stdout.trim() || 'null');
	} catch {
		json = { error: 'parse', raw: r.stdout.slice(0, 200) };
	}
	return { ok: r.status === 0, json, status: r.status };
}

export function dumpDocxStylesRust(
	fixturePath: string,
): { ok: boolean; json: unknown; status: number | null } {
	const bin = ensureExample('docx_styles_dump', 'nitro-xml-docx');
	const r = runBin(bin, [fixturePath], { timeoutMs: 60_000 });
	let json: unknown = null;
	try {
		json = JSON.parse(r.stdout.trim() || 'null');
	} catch {
		json = { error: 'parse', raw: r.stdout.slice(0, 200) };
	}
	return { ok: r.status === 0, json, status: r.status };
}

/** Issue #24 presentation-model dump (`pptx_dump` example). */
export function dumpPptxModelRust(
	fixturePath: string,
): { ok: boolean; json: unknown; status: number | null } {
	const bin = ensureExample('pptx_dump', 'nitro-xml-pptx');
	const r = runBin(bin, [fixturePath], { timeoutMs: 120_000 });
	let json: unknown = null;
	try {
		json = JSON.parse(r.stdout.trim() || 'null');
	} catch {
		json = { error: 'parse', raw: r.stdout.slice(0, 200) };
	}
	return { ok: r.status === 0, json, status: r.status };
}

export function dumpXmlListRust(listPath: string): unknown[] {
	const bin = ensureExample('xml_dump');
	const r = runBin(bin, [listPath], { timeoutMs: 300_000 });
	if (r.status === 101 || r.status === 124 || (r.status !== null && r.status > 128)) {
		throw new Error(`xml_dump panic/timeout/crash rc=${r.status}`);
	}
	const out: unknown[] = [];
	for (const line of r.stdout.split('\n')) {
		if (!line.trim()) continue;
		try {
			out.push(JSON.parse(line));
		} catch {
			out.push({ error: 'parse', raw: line.slice(0, 80) });
		}
	}
	return out;
}

export function dumpXmlFileRust(filePath: string): unknown {
	const dir = mkdtempSync(join(tmpdir(), 'parity-xml-'));
	const list = join(dir, 'list.txt');
	writeFileSync(list, `${filePath}\n`);
	try {
		const rows = dumpXmlListRust(list);
		return rows[0] ?? { file: filePath, error: 'empty' };
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

export function dumpOpcPackageRust(fixturePath: string): { ok: boolean; json: unknown } {
	const bin = ensureExample('opc_dump');
	const r = runBin(bin, ['package', fixturePath], { timeoutMs: 60_000 });
	let json: unknown = null;
	try {
		json = JSON.parse(r.stdout.trim() || 'null');
	} catch {
		json = { error: 'parse', raw: r.stdout.slice(0, 200) };
	}
	return { ok: r.status === 0, json };
}

export function dumpOpcHrefRust(hrefArg: string): { ok: boolean; json: unknown } {
	const bin = ensureExample('opc_dump');
	const r = runBin(bin, ['href', hrefArg], { timeoutMs: 30_000 });
	let json: unknown = null;
	try {
		json = JSON.parse(r.stdout.trim() || 'null');
	} catch {
		json = { error: 'parse', raw: r.stdout.slice(0, 200) };
	}
	return { ok: r.status === 0, json };
}

/**
 * Format view-IR dump. Looks for `view_ir_dump` in the format crate; until the
 * parser issue lands, returns a not_implemented marker so oracle.json / rust.json
 * are still written.
 */
export function dumpFormatViewIrRust(
	format: 'xlsx' | 'docx' | 'pptx',
	fixturePath: string,
	todo: string,
): unknown {
	const root = repoRoot();
	const crate =
		format === 'xlsx'
			? 'nitro-xml-xlsx'
			: format === 'docx'
				? 'nitro-xml-docx'
				: 'nitro-xml-pptx';
	const bin = join(root, 'rs/target/release/examples/view_ir_dump');
	const alt = join(root, `rs/crates/${crate}/../../target/release/examples/view_ir_dump`);
	const path = existsSync(bin) ? bin : existsSync(alt) ? alt : null;
	// Prefer a format-specific example name when present.
	const named = join(root, 'rs/target/release/examples', `${format}_view_ir_dump`);
	const use = existsSync(named) ? named : path;
	if (!use) {
		return {
			__parity: 'not_implemented',
			format,
			todo,
			fixture: fixturePath,
		};
	}
	const r = runBin(use, [format, fixturePath], { timeoutMs: 120_000 });
	try {
		return JSON.parse(r.stdout.trim() || 'null');
	} catch {
		return {
			__parity: 'dump_failed',
			format,
			todo,
			status: r.status,
			stderr: r.stderr.slice(0, 200),
		};
	}
}
