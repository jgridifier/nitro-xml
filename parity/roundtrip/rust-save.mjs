// SPDX-License-Identifier: MIT
/**
 * Rust no-op save via nitro-xml-opc example opc_noop_save (#65).
 * Spawns the release binary; builds it on first use.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ROOT } from './corpus.mjs';

function rustExampleBin(name) {
	return join(ROOT, 'rs/target/release/examples', name);
}

export function ensureOpcNoopSave() {
	const bin = rustExampleBin('opc_noop_save');
	if (existsSync(bin)) return bin;
	const build = spawnSync(
		'cargo',
		['build', '--release', '-q', '-p', 'nitro-xml-opc', '--example', 'opc_noop_save'],
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
		throw new Error(`failed to build opc_noop_save: ${build.stderr || build.stdout}`);
	}
	if (!existsSync(bin)) {
		throw new Error(`opc_noop_save binary missing after build: ${bin}`);
	}
	return bin;
}

/**
 * @param {Uint8Array} bytes
 * @returns {Uint8Array}
 */
export function rustNoopSave(bytes) {
	const bin = ensureOpcNoopSave();
	const dir = mkdtempSync(join(tmpdir(), 'nitro-opc-noop-'));
	const input = join(dir, 'in.bin');
	const output = join(dir, 'out.bin');
	try {
		writeFileSync(input, bytes);
		const r = spawnSync(bin, [input, output], {
			encoding: 'buffer',
			maxBuffer: 64 * 1024 * 1024,
			timeout: 60_000,
		});
		if (r.status !== 0) {
			const err = (r.stderr && r.stderr.toString('utf8')) || r.stdout?.toString('utf8') || `exit ${r.status}`;
			throw new Error(err.trim().slice(0, 300) || `opc_noop_save failed (${r.status})`);
		}
		return new Uint8Array(readFileSync(output));
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}
