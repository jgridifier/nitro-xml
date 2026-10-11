// SPDX-License-Identifier: MIT
/** Load fixtures/manifest.json and select the parity corpus (skip on-demand). */

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { FixtureRow, Manifest } from './types.ts';

export function repoRoot(): string {
	// parity/ is at <root>/parity
	return join(import.meta.dir, '..');
}

export function loadManifest(root = repoRoot()): Manifest {
	const path = join(root, 'fixtures/manifest.json');
	return JSON.parse(readFileSync(path, 'utf8')) as Manifest;
}

/**
 * Fixtures eligible for CI parity: redistributable, not on-demand (same skip as
 * `fixtures/verify --skip-on-demand`), optionally filtered by format / feature.
 */
export function selectCorpus(
	manifest: Manifest,
	opts: {
		format?: string;
		/** Require this feature tag (default: any redistributable non-on-demand). */
		feature?: string;
		/** When true (default), skip `on_demand` rows. */
		skipOnDemand?: boolean;
		/** Restrict to these tiers (default: S and M for PR CI). */
		tiers?: string[];
	} = {},
): FixtureRow[] {
	const skipOnDemand = opts.skipOnDemand !== false;
	const tiers = opts.tiers ?? ['S', 'M'];
	return manifest.fixtures.filter((f) => {
		if (!f.redistributable) return false;
		if (skipOnDemand && f.on_demand) return false;
		if (opts.format && f.format !== opts.format) return false;
		if (opts.feature && !f.features.includes(opts.feature)) return false;
		if (tiers.length && !tiers.includes(f.tier)) return false;
		return true;
	});
}

/** Resolve a fixture id to an absolute path. */
export function resolveFixture(
	manifest: Manifest,
	id: string,
	root = repoRoot(),
): { row: FixtureRow; absPath: string } {
	const row = manifest.fixtures.find((f) => f.id === id);
	if (!row) throw new Error(`unknown fixture id: ${id}`);
	if (!row.path) throw new Error(`fixture ${id} has no path (non-redistributable?)`);
	return { row, absPath: join(root, row.path) };
}
