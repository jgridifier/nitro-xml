// SPDX-License-Identifier: MIT
/**
 * Normalise oracle/rust dumps before diffing: sort object keys, round floats to
 * 1e-3, drop volatile ids.
 */

const FLOAT_PLACES = 3;
const VOLATILE_KEY =
	/^(id|uuid|uid|guid|instanceId|runtimeId|objectId|revisionId|sessionId|timestamp|created|modified|lastModified|msid)$/i;

/** Opaque XML snapshots and volatile diagram host fields (#21 docx gate). */
const DOCX_XML_SNAPSHOT =
	/^(sourceParagraphPropertiesXml|restoredParagraphPropertiesXml|previousParagraphPropertiesXml|previousRunPropertiesXml|sourceRunPropertiesXml|restoredRunPropertiesXml)$/;

function stripDiagramHost(child: Record<string, unknown>): Record<string, unknown> {
	const {
		drawing,
		hostRelationships,
		colorsId,
		relationshipIds,
		parts,
		nodes,
		layout,
		quickStyleId,
		notice,
		rendering,
		issues,
		...rest
	} = child;
	return rest;
}

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
			if (DOCX_XML_SNAPSHOT.test(key)) continue;
			let child = value[key];
			// #28: deep chart part parsing is not in the #21 Rust gate yet.
			if (key === 'chart' && isPlainObject(child)) {
				const { chartSpace, formatting, colorPalette, workbookPartName, title, ...rest } =
					child as Record<string, unknown>;
				child = rest;
			}
			if (key === 'diagram' && isPlainObject(child)) {
				child = stripDiagramHost(child as Record<string, unknown>);
			}
			if (key === 'warnings' && Array.isArray(child)) {
				child = [...child].map(normalize).sort();
			}
			out[key] = normalize(child);
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
