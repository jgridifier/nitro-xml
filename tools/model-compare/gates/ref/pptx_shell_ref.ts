// SPDX-License-Identifier: MIT
// Reference side of the #29 pptx shell dump: zoom, slide tabs, visible-text
// find/copy, diagnostics. Table cells, speaker notes, and a:hlinkClick are
// outside copy_text — TODO(#27) — so hyperlinks stay empty and element text
// is the trimmed shape string (or child text for groups).
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR!;
const { PptxHandler } = await import(`${up}/src/core/pptx/core/PptxHandler.ts`);

const FIND_CAP = 20;
const TEXT_CAP = 180;
const COPY_CAP = 4096;

/** Kinds Rust keeps on `slide.elements` (search row index). */
const IN_TREE = new Set([
	'shape',
	'text',
	'picture',
	'image',
	'group',
	'connector',
	'table',
	'chart',
	'smartArt',
	'ole',
	'media',
]);

function foldAscii(s: string): string {
	let out = '';
	for (const c of s) {
		const code = c.codePointAt(0)!;
		out += code >= 65 && code <= 90 ? String.fromCodePoint(code + 32) : c;
	}
	return out;
}

function capScalars(s: string, max: number): string {
	return [...s].slice(0, max).join('');
}

type Hit = { sheet: number; row: number; col: number; text: string };

function pushHits(matches: Hit[], sheet: number, row: number, text: string, query: string) {
	if (!query || !text) return;
	const hay = [...foldAscii(text)];
	const needle = [...foldAscii(query)];
	const original = [...text];
	if (!needle.length) return;
	let start = 0;
	while (start + needle.length <= hay.length) {
		let ok = true;
		for (let i = 0; i < needle.length; i++) {
			if (hay[start + i] !== needle[i]) {
				ok = false;
				break;
			}
		}
		if (!ok) {
			start++;
			continue;
		}
		matches.push({
			sheet,
			row,
			col: start,
			text: capScalars(original.slice(start, start + needle.length).join(''), TEXT_CAP),
		});
		start += Math.max(needle.length, 1);
	}
}

function elementText(el: Record<string, unknown>): string {
	if (typeof el.text === 'string') {
		const trimmed = el.text.trim();
		if (trimmed) return trimmed;
	}
	const parts: string[] = [];
	const children = el.children;
	if (Array.isArray(children)) {
		for (const child of children) {
			if (!child || typeof child !== 'object') continue;
			const text = elementText(child as Record<string, unknown>);
			if (text) parts.push(text);
		}
	}
	return parts.join('\n');
}

function otherLabel(type: string): string | null {
	if (IN_TREE.has(type)) return null;
	// p16:model3D and unparsed spTree children are recorded as `unknown`.
	if (type === 'model3d' || type === 'unknown') return 'unknown';
	return type;
}

const file = process.argv[2];
if (!file) {
	console.log(JSON.stringify({ error: 'usage' }));
	process.exit(2);
}

try {
	const bytes = new Uint8Array(readFileSync(file));
	const handler = new PptxHandler();
	const quiet = (..._args: unknown[]) => {};
	const orig = {
		log: console.log,
		info: console.info,
		warn: console.warn,
		debug: console.debug,
	};
	console.log = quiet;
	console.info = quiet;
	console.warn = quiet;
	console.debug = quiet;
	let data: { slides?: Array<{ elements?: Array<Record<string, unknown>> }> };
	try {
		data = await handler.load(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength), {
			eagerDecodeImages: false,
		});
	} finally {
		console.log = orig.log;
		console.info = orig.info;
		console.warn = orig.warn;
		console.debug = orig.debug;
	}
	const slides = data.slides ?? [];
	const hits: Hit[] = [];
	const unsupported = new Set<string>();
	let copySample = '';
	let selectionKind = 'none';
	slides.forEach((slide, si) => {
		let row = 0;
		for (const el of slide.elements ?? []) {
			const type = String(el.type ?? '');
			const label = otherLabel(type);
			if (label) {
				unsupported.add(label);
				continue;
			}
			if (!IN_TREE.has(type)) continue;
			const text = elementText(el);
			if (si === 0 && selectionKind === 'none' && text) {
				selectionKind = 'shape';
				copySample = capScalars(text, COPY_CAP);
			}
			pushHits(hits, si, row, text, 'a');
			row++;
		}
	});
	const unsupportedItems = [...unsupported]
		.sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))
		.map((message) => ({ code: 'unsupported_content', message }));
	const tabs = slides.map((_, i) => ({
		index: i,
		name: `Slide ${i + 1}`,
		active: i === 0,
	}));
	console.log(
		JSON.stringify({
			format: 'pptx',
			zoom: 100,
			navIndex: 0,
			tabs,
			selectionKind,
			copySample,
			hyperlinks: [],
			findSample: hits.slice(0, FIND_CAP),
			diagnostics: {
				warnings: [],
				fontSubstitutions: [],
				unsupported: unsupportedItems,
			},
		}),
	);
} catch (e) {
	console.log(JSON.stringify({ error: String(e) }));
	process.exit(1);
}
