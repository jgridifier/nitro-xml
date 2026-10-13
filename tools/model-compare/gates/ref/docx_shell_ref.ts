// SPDX-License-Identifier: MIT
// Reference side of the #29 docx shell dump: zoom, page tab, body find/copy,
// safe-href links, diagnostics. Layout pages stay "Page 1" until measure
// replies, matching the Rust shell snapshot taken at open.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR!;
const { hyperlinkPolicy } = await import(`${up}/src/core/opc/safe-href.ts`);
const { loadDocx } = await import(`${up}/src/core/docx/parse.ts`);

const FIND_CAP = 20;
const TEXT_CAP = 180;
const COPY_CAP = 4096;

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

type RunLink = { run: number; href?: string; anchor?: string; tooltip?: string };
type Para = { ordinal: number; text: string; links: RunLink[] };

function trimOrUndef(v: unknown): string | undefined {
	if (typeof v !== 'string') return undefined;
	const t = v.trim();
	return t ? t : undefined;
}

function walk(blocks: unknown[], out: Para[]) {
	for (const block of blocks) {
		if (!block || typeof block !== 'object') continue;
		const b = block as Record<string, unknown>;
		if (b.type === 'paragraph') {
			const runs = Array.isArray(b.runs) ? b.runs : [];
			let text = '';
			const links: RunLink[] = [];
			runs.forEach((run, i) => {
				if (!run || typeof run !== 'object') return;
				const r = run as Record<string, unknown>;
				text += typeof r.text === 'string' ? r.text : '';
				const link = r.link;
				if (!link || typeof link !== 'object') return;
				const l = link as Record<string, unknown>;
				links.push({
					run: i,
					href: trimOrUndef(l.href),
					anchor: trimOrUndef(l.anchor),
					tooltip: trimOrUndef(l.tooltip),
				});
			});
			out.push({ ordinal: out.length, text, links });
		} else if (b.type === 'table' && Array.isArray(b.rows)) {
			for (const row of b.rows) {
				if (!Array.isArray(row)) continue;
				for (const cell of row) {
					if (!cell || typeof cell !== 'object') continue;
					const c = cell as Record<string, unknown>;
					if (Array.isArray(c.paragraphs)) walk(c.paragraphs, out);
					if (Array.isArray(c.nestedTables)) walk(c.nestedTables, out);
				}
			}
		}
	}
}

function linkEntry(para: Para, link: RunLink): Record<string, unknown> | null {
	const anchor = link.anchor;
	const href = link.href;
	const allowed = !!anchor || hyperlinkPolicy(href).store;
	if (!allowed) return null;
	const open = !!anchor || hyperlinkPolicy(href).open;
	const entry: Record<string, unknown> = {
		row: para.ordinal,
		run: link.run,
		open,
	};
	if (!anchor && href) entry.target = href;
	if (anchor) entry.location = anchor;
	if (link.tooltip) entry.tooltip = link.tooltip;
	return entry;
}

function unsupportedOpen(message: string): boolean {
	const l = message.toLowerCase();
	return (
		l.includes('placeholder') ||
		l.includes('not represented') ||
		l.includes('cannot be shown') ||
		l.includes('never run') ||
		l.includes('not refreshed') ||
		l.includes('not editable')
	);
}

function diagnostics(messages: string[]) {
	const warnings: { code: string; message: string }[] = [];
	const unsupported: { code: string; message: string }[] = [];
	for (const message of messages) {
		const code = unsupportedOpen(message) ? 'unsupported_content' : 'docx_warning';
		(code === 'unsupported_content' ? unsupported : warnings).push({ code, message });
	}
	const cmp = (a: { message: string; code: string }, b: { message: string; code: string }) =>
		a.message < b.message ? -1 : a.message > b.message ? 1 : a.code < b.code ? -1 : a.code > b.code ? 1 : 0;
	warnings.sort(cmp);
	unsupported.sort(cmp);
	return { warnings, fontSubstitutions: [], unsupported };
}

async function dump(file: string) {
	const bytes = new Uint8Array(readFileSync(file));
	const loaded = await loadDocx(bytes);
	const paras: Para[] = [];
	walk(loaded.model.blocks ?? [], paras);
	const hits: Hit[] = [];
	for (const para of paras) pushHits(hits, 0, para.ordinal, para.text, 'a');
	const copy = paras
		.map((p) => p.text)
		.filter((t) => t.length > 0)
		.join('\n');
	const hyperlinks = [];
	for (const para of paras) {
		for (const link of para.links) {
			const entry = linkEntry(para, link);
			if (entry) hyperlinks.push(entry);
		}
	}
	const messages = Array.isArray(loaded.model.warnings) ? loaded.model.warnings.map(String) : [];
	return {
		format: 'docx',
		zoom: 100,
		navIndex: 0,
		tabs: [{ index: 0, name: 'Page 1', active: true }],
		selectionKind: 'body',
		copySample: capScalars(copy, COPY_CAP),
		hyperlinks,
		findSample: hits.slice(0, FIND_CAP),
		diagnostics: diagnostics(messages),
	};
}

const file = process.argv[2];
if (!file) {
	console.log(JSON.stringify({ error: 'usage' }));
	process.exit(2);
}
try {
	console.log(JSON.stringify(await dump(file)));
} catch (e) {
	console.log(JSON.stringify({ error: String(e) }));
	process.exit(1);
}
