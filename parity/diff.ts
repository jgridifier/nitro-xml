// SPDX-License-Identifier: MIT
/**
 * Readable per-path diff of two normalised JSON values.
 */

import type { DiffEntry, KnownDiffEntry } from './types.ts';
import { normalize } from './normalize.ts';

function isPlainObject(v: unknown): v is Record<string, unknown> {
	return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function pushChanged(out: DiffEntry[], path: string, oracle: unknown, rust: unknown): void {
	out.push({ path, kind: 'changed', oracle, rust });
}

function walk(oracle: unknown, rust: unknown, path: string, out: DiffEntry[]): void {
	if (Object.is(oracle, rust)) return;
	if (typeof oracle === 'number' && typeof rust === 'number' && Object.is(oracle, rust)) return;

	if (Array.isArray(oracle) && Array.isArray(rust)) {
		const n = Math.max(oracle.length, rust.length);
		for (let i = 0; i < n; i++) {
			const p = `${path}/${i}`;
			if (i >= oracle.length) {
				out.push({ path: p, kind: 'extra', rust: rust[i] });
			} else if (i >= rust.length) {
				out.push({ path: p, kind: 'missing', oracle: oracle[i] });
			} else {
				walk(oracle[i], rust[i], p, out);
			}
		}
		return;
	}

	if (isPlainObject(oracle) && isPlainObject(rust)) {
		const keys = new Set([...Object.keys(oracle), ...Object.keys(rust)]);
		for (const key of [...keys].sort()) {
			const p = path === '' ? `/${key}` : `${path}/${key}`;
			if (!(key in oracle)) {
				out.push({ path: p, kind: 'extra', rust: rust[key] });
			} else if (!(key in rust)) {
				out.push({ path: p, kind: 'missing', oracle: oracle[key] });
			} else {
				walk(oracle[key], rust[key], p, out);
			}
		}
		return;
	}

	pushChanged(out, path || '/', oracle, rust);
}

/** Diff two values after normalisation. Returns [] when equal. */
export function diffIr(oracle: unknown, rust: unknown): DiffEntry[] {
	const out: DiffEntry[] = [];
	walk(normalize(oracle), normalize(rust), '', out);
	if (out.length === 0) return out;
	// Root-only change uses path `/`.
	return out.map((d) => (d.path === '' ? { ...d, path: '/' } : d));
}

function short(v: unknown, max = 120): string {
	const s = JSON.stringify(v);
	if (s === undefined) return 'undefined';
	return s.length <= max ? s : `${s.slice(0, max)}…`;
}

/** Format diffs for a terminal / CI log (capped). */
export function formatDiffs(diffs: DiffEntry[], limit = 40): string {
	const lines: string[] = [];
	const show = diffs.slice(0, limit);
	for (const d of show) {
		if (d.kind === 'changed') {
			lines.push(`  ${d.path}: oracle=${short(d.oracle)} rust=${short(d.rust)}`);
		} else if (d.kind === 'missing') {
			lines.push(`  ${d.path}: missing in rust (oracle=${short(d.oracle)})`);
		} else {
			lines.push(`  ${d.path}: extra in rust (rust=${short(d.rust)})`);
		}
	}
	if (diffs.length > limit) lines.push(`  … ${diffs.length - limit} more`);
	return lines.join('\n');
}

/**
 * Drop diffs covered by the known-diffs allow-list.
 * An entry with path `/` (or omitted path) covers every path for that fixture.
 */
export function filterKnownDiffs(
	diffs: DiffEntry[],
	format: string,
	fixture: string,
	known: KnownDiffEntry[],
): { unexpected: DiffEntry[]; allowed: DiffEntry[] } {
	const rules = known.filter((e) => e.format === format && e.fixture === fixture);
	if (rules.length === 0) return { unexpected: diffs, allowed: [] };

	const allowed: DiffEntry[] = [];
	const unexpected: DiffEntry[] = [];
	for (const d of diffs) {
		const ok = rules.some((r) => {
			const rp = r.path ?? '/';
			return rp === '/' || d.path === rp || d.path.startsWith(rp.endsWith('/') ? rp : `${rp}/`);
		});
		if (ok) allowed.push(d);
		else unexpected.push(d);
	}
	return { unexpected, allowed };
}
