// SPDX-License-Identifier: MIT
/**
 * TS oracle for the OPC layer: reuses the dump shape of
 * tools/model-compare/gates/ref/opc_ref.ts (package + href modes).
 */

import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { repoRoot } from '../corpus.ts';
import { upstreamDir } from '../paths.ts';

function runRef(args: string[]): { ok: boolean; json: unknown; raw: string } {
	const root = repoRoot();
	const script = join(root, 'tools/model-compare/gates/ref/opc_ref.ts');
	const r = spawnSync('bun', [script, ...args], {
		encoding: 'utf8',
		env: { ...process.env, UPSTREAM_DIR: upstreamDir(root) },
		maxBuffer: 32 * 1024 * 1024,
	});
	const raw = (r.stdout || '').trim();
	let json: unknown = null;
	try {
		json = raw ? JSON.parse(raw) : null;
	} catch {
		json = { error: 'parse', raw: raw.slice(0, 200) };
	}
	return { ok: r.status === 0, json, raw };
}

export async function dumpOpcPackage(fixturePath: string): Promise<unknown> {
	const { ok, json } = runRef(['package', fixturePath]);
	if (!ok && json && typeof json === 'object') return json;
	if (!ok) return { error: 'oracle_failed' };
	return json;
}

export async function dumpOpcHref(hrefArg: string): Promise<unknown> {
	const { ok, json } = runRef(['href', hrefArg]);
	if (!ok && json && typeof json === 'object') return json;
	if (!ok) return { error: 'oracle_failed' };
	return json;
}

/** Inline oracle (no subprocess) for package mode — used when bun can import upstream. */
export async function dumpOpcPackageInline(fixturePath: string): Promise<unknown> {
	const up = upstreamDir();
	const {
		parseContentTypes,
		parseRelationships,
		relationshipsPartFor,
		resolvePartPath,
		contentTypeForPart,
		RELATIONSHIP_TYPES,
		signature,
	} = await import(`${up}/src/core/opc/index.ts`);
	const { detectDigitalSignatures } = signature;
	const { readZipParts } = await import(`${up}/src/core/xlsx/read/package.ts`);

	try {
		const parts = await readZipParts(new Uint8Array(readFileSync(fixturePath)));
		const lower = new Map<string, string>();
		for (const name of parts.keys()) lower.set(name.toLowerCase(), name);
		const actual = (partName: string): string =>
			parts.has(partName) ? partName : (lower.get(partName.toLowerCase()) ?? partName);
		const decoder = new TextDecoder('utf-8');
		const textCache = new Map<string, string>();
		const text = (partName: string): string | undefined => {
			if (textCache.has(partName)) return textCache.get(partName);
			const bytes = parts.get(actual(partName));
			if (!bytes) return undefined;
			let t = decoder.decode(bytes);
			if (t.charCodeAt(0) === 0xfeff) t = t.slice(1);
			textCache.set(partName, t);
			return t;
		};
		const contentTypes = parseContentTypes(text('[Content_Types].xml'));
		const relsCache = new Map<string, Map<string, { id: string; type: string; target: string; mode: string }>>();
		const relsOf = (partName: string) => {
			const relsPart = partName === '' ? '_rels/.rels' : relationshipsPartFor(partName);
			let rels = relsCache.get(relsPart);
			if (!rels) {
				rels = parseRelationships(text(relsPart));
				relsCache.set(relsPart, rels);
			}
			return rels;
		};
		const targetOf = (partName: string, rel: { mode: string; target: string }): string | undefined => {
			if (rel.mode === 'External') return undefined;
			return actual(resolvePartPath(partName === '' ? '/' : partName, rel.target));
		};

		const partList = [...parts.keys()].sort();
		const relOwners = new Set<string>(['']);
		for (const name of partList) {
			if (name.endsWith('.rels')) continue;
			if (parts.has(relationshipsPartFor(name))) relOwners.add(name);
		}
		const owners = [...relOwners].sort();
		const relsOut: Record<string, unknown> = {};
		const targetsOut: Record<string, unknown> = {};
		for (const owner of owners) {
			const rels = relsOf(owner);
			const relMap: Record<string, unknown> = {};
			const typeTargets: Record<string, string> = {};
			for (const [id, rel] of rels) {
				relMap[id] = { id: rel.id, type: rel.type, target: rel.target, mode: rel.mode };
				const resolved = targetOf(owner, rel);
				if (resolved !== undefined && typeTargets[rel.type] === undefined)
					typeTargets[rel.type] = resolved;
			}
			relsOut[owner] = relMap;
			targetsOut[owner] = typeTargets;
		}

		const office =
			(() => {
				for (const rel of relsOf('').values())
					if (rel.type === RELATIONSHIP_TYPES.officeDocument) return targetOf('', rel);
				return undefined;
			})() ??
			(parts.has('xl/workbook.xml')
				? 'xl/workbook.xml'
				: parts.has('word/document.xml')
					? 'word/document.xml'
					: parts.has('ppt/presentation.xml')
						? 'ppt/presentation.xml'
						: undefined);
		const mainCt = office ? contentTypeForPart(contentTypes, office) : undefined;

		const defaults: Record<string, string> = {};
		for (const [k, v] of [...contentTypes.defaults.entries()].sort((a, b) =>
			a[0] < b[0] ? -1 : 1,
		))
			defaults[k] = v;
		const overrides: Record<string, string> = {};
		for (const [k, v] of [...contentTypes.overrides.entries()].sort((a, b) =>
			a[0] < b[0] ? -1 : 1,
		))
			overrides[k] = v;

		const firstTarget = (partName: string, type: string): string | null => {
			for (const rel of relsOf(partName).values()) {
				if (rel.type === type) {
					const t = targetOf(partName, rel);
					return t ?? null;
				}
			}
			return null;
		};

		return {
			format: formatFromContentType(mainCt),
			parts: partList,
			defaults,
			overrides,
			rels: relsOut,
			targets: targetsOut,
			signatures: detectDigitalSignatures(partList).signaturePaths.slice().sort(),
			properties: {
				core: firstTarget('', RELATIONSHIP_TYPES.coreProperties),
				app: firstTarget('', RELATIONSHIP_TYPES.extendedProperties),
				custom: firstTarget('', RELATIONSHIP_TYPES.customProperties),
			},
		};
	} catch (e) {
		return { error: String((e as Error).message).slice(0, 80) };
	}
}

function formatFromContentType(ct: string | undefined): string | null {
	if (!ct) return null;
	const map: Record<string, string> = {
		'application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml': 'docx',
		'application/vnd.ms-word.document.macroEnabled.main+xml': 'docm',
		'application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml': 'dotx',
		'application/vnd.ms-word.template.macroEnabledTemplate.main+xml': 'dotm',
		'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml': 'xlsx',
		'application/vnd.ms-excel.sheet.macroEnabled.main+xml': 'xlsm',
		'application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml': 'xltx',
		'application/vnd.ms-excel.template.macroEnabled.main+xml': 'xltm',
		'application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml': 'pptx',
		'application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml': 'pptm',
		'application/vnd.openxmlformats-officedocument.presentationml.template.main+xml': 'potx',
		'application/vnd.ms-powerpoint.template.macroEnabled.main+xml': 'potm',
	};
	return map[ct] ?? null;
}
