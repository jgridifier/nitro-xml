// SPDX-License-Identifier: MIT
/** Locate the pinned upstream checkout and Rust dump binaries. */

import { existsSync } from 'node:fs';
import { execSync } from 'node:child_process';
import { join } from 'node:path';
import { repoRoot } from './corpus.ts';

export const UPSTREAM_PIN = 'e66aaec24adc0b68472eae1fc42aa4598e09c352';

export function upstreamDir(root = repoRoot()): string {
	const fromEnv = process.env.UPSTREAM_DIR;
	if (fromEnv && existsSync(fromEnv)) return fromEnv;
	const sub = join(root, 'upstream');
	if (existsSync(join(sub, 'src/core'))) return sub;
	// Local model-compare default used by issue-11/12/13 gates.
	if (existsSync('/workspace/ooxml-src/src/core')) return '/workspace/ooxml-src';
	return sub;
}

export function assertUpstreamPin(dir = upstreamDir()): void {
	let head: string;
	try {
		head = execSync('git rev-parse HEAD', { cwd: dir, encoding: 'utf8' }).trim();
	} catch {
		throw new Error(`upstream checkout missing at ${dir}; run git submodule update --init`);
	}
	if (head !== UPSTREAM_PIN) {
		throw new Error(`upstream not at pin ${UPSTREAM_PIN} (got ${head} in ${dir})`);
	}
}

export function rustExample(name: string, root = repoRoot()): string {
	const release = join(root, 'rs/target/release/examples', name);
	const debug = join(root, 'rs/target/debug/examples', name);
	if (existsSync(release)) return release;
	if (existsSync(debug)) return debug;
	return release;
}
