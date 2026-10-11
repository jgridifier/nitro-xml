// SPDX-License-Identifier: MIT
/** Canonicalize embedded chart XML fragments before IR diff (xmldom round-trip). */

const XML_KEYS =
	/(Xml|extLst|sourceXml|effectsXml|printSettings|chartExtLst|pictureOptionsXml)$/;

let canonXml: ((s: string) => string) | null = null;

async function getCanonXml(): Promise<(s: string) => string> {
	if (canonXml) return canonXml;
	const up = process.env.UPSTREAM_DIR ?? `${import.meta.dir}/../upstream`;
	const { parseXml, buildXml } = await import(`${up}/src/core/xml/index.ts`);
	const { NS } = await import(`${up}/src/core/xml/namespaces.ts`);
	const wrap = (s: string) =>
		`<chartFragmentWrap xmlns:c="${NS.c}" xmlns:a="${NS.a}" xmlns:r="${NS.r}">${s}</chartFragmentWrap>`;
	canonXml = (s: string) => {
		if (typeof s !== 'string' || !s.trim().startsWith('<')) return s;
		try {
			const doc = parseXml(wrap(s), { label: 'Chart fragment' });
			const child = doc.documentElement?.firstChild;
			if (!child) return s;
			return buildXml(child);
		} catch {
			return s;
		}
	};
	return canonXml;
}

export async function normalizeChartIr(value: unknown): Promise<unknown> {
	const canon = await getCanonXml();
	const walk = (v: unknown): unknown => {
		if (typeof v === 'string') return v;
		if (Array.isArray(v)) return v.map(walk);
		if (v && typeof v === 'object') {
			const out: Record<string, unknown> = {};
			for (const [k, val] of Object.entries(v as Record<string, unknown>)) {
				if (typeof val === 'string' && XML_KEYS.test(k)) {
					out[k] = canon(val);
				} else {
					out[k] = walk(val);
				}
			}
			return out;
		}
		return v;
	};
	return walk(value);
}
