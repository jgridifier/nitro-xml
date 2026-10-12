// SPDX-License-Identifier: MIT
/**
 * Package comparer for gate (a) (#49 / PRODUCT_SPEC §5):
 * part set + content types; rels on (Id, Type, Target, TargetMode);
 * binary byte-equality; C14N XML (prefixes → namespaces, attr order ignored,
 * insignificant whitespace ignored except xml:space="preserve");
 * unknown parts / extLst / mc:AlternateContent must survive.
 */

import JSZip from 'jszip';
import { DOMParser } from '@xmldom/xmldom';

/**
 * @typedef {{ category: string, part?: string, detail: string }} Loss
 * @typedef {{ lossless: boolean, losses: Loss[], partCounts: { input: number, output: number } }} CompareResult
 */

function isXmlPart(name) {
	return /\.(xml|rels)$/i.test(name) || name === '[Content_Types].xml';
}

function isBinaryPart(name) {
	return !isXmlPart(name);
}

/** Collapse insignificant whitespace; keep text under xml:space="preserve". */
function normalizeText(s, preserve) {
	if (preserve) return s;
	return s.replace(/\s+/g, ' ').trim();
}

/**
 * Canonicalize an XML document for equality:
 * - element QName as `{ns}local`
 * - attributes sorted by `{ns}local`, xmlns:* omitted (folded into QNames)
 * - insignificant whitespace dropped unless xml:space="preserve"
 */
export function canonicalizeXml(xmlText) {
	const doc = new DOMParser().parseFromString(xmlText, 'text/xml');
	const parseError = doc.getElementsByTagName('parsererror')[0];
	if (parseError) {
		// Fall back to raw normalized whitespace for broken/partial XML.
		return xmlText.replace(/\r\n/g, '\n').replace(/>\s+</g, '><').trim();
	}

	function qname(node) {
		const local = node.localName || node.nodeName.replace(/^.*:/, '');
		const ns = node.namespaceURI || '';
		return `{${ns}}${local}`;
	}

	function walk(node, preserveSpace) {
		if (node.nodeType === 3 || node.nodeType === 4) {
			// TEXT / CDATA
			const t = normalizeText(node.nodeValue || '', preserveSpace);
			return t ? JSON.stringify(t) : '';
		}
		if (node.nodeType === 8) return ''; // comment
		if (node.nodeType !== 1) return '';

		let preserve = preserveSpace;
		const spaceAttr = node.getAttribute?.('xml:space') || node.getAttribute?.('space');
		if (spaceAttr === 'preserve') preserve = true;
		if (spaceAttr === 'default') preserve = false;

		const attrs = [];
		if (node.attributes) {
			for (let i = 0; i < node.attributes.length; i++) {
				const a = node.attributes.item(i);
				const name = a.name || a.nodeName;
				if (name === 'xmlns' || name.startsWith('xmlns:')) continue;
				const local = a.localName || name.replace(/^.*:/, '');
				const ns = a.namespaceURI || '';
				// xml:space kept as ordinary attr for preserve semantics already applied
				attrs.push([`{${ns}}${local}`, a.value]);
			}
		}
		attrs.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
		const attrStr = attrs.map(([k, v]) => `${k}=${JSON.stringify(v)}`).join(' ');
		const kids = [];
		for (let c = node.firstChild; c; c = c.nextSibling) {
			const piece = walk(c, preserve);
			if (piece) kids.push(piece);
		}
		return `<${qname(node)}${attrStr ? ' ' + attrStr : ''}>${kids.join('')}</${qname(node)}>`;
	}

	const root = doc.documentElement;
	if (!root) return '';
	return walk(root, false);
}

