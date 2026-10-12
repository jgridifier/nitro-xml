// SPDX-License-Identifier: MIT
/**
 * Rust PackageWriter saves via nitro-xml-opc examples (#65).
 *   opc_noop_save  — identity shortcut (original bytes)
 *   opc_touch_save — dirty one part + rebuild/raw_copy carry
 */

import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ROOT } from './corpus.mjs';

function rustExampleBin(name) {
	return join(ROOT, 'rs/target/release/examples', name);
}

function ensureExample(name) {
	const bin = rustExampleBin(name);
	if (existsSync(bin)) return bin;
	const build = spawnSync(
		'cargo',
		['build', '--release', '-q', '-p', 'nitro-xml-opc', '--example', name],
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
		throw new Error(`failed to build ${name}: ${build.stderr || build.stdout}`);
	}
	if (!existsSync(bin)) {
		throw new Error(`${name} binary missing after build: ${bin}`);
	}
	return bin;
}

export function ensureOpcNoopSave() {
	return ensureExample('opc_noop_save');
}

export function ensureOpcTouchSave() {
	return ensureExample('opc_touch_save');
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
			const err =
				(r.stderr && r.stderr.toString('utf8')) ||
				r.stdout?.toString('utf8') ||
				`exit ${r.status}`;
			throw new Error(err.trim().slice(0, 300) || `opc_noop_save failed (${r.status})`);
		}
		return new Uint8Array(readFileSync(output));
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

/**
 * @param {Uint8Array} bytes
 * @param {{ corruptCarry?: string }} [opts]
 * @returns {{ bytes: Uint8Array, dirtyPart: string, meta: object }}
 */
export function rustTouchSave(bytes, opts = {}) {
	const bin = ensureOpcTouchSave();
	const dir = mkdtempSync(join(tmpdir(), 'nitro-opc-touch-'));
	const input = join(dir, 'in.bin');
	const output = join(dir, 'out.bin');
	try {
		writeFileSync(input, bytes);
		const args = [input, output];
		if (opts.corruptCarry) {
			args.push('--corrupt-carry', opts.corruptCarry);
		}
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
			throw new Error(err.trim().slice(0, 300) || `opc_touch_save failed (${r.status})`);
		}
		let meta = {};
		try {
			meta = JSON.parse((r.stdout && r.stdout.toString('utf8')) || '{}');
		} catch {
			meta = {};
		}
		return {
			bytes: new Uint8Array(readFileSync(output)),
			dirtyPart: meta.dirtyPart || '',
			meta,
		};
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}
