// SPDX-License-Identifier: MIT
// Shared helpers for the interaction audit: page driver, pixel-diff, tiny zip reader.
import { inflateRawSync } from 'node:zlib';
import { PNG } from '../../parity/visual/node_modules/pngjs/lib/png.js';

export const TAGS = { xlsx: 'nitro-xlsx', docx: 'nitro-docx', pptx: 'nitro-pptx' };

/** Open a demo page and wait for the document to paint. */
export async function openDemo(page, base, fmt, mode = 'full') {
	await page.goto(`${base}/${fmt}-${mode}.html`, { waitUntil: 'domcontentloaded', timeout: 60_000 });
	await page.waitForFunction(
		(tag) => {
			const el = document.querySelector(tag);
			const sr = el && el.shadowRoot;
			if (!sr || !el.document) return false;
			return !!(sr.querySelector('.dve-print-line, .xg-c, [aria-roledescription=slide]'));
		},
		TAGS[fmt],
		{ timeout: 60_000 },
	);
	await page.waitForTimeout(400);
}

/** Run `fn(el, sr)` in the page with the viewer element and its shadow root. */
export const inEl = (page, fmt, fn, arg) =>
	page.evaluate(
		([tag, src, a]) => {
			const el = document.querySelector(tag);
			// eslint-disable-next-line no-new-func
			return new Function('el', 'sr', 'a', `return (${src})(el, sr, a)`)(el, el.shadowRoot, a);
		},
		[TAGS[fmt], fn.toString(), arg],
	);

export const rectOf = (page, fmt, sel, i = 0) =>
	inEl(
		page,
		fmt,
		(el, sr, a) => {
			const e = sr.querySelectorAll(a.sel)[a.i];
			if (!e) return null;
			const r = e.getBoundingClientRect();
			return { x: r.x, y: r.y, w: r.width, h: r.height };
		},
		{ sel, i },
	);

export const uiState = (page, fmt) =>
	inEl(page, fmt, async (el) => (await el.shellAction('ui')).ui).then((u) => ({
		selection: u?.selection ?? '',
		lastCopy: u?.lastCopy ?? '',
	}));

export const dirtyText = (page) =>
	page.evaluate(() => /Unsaved changes/.test(document.body.innerText) ? 'dirty' : 'clean');

/** Concatenated rendered text for a format (DOM text; canvas text is invisible to this). */
export const domText = (page, fmt) =>
	inEl(page, fmt, (el, sr) => {
		const q = sr.querySelector('.dom');
		return q ? q.textContent : '';
	});

export const domKids = (page, fmt) => inEl(page, fmt, (el, sr) => sr.querySelector('.dom')?.childElementCount ?? -1);

// ---------- pixel helpers ----------
export async function clipPng(page, clip) {
	const buf = await page.screenshot({ clip, animations: 'allow' });
	return PNG.sync.read(buf);
}

/** Changed-pixel stats between two same-size PNGs. */
export function diffStats(a, b, tol = 24, skip = null) {
	let n = 0, x0 = 1e9, x1 = -1, y0 = 1e9, y1 = -1;
	const w = a.width, h = a.height;
	let blueish = 0;
	for (let y = 0; y < h; y++)
		for (let x = 0; x < w; x++) {
			if (skip && x >= skip.x && x < skip.x + skip.w && y >= skip.y && y < skip.y + skip.h) continue;
			const i = (y * w + x) * 4;
			const d = Math.abs(a.data[i] - b.data[i]) + Math.abs(a.data[i + 1] - b.data[i + 1]) + Math.abs(a.data[i + 2] - b.data[i + 2]);
			if (d > tol) {
				n++;
				if (x < x0) x0 = x;
				if (x > x1) x1 = x;
				if (y < y0) y0 = y;
				if (y > y1) y1 = y;
				if (b.data[i + 2] > b.data[i] + 20) blueish++;
			}
		}
	return { changed: n, ratio: n / (w * h), bbox: n ? { x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1 } : null, blueish };
}

/** True when any of several samples over ~1 caret-blink period differs from `base`. */
export async function anyChange(page, clip, base, samples = [0, 250, 550, 900], tol = 24, skip = null) {
	let best = { changed: 0, ratio: 0, bbox: null, blueish: 0 };
	for (const t of samples) {
		if (t) await page.waitForTimeout(t === samples[0] ? 0 : 250);
		const s = diffStats(base, await clipPng(page, clip), tol, skip);
		if (s.changed > best.changed) best = s;
	}
	return best;
}

// ---------- minimal zip reader (central directory; stored + deflate) ----------
export function zipEntries(buf) {
	const out = {};
	let eocd = -1;
	for (let i = buf.length - 22; i >= 0; i--) if (buf.readUInt32LE(i) === 0x06054b50) { eocd = i; break; }
	if (eocd < 0) return out;
	const n = buf.readUInt16LE(eocd + 10);
	let p = buf.readUInt32LE(eocd + 16);
	for (let k = 0; k < n; k++) {
		const method = buf.readUInt16LE(p + 10);
		const csize = buf.readUInt32LE(p + 20);
		const nlen = buf.readUInt16LE(p + 28), elen = buf.readUInt16LE(p + 30), clen = buf.readUInt16LE(p + 32);
		const lho = buf.readUInt32LE(p + 42);
		const name = buf.toString('utf8', p + 46, p + 46 + nlen);
		const lnlen = buf.readUInt16LE(lho + 26), lelen = buf.readUInt16LE(lho + 28);
		const data = buf.subarray(lho + 30 + lnlen + lelen, lho + 30 + lnlen + lelen + csize);
		out[name] = () => (method === 0 ? data : inflateRawSync(data));
		p += 46 + nlen + elen + clen;
	}
	return out;
}

/** Save the open document through the element and return whether any XML part contains `needle`. */
export async function savedContains(page, fmt, needle) {
	const arr = await inEl(page, fmt, async (el) => Array.from(await el.save()));
	const z = zipEntries(Buffer.from(arr));
	for (const [name, get] of Object.entries(z)) {
		if (!/\.xml$/.test(name)) continue;
		try {
			if (get().toString('utf8').includes(needle)) return true;
		} catch { /* skip unreadable part */ }
	}
	return false;
}

export const pct = (a, p) => {
	if (!a.length) return null;
	const s = [...a].sort((x, y) => x - y);
	return +s[Math.min(s.length - 1, Math.ceil((p / 100) * s.length) - 1)].toFixed(1);
};

/** Paragraph texts from the saved model: docx word/document.xml (top-level + table paragraphs, in order). */
export async function docxModelParas(page) {
	const arr = await inEl(page, 'docx', async (el) => Array.from(await el.save()));
	const z = zipEntries(Buffer.from(arr));
	const xml = z['word/document.xml']().toString('utf8');
	const paras = xml.split(/<\/w:p>/).slice(0, -1);
	return paras.map((p) => [...p.matchAll(/<w:t(?:\s[^>]*)?>([^<]*)<\/w:t>/g)].map((m) => m[1]).join(''));
}
