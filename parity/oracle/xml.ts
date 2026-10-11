// SPDX-License-Identifier: MIT
/** TS oracle for the XML layer: same dump shape as tools/model-compare/gates/ref/xml_ref.ts. */

import { readFileSync } from 'node:fs';
import { upstreamDir } from '../paths.ts';

type Tree = unknown[];

function el(e: any): Tree {
	const attrs: [string | null, string, string][] = [];
	for (let i = 0; i < e.attributes.length; i++) {
		const a = e.attributes[i];
		if (a.name === 'xmlns' || a.prefix === 'xmlns') continue;
		attrs.push([a.namespaceURI || null, a.localName, a.value]);
	}
	attrs.sort((x, y) => {
		const kx = (x[0] ?? '') + '\u0000' + x[1];
		const ky = (y[0] ?? '') + '\u0000' + y[1];
		return kx < ky ? -1 : kx > ky ? 1 : 0;
	});
	const out: Tree = ['e', e.namespaceURI || null, e.localName, attrs];
	let text: string | null = null;
	for (let c = e.firstChild; c; c = c.nextSibling) {
		if (c.nodeType === 3 || c.nodeType === 4) {
			text = (text ?? '') + c.data;
			continue;
		}
		if (c.nodeType !== 1) continue;
		if (text !== null) {
			out.push(['t', text]);
			text = null;
		}
		out.push(el(c));
	}
	if (text !== null) out.push(['t', text]);
	return out;
}

export async function dumpXmlTree(filePath: string): Promise<unknown> {
	const up = upstreamDir();
	const { parseXml } = await import(`${up}/src/core/xml/xml.ts`);
	const decoder = new TextDecoder('utf-8');
	try {
		const doc = parseXml(decoder.decode(readFileSync(filePath)));
		return { file: filePath, tree: el(doc.documentElement) };
	} catch (e) {
		return { file: filePath, error: String((e as Error).message).slice(0, 80) };
	}
}

/** Batch oracle over a listfile (one path per line), JSONL to stdout-compatible array. */
export async function dumpXmlList(listPath: string): Promise<unknown[]> {
	const files = readFileSync(listPath, 'utf8').split('\n').filter(Boolean);
	const out: unknown[] = [];
	for (const file of files) out.push(await dumpXmlTree(file));
	return out;
}
