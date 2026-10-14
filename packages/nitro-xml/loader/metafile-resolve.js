// SPDX-License-Identifier: MIT
/** Lazy EMF/WMF → PNG data URLs for `nitro-metafile:` src (upstream emf-converter). */

const METAFILE_RE = /^nitro-metafile:(emf|wmf);base64,(.+)$/i;

let convertMetafileToDataUrl;

async function loadConverter() {
	if (convertMetafileToDataUrl) return convertMetafileToDataUrl;
	const mod = await import("emf-converter");
	convertMetafileToDataUrl = mod.convertMetafileToDataUrl;
	return convertMetafileToDataUrl;
}

function decodeBase64(b64) {
	const bin = atob(b64);
	const out = new Uint8Array(bin.length);
	for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
	return out.buffer;
}

const pending = new WeakSet();

/**
 * @param {Element} el
 * @param {string} value
 */
export function scheduleMetafileAttr(el, value) {
	if (!value || !METAFILE_RE.test(value) || pending.has(el)) return;
	pending.add(el);
	const m = value.match(METAFILE_RE);
	const bytes = decodeBase64(m[2]);
	void loadConverter()
		.then((convert) => convert(bytes))
		.then((url) => {
			if (url && el.isConnected) {
				el.setAttribute("src", url);
				if (el.style) el.style.backgroundImage = `url(${url})`;
			}
		})
		.catch(() => {})
		.finally(() => pending.delete(el));
}

/**
 * @param {Element} el
 * @param {string} prop
 * @param {string} value
 */
export function scheduleMetafileStyle(el, prop, value) {
	if (prop !== "backgroundImage" || !value?.includes("nitro-metafile:")) return;
	const inner = value.match(/url\(([^)]+)\)/)?.[1]?.trim();
	if (inner) scheduleMetafileAttr(el, inner);
}
