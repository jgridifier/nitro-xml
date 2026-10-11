// SPDX-License-Identifier: MIT
// Reference side of the #13 gate: upstream SourceIndex + hyperlinkPolicy in the opc_dump JSON shape.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR!;
const {
	parseContentTypes,
	parseRelationships,
	relationshipsPartFor,
	resolvePartPath,
	contentTypeForPart,
	RELATIONSHIP_TYPES,
	hyperlinkPolicy,
	signature,
} = await import(`${up}/src/core/opc/index.ts`);
const { detectDigitalSignatures } = signature;
const { readZipParts } = await import(`${up}/src/core/xlsx/read/package.ts`);

const mode = process.argv[2];

if (mode === 'href') {
	const raw = process.argv[3];
	const href = raw === undefined || raw === '__NULL__' ? null : raw;
	const policy = hyperlinkPolicy(href);
	const out: Record<string, unknown> = { store: policy.store, open: policy.open };
	if (policy.reason !== undefined) out.reason = policy.reason;
	console.log(JSON.stringify(out));
	process.exit(0);
}

if (mode !== 'package') {
	console.log(JSON.stringify({ error: 'usage' }));
	process.exit(2);
}

const file = process.argv[3]!;
try {
	const parts = await readZipParts(new Uint8Array(readFileSync(file)));
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
			if (resolved !== undefined && typeTargets[rel.type] === undefined) typeTargets[rel.type] = resolved;
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
	const format = formatFromContentType(mainCt);

	const defaults: Record<string, string> = {};
	for (const [k, v] of [...contentTypes.defaults.entries()].sort((a, b) => (a[0] < b[0] ? -1 : 1)))
		defaults[k] = v;
	const overrides: Record<string, string> = {};
	for (const [k, v] of [...contentTypes.overrides.entries()].sort((a, b) => (a[0] < b[0] ? -1 : 1)))
		overrides[k] = v;

	const signatures = detectDigitalSignatures(partList).signaturePaths.slice().sort();
	const props = {
		core: firstTarget('', RELATIONSHIP_TYPES.coreProperties),
		app: firstTarget('', RELATIONSHIP_TYPES.extendedProperties),
		custom: firstTarget('', RELATIONSHIP_TYPES.customProperties),
	};

	function firstTarget(partName: string, type: string): string | null {
		for (const rel of relsOf(partName).values()) {
			if (rel.type === type) {
				const t = targetOf(partName, rel);
				return t ?? null;
			}
		}
		return null;
	}

	console.log(
		JSON.stringify({
			format,
			parts: partList,
			defaults,
			overrides,
			rels: relsOut,
			targets: targetsOut,
			signatures,
			properties: props,
		}),
	);
} catch (e) {
	console.log(JSON.stringify({ error: String((e as Error).message).slice(0, 80) }));
	process.exit(2);
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
