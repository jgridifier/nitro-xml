// SPDX-License-Identifier: MIT
/**
 * Rust xlsx save via `nitro-xml-xlsx` example `xlsx_save` (#68).
 * Test-only. The binary is built by scripts/ci/edit_parity.sh; this rebuilds if it is missing.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ROOT } from './scripts.mjs';

function rustExampleBin() {
	return join(ROOT, 'rs/target/release/examples/xlsx_save');
}

function ensureExample() {
	const bin = rustExampleBin();
	if (existsSync(bin)) return bin;
	const build = spawnSync(
		'cargo',
		['build', '--release', '-q', '-p', 'nitro-xml-xlsx', '--example', 'xlsx_save'],
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
		throw new Error(`failed to build xlsx_save: ${build.stderr || build.stdout}`);
	}
	if (!existsSync(bin)) {
		throw new Error(`xlsx_save binary missing after build: ${bin}`);
	}
	return bin;
}

/**
 * @param {Uint8Array} bytes
 * @param {string | null} scriptPath
 * @param {{ omitFullCalc?: boolean }} [opts]
 * @returns {Uint8Array}
 */
export function rustXlsxSave(bytes, scriptPath, opts = {}) {
	const bin = ensureExample();
	const dir = mkdtempSync(join(tmpdir(), 'nitro-xlsx-save-'));
	const input = join(dir, 'in.xlsx');
	const output = join(dir, 'out.xlsx');
	try {
		writeFileSync(input, bytes);
		const args = [input, output];
		if (scriptPath) args.push('--script', scriptPath);
		if (opts.omitFullCalc) args.push('--omit-full-calc');
		const r = spawnSync(bin, args, {
			encoding: 'buffer',
			maxBuffer: 64 * 1024 * 1024,
			timeout: 120_000,
		});
		if (r.status !== 0) {
			const err =
				(r.stderr && r.stderr.toString('utf8')) ||
				r.stdout?.toString('utf8') ||
				`exit ${r.status}`;
			throw new Error(err.trim().slice(0, 400) || `xlsx_save failed (${r.status})`);
		}
		return new Uint8Array(readFileSync(output));
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}
