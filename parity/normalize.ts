// SPDX-License-Identifier: MIT
/**
 * Normalise oracle/rust dumps before diffing: sort object keys, round floats to
 * 1e-3, drop volatile ids.
 */

const FLOAT_PLACES = 3;
const VOLATILE_KEY =
	/^(id|uuid|uid|guid|instanceId|runtimeId|objectId|revisionId|sessionId|timestamp|created|modified|lastModified|msid)$/i;

function roundNumber(n: number): number {
	if (!Number.isFinite(n)) return n;
	const f = 10 ** FLOAT_PLACES;
	return Math.round(n * f) / f;
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
	return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** Deep-normalise a JSON-compatible value for stable comparison. */
export function normalize(value: unknown): unknown {
	if (value === null || value === undefined) return value;
	if (typeof value === 'number') return roundNumber(value);
	if (typeof value === 'string' || typeof value === 'boolean') return value;
	if (Array.isArray(value)) return value.map(normalize);
	if (isPlainObject(value)) {
		const out: Record<string, unknown> = {};
		for (const key of Object.keys(value).sort()) {
			if (VOLATILE_KEY.test(key)) continue;
			out[key] = normalize(value[key]);
		}
		return out;
	}
	// Functions / symbols / bigint should not appear in IR dumps.
	return String(value);
}

/** Stable JSON string of a normalised value (sorted keys already applied). */
export function normalizeJson(value: unknown): string {
	return JSON.stringify(normalize(value));
}
