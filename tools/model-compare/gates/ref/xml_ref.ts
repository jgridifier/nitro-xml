// SPDX-License-Identifier: MIT
// Reference side of the #12 gate: upstream parseXml() trees in the xml_dump JSONL shape.
import { readFileSync } from 'node:fs';
const up = process.env.UPSTREAM_DIR!;
const { parseXml } = await import(`${up}/src/core/xml/xml.ts`);
const decoder = new TextDecoder('utf-8');
type Tree = unknown[];
function el(e: any): Tree {
	const attrs: [string | null, string, string][] = [];
	for (let i = 0; i < e.attributes.length; i++) {
		const a = e.attributes[i];
		if (a.name === 'xmlns' || a.prefix === 'xmlns') continue;
		attrs.push([a.namespaceURI || null, a.localName, a.value]);
	}
	attrs.sort((x, y) => ((x[0] ?? '') + '\u0000' + x[1] < (y[0] ?? '') + '\u0000' + y[1] ? -1 : (x[0] ?? '') + '\u0000' + x[1] > (y[0] ?? '') + '\u0000' + y[1] ? 1 : 0));
	const out: Tree = ['e', e.namespaceURI || null, e.localName, attrs];
	let text: string | null = null;
	for (let c = e.firstChild; c; c = c.nextSibling) {
		if (c.nodeType === 3 || c.nodeType === 4) { text = (text ?? '') + c.data; continue; }
		if (c.nodeType !== 1) continue;
		if (text !== null) { out.push(['t', text]); text = null; }
		out.push(el(c));
	}
	if (text !== null) out.push(['t', text]);
	return out;
}
const files = readFileSync(process.argv[2]!, 'utf8').split('\n').filter(Boolean);
for (const file of files) {
	try {
		const doc = parseXml(decoder.decode(readFileSync(file)));
		console.log(JSON.stringify({ file, tree: el(doc.documentElement) }));
	} catch (e) {
		console.log(JSON.stringify({ file, error: String((e as Error).message).slice(0, 80) }));
	}
}