async function zipParts(bytes) {
	const zip = await JSZip.loadAsync(bytes);
	/** @type {Map<string, Uint8Array>} */
	const parts = new Map();
	for (const [name, entry] of Object.entries(zip.files)) {
		if (entry.dir) continue;
		// Normalize zip paths (no leading ./)
		const key = name.replace(/^\.\//, '');
		parts.set(key, await entry.async('uint8array'));
	}
	return parts;
}

function bytesEqual(a, b) {
	if (a.length !== b.length) return false;
	for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
	return true;
}

function decodeUtf8(u8) {
	return new TextDecoder('utf-8', { fatal: false }).decode(u8);
}

/** Parse OPC relationships XML → sorted list of {id,type,target,targetMode}. */
export function parseRels(xml) {
	const doc = new DOMParser().parseFromString(xml, 'text/xml');
	const out = [];
	const nodes = doc.getElementsByTagName('Relationship');
	for (let i = 0; i < nodes.length; i++) {
		const n = nodes.item(i);
		out.push({
			id: n.getAttribute('Id') || '',
			type: n.getAttribute('Type') || '',
			target: n.getAttribute('Target') || '',
			targetMode: n.getAttribute('TargetMode') || '',
		});
	}
	out.sort((a, b) =>
		a.id < b.id ? -1 : a.id > b.id ? 1 : a.type < b.type ? -1 : a.type > b.type ? 1 : 0,
	);
	return out;
}

/** Content types as sorted default/override lines. */
export function parseContentTypes(xml) {
	const doc = new DOMParser().parseFromString(xml, 'text/xml');
	const defaults = [];
	const overrides = [];
	for (const tag of ['Default', 'Override']) {
		const nodes = doc.getElementsByTagName(tag);
		for (let i = 0; i < nodes.length; i++) {
			const n = nodes.item(i);
			if (tag === 'Default') {
				defaults.push(
					`Default:${n.getAttribute('Extension') || ''}:${n.getAttribute('ContentType') || ''}`,
				);
			} else {
				overrides.push(
					`Override:${n.getAttribute('PartName') || ''}:${n.getAttribute('ContentType') || ''}`,
				);
			}
		}
	}
	defaults.sort();
	overrides.sort();
	return { defaults, overrides, all: [...defaults, ...overrides] };
}

function categorizePartLoss(part, detail) {
	const lower = part.toLowerCase();
	if (lower.includes('vbaproject')) return 'vba';
	if (lower.includes('[content_types]')) return 'content-types';
	if (lower.endsWith('.rels')) return 'rels';
	if (lower.includes('extlst') || detail.includes('extLst')) return 'extLst';
	if (detail.includes('AlternateContent') || lower.includes('alternatecontent'))
		return 'alternate-content';
	if (isBinaryPart(part)) return 'binary';
	if (lower.includes('calcchain')) return 'calcChain';
	if (lower.includes('docprops')) return 'docProps';
	if (lower.includes('theme')) return 'theme';
	if (lower.includes('styles')) return 'styles';
	if (lower.includes('sharedstrings')) return 'sharedStrings';
	return 'xml-rewrite';
}

/**
 * Compare input package bytes to TS no-op-save output.
 * @param {Uint8Array} input
 * @param {Uint8Array} output
 * @returns {Promise<CompareResult>}
 */
export async function comparePackages(input, output) {
	/** @type {Loss[]} */
	const losses = [];
	const inParts = await zipParts(input);
	const outParts = await zipParts(output);

	const inNames = new Set(inParts.keys());
	const outNames = new Set(outParts.keys());

	for (const name of inNames) {
		if (!outNames.has(name)) {
			losses.push({
				category: categorizePartLoss(name, 'missing in output'),
				part: name,
				detail: 'part missing after save',
			});
		}
	}
	for (const name of outNames) {
		if (!inNames.has(name)) {
			losses.push({
				category: 'part-added',
				part: name,
				detail: 'part added on save',
			});
		}
	}

	// Content types
	const ctIn = inParts.get('[Content_Types].xml');
	const ctOut = outParts.get('[Content_Types].xml');
	if (ctIn && ctOut) {
		const a = parseContentTypes(decodeUtf8(ctIn));
		const b = parseContentTypes(decodeUtf8(ctOut));
		if (JSON.stringify(a.all) !== JSON.stringify(b.all)) {
			const missing = a.all.filter((x) => !b.all.includes(x));
			const added = b.all.filter((x) => !a.all.includes(x));
			losses.push({
				category: 'content-types',
				part: '[Content_Types].xml',
				detail: `defaults/overrides differ (missing=${missing.length}, added=${added.length})`,
			});
		}
	}

	for (const name of inNames) {
		if (!outNames.has(name)) continue;
		const a = inParts.get(name);
		const b = outParts.get(name);
		if (name.endsWith('.rels')) {
			const ra = parseRels(decodeUtf8(a));
			const rb = parseRels(decodeUtf8(b));
			// Compare on (Id, Type, Target, TargetMode) — order-insensitive via sort in parseRels
			if (JSON.stringify(ra) !== JSON.stringify(rb)) {
				losses.push({
					category: 'rels',
					part: name,
					detail: `relationship tuples differ (in=${ra.length}, out=${rb.length})`,
				});
			}
			continue;
		}
		if (name === '[Content_Types].xml') continue; // already handled
		if (isBinaryPart(name)) {
			if (!bytesEqual(a, b)) {
				losses.push({
					category: categorizePartLoss(name, 'binary'),
					part: name,
					detail: `binary bytes differ (in=${a.length}, out=${b.length})`,
				});
			}
			continue;
		}
		// XML
		const ca = canonicalizeXml(decodeUtf8(a));
		const cb = canonicalizeXml(decodeUtf8(b));
		if (ca !== cb) {
			const hasExt = decodeUtf8(a).includes('extLst') || decodeUtf8(b).includes('extLst');
			const hasAlt =
				decodeUtf8(a).includes('AlternateContent') ||
				decodeUtf8(b).includes('AlternateContent');
			losses.push({
				category: hasAlt
					? 'alternate-content'
					: hasExt
						? 'extLst'
						: categorizePartLoss(name, 'xml'),
				part: name,
				detail: 'canonical XML differs',
			});
		}
	}

	return {
		lossless: losses.length === 0,
		losses,
		partCounts: { input: inNames.size, output: outNames.size },
	};
}

/**
 * Touch-rebuild compare (#65): dirty part C14N-equal; every other part
 * byte-identical; carried zip CD (order/method/crc/sizes) identical.
 * @param {Uint8Array} input
 * @param {Uint8Array} output
 * @param {string} dirtyPart
 */
export async function compareTouchRebuild(input, output, dirtyPart) {
	const { compareCarriedZipCd } = await import('./zip-cd.mjs');
	/** @type {Loss[]} */
	const losses = [];
	const dirty = String(dirtyPart || '').replace(/^\//, '');

	const inParts = await zipParts(input);
	const outParts = await zipParts(output);
	const inNames = new Set(inParts.keys());
	const outNames = new Set(outParts.keys());

	for (const name of inNames) {
		if (!outNames.has(name)) {
			losses.push({
				category: 'part-missing',
				part: name,
				detail: 'part missing after touch save',
			});
		}
	}
	for (const name of outNames) {
		if (!inNames.has(name)) {
			losses.push({
				category: 'part-added',
				part: name,
				detail: 'part added on touch save',
			});
		}
	}

	for (const name of inNames) {
		if (!outNames.has(name)) continue;
		const a = inParts.get(name);
		const b = outParts.get(name);
		if (name === dirty) {
			if (isBinaryPart(name)) {
				if (!bytesEqual(a, b)) {
					losses.push({
						category: 'dirty-binary',
						part: name,
						detail: 'dirty binary part bytes differ',
					});
				}
			} else {
				const ca = canonicalizeXml(decodeUtf8(a));
				const cb = canonicalizeXml(decodeUtf8(b));
				if (ca !== cb) {
					losses.push({
						category: 'dirty-c14n',
						part: name,
						detail: 'dirty part not canonically equal',
					});
				}
			}
			continue;
		}
		// Untouched parts: inflated bytes must be identical.
		if (!bytesEqual(a, b)) {
			losses.push({
				category: 'carry-bytes',
				part: name,
				detail: `untouched part bytes differ (in=${a.length}, out=${b.length})`,
			});
		}
	}

	const cd = compareCarriedZipCd(input, output, dirty);
	if (!cd.ok) losses.push(...cd.losses);

	return {
		lossless: losses.length === 0,
		losses,
		partCounts: { input: inNames.size, output: outNames.size },
		dirtyPart: dirty,
	};
}

/** Catches: C14N must ignore attribute order and xmlns prefix renaming. */
export function _testOnly_canonicalizeXml(xml) {
	return canonicalizeXml(xml);
}
