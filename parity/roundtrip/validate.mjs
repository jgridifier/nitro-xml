// SPDX-License-Identifier: MIT
/**
 * Validator + LibreOffice hooks for gate (a) (#49 / #48).
 * Requires tools from scripts/ci/setup-validators.sh in CI.
 */

import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { mkdtempSync, writeFileSync, rmSync, readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');

function toolsDir() {
	return process.env.NITRO_TOOLS_DIR || join(process.env.HOME || '/tmp', '.nitro-tools');
}

export function ooxmlValidateBin() {
	const p = join(toolsDir(), 'ooxml-validate', 'ooxml-validate');
	const dll = join(toolsDir(), 'ooxml-validate', 'ooxml-validate.dll');
	if (existsSync(p)) return { kind: 'exe', path: p };
	if (existsSync(dll)) return { kind: 'dotnet', path: dll };
	return null;
}

export function sofficeBin() {
	for (const c of ['soffice', 'libreoffice']) {
		const r = spawnSync('which', [c], { encoding: 'utf8' });
		if (r.status === 0 && r.stdout.trim()) return r.stdout.trim();
	}
	return null;
}

function dotnetEnv() {
	const env = { ...process.env };
	const dotnetRoot = join(toolsDir(), 'dotnet');
	if (existsSync(join(dotnetRoot, 'dotnet'))) {
		env.DOTNET_ROOT = dotnetRoot;
		env.PATH = `${dotnetRoot}:${env.PATH || ''}`;
	}
	return env;
}

/**
 * Run Open XML SDK validator; return JSON error array.
 * @param {string} filePath
 * @returns {{ skipped: boolean, errors?: object[], reason?: string }}
 */
export function validateOpenXml(filePath) {
	const bin = ooxmlValidateBin();
	if (!bin) {
		return { skipped: true, reason: 'ooxml-validate not installed (run setup-validators.sh / #48)' };
	}
	const env = dotnetEnv();
	const cmd =
		bin.kind === 'exe'
			? spawnSync(bin.path, [filePath], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024, env })
			: spawnSync('dotnet', [bin.path, filePath], {
					encoding: 'utf8',
					maxBuffer: 16 * 1024 * 1024,
					env,
				});
	if (cmd.status !== 0 && !cmd.stdout) {
		return { skipped: false, errors: [{ id: 'validator-fail', description: cmd.stderr || 'validator failed' }] };
	}
	try {
		const parsed = JSON.parse(cmd.stdout || '[]');
		const errors = Array.isArray(parsed) ? parsed : parsed.errors || [];
		return { skipped: false, errors };
	} catch {
		return { skipped: false, errors: [{ id: 'validator-non-json', description: 'validator returned non-JSON' }] };
	}
}

/**
 * Error-set subset: every output error must appear in the input set (stringified).
 */
export function errorSubset(inputErrors, outputErrors) {
	const inSet = new Set((inputErrors || []).map((e) => JSON.stringify(e)));
	const extras = (outputErrors || []).filter((e) => !inSet.has(JSON.stringify(e)));
	return { ok: extras.length === 0, extras };
}

function pdfPageCount(pdfPath) {
	const buf = readFileSync(pdfPath);
	const re = /\/Type\s*\/Page(?!\s*s)\b/g;
	return (buf.toString('latin1').match(re) || []).length;
}

/**
 * LibreOffice headless unit count for one package (PRODUCT_SPEC §5).
 * xlsx/xlsm → convert to ODS, count <table:table>; pptx/docx → PDF page count.
 * @param {Uint8Array} bytes
 * @param {string} ext
 * @returns {{ ok: boolean, count: number, detail?: string }}
 */
function libreOfficeCount(bytes, ext) {
	const bin = sofficeBin();
	if (!bin) return { ok: false, count: -1, detail: 'no soffice' };
	if (ext == null || ext === '') {
		return { ok: false, count: -1, detail: 'missing package extension' };
	}
	const e = String(ext).replace(/^\./, '').toLowerCase();
	const dir = mkdtempSync(join(tmpdir(), 'nitro-lo-'));
	try {
		const input = join(dir, `in.${e}`);
		writeFileSync(input, bytes);
		if (e.startsWith('xls')) {
			const r = spawnSync(
				bin,
				['--headless', '--norestore', '--convert-to', 'ods', '--outdir', dir, input],
				{ encoding: 'utf8', timeout: 180_000 },
			);
			const ods = readdirSync(dir).filter((f) => f.endsWith('.ods'));
			if (r.status !== 0 || !ods.length) {
				return { ok: false, count: -1, detail: (r.stderr || r.stdout || 'ods convert failed').slice(0, 240) };
			}
			// ODS is a zip; count table:table in content.xml without extra deps.
			const unzip = spawnSync('unzip', ['-p', join(dir, ods[0]), 'content.xml'], {
				encoding: 'utf8',
				maxBuffer: 64 * 1024 * 1024,
			});
			if (unzip.status !== 0) {
				return { ok: false, count: -1, detail: 'ods content.xml missing' };
			}
			const count = (unzip.stdout.match(/<table:table[\s>]/g) || []).length;
			return { ok: count > 0, count };
		}
		const r = spawnSync(
			bin,
			['--headless', '--norestore', '--convert-to', 'pdf', '--outdir', dir, input],
			{ encoding: 'utf8', timeout: 180_000 },
		);
		const pdfs = readdirSync(dir).filter((f) => f.endsWith('.pdf'));
		if (r.status !== 0 || !pdfs.length) {
			return { ok: false, count: -1, detail: (r.stderr || r.stdout || 'pdf convert failed').slice(0, 240) };
		}
		const count = pdfPageCount(join(dir, pdfs[0]));
		return { ok: count > 0, count };
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

/**
 * LibreOffice reopen: convert input and output; require exit 0 and equal counts.
 * @param {Uint8Array} inputBytes
 * @param {Uint8Array} outputBytes
 * @param {string} ext
 */
export function libreOfficeReopen(inputBytes, outputBytes, ext) {
	const bin = sofficeBin();
	if (!bin) {
		return { skipped: true, reason: 'LibreOffice not installed (run setup-validators.sh / #48)', ok: true };
	}
	const inn = libreOfficeCount(inputBytes, ext);
	if (!inn.ok) {
		return { skipped: false, ok: false, count: inn.count, expectCount: inn.count, detail: `input: ${inn.detail}` };
	}
	const out = libreOfficeCount(outputBytes, ext);
	if (!out.ok) {
		return {
			skipped: false,
			ok: false,
			count: out.count,
			expectCount: inn.count,
			detail: `output: ${out.detail}`,
		};
	}
	const ok = out.count === inn.count;
	return {
		skipped: false,
		ok,
		count: out.count,
		expectCount: inn.count,
		detail: ok ? undefined : `count ${out.count} != input ${inn.count}`,
	};
}

export { ROOT };
