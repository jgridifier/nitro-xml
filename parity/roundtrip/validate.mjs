// SPDX-License-Identifier: MIT
/**
 * Validator + LibreOffice hooks for gate (a) (#49).
 * Soft-skip until #48 installs tools via setup-validators.sh.
 */

import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { mkdtempSync, writeFileSync, rmSync, readdirSync } from 'node:fs';
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

/**
 * Run Open XML SDK validator; return JSON error array or null if tool missing.
 * @param {string} filePath
 * @returns {{ skipped: boolean, errors?: object[], reason?: string }}
 */
export function validateOpenXml(filePath) {
	const bin = ooxmlValidateBin();
	if (!bin) {
		return { skipped: true, reason: 'TODO(#48): ooxml-validate not installed' };
	}
	const cmd =
		bin.kind === 'exe'
			? spawnSync(bin.path, [filePath], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 })
			: spawnSync('dotnet', [bin.path, filePath], {
					encoding: 'utf8',
					maxBuffer: 16 * 1024 * 1024,
				});
	if (cmd.status !== 0 && !cmd.stdout) {
		return { skipped: false, errors: [{ message: cmd.stderr || 'validator failed' }] };
	}
	try {
		const parsed = JSON.parse(cmd.stdout || '[]');
		return { skipped: false, errors: Array.isArray(parsed) ? parsed : parsed.errors || [] };
	} catch {
		return { skipped: false, errors: [{ message: 'validator returned non-JSON' }] };
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

/**
 * LibreOffice headless reopen. Soft-skip if soffice missing (#48).
 * @param {Uint8Array} bytes
 * @param {string} ext file extension without dot
 */
export function libreOfficeReopen(bytes, ext) {
	const bin = sofficeBin();
	if (!bin) {
		return { skipped: true, reason: 'TODO(#48): LibreOffice not installed', ok: true };
	}
	const dir = mkdtempSync(join(tmpdir(), 'nitro-lo-'));
	try {
		const input = join(dir, `in.${ext}`);
		writeFileSync(input, bytes);
		const r = spawnSync(
			bin,
			['--headless', '--norestore', '--convert-to', 'pdf', '--outdir', dir, input],
			{ encoding: 'utf8', timeout: 120_000 },
		);
		const pdfs = readdirSync(dir).filter((f) => f.endsWith('.pdf'));
		return {
			skipped: false,
			ok: r.status === 0 && pdfs.length > 0,
			detail: r.status !== 0 ? (r.stderr || r.stdout || '').slice(0, 200) : undefined,
		};
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

export { ROOT };
