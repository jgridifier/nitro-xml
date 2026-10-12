// SPDX-License-Identifier: MIT
/**
 * Zip central-directory reader for #65 touch-rebuild checks.
 * Compares entry order, compression method, CRC-32, compressed/uncompressed sizes
 * for carried (non-dirty) parts — proves raw_copy, not inflate+recompress.
 */

const EOCD_SIG = 0x06054b50;
const CDFH_SIG = 0x02014b50;

function readU16(view, off) {
	return view.getUint16(off, true);
}
function readU32(view, off) {
	return view.getUint32(off, true);
}

/**
 * @typedef {{ name: string, method: number, crc32: number, compressedSize: number, uncompressedSize: number }} ZipCdEntry
 */

/**
 * Parse the zip central directory (no inflation).
 * @param {Uint8Array} bytes
 * @returns {ZipCdEntry[]}
 */
export function listZipCentralDirectory(bytes) {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	// EOCD is at least 22 bytes; scan backwards for signature.
	let eocd = -1;
	const min = Math.max(0, bytes.length - 64 * 1024);
	for (let i = bytes.length - 22; i >= min; i--) {
		if (readU32(view, i) === EOCD_SIG) {
			eocd = i;
			break;
		}
	}
	if (eocd < 0) {
		throw new Error('zip: EOCD not found');
	}
	const cdCount = readU16(view, eocd + 10);
	const cdOffset = readU32(view, eocd + 16);
	/** @type {ZipCdEntry[]} */
	const entries = [];
	let off = cdOffset;
	for (let n = 0; n < cdCount; n++) {
		if (off + 46 > bytes.length || readU32(view, off) !== CDFH_SIG) {
			throw new Error(`zip: bad central directory header at ${off}`);
		}
		const method = readU16(view, off + 10);
		const crc32 = readU32(view, off + 16);
		const compressedSize = readU32(view, off + 20);
		const uncompressedSize = readU32(view, off + 24);
		const nameLen = readU16(view, off + 28);
		const extraLen = readU16(view, off + 30);
		const commentLen = readU16(view, off + 32);
		const nameBytes = bytes.subarray(off + 46, off + 46 + nameLen);
		const name = new TextDecoder('utf-8', { fatal: false })
			.decode(nameBytes)
			.replace(/^\//, '');
		if (!name.endsWith('/')) {
			entries.push({ name, method, crc32, compressedSize, uncompressedSize });
		}
		off += 46 + nameLen + extraLen + commentLen;
	}
	return entries;
}

/**
 * Compare CD metadata for carried parts (everything except dirtyPart).
 * @param {Uint8Array} input
 * @param {Uint8Array} output
 * @param {string} dirtyPart
 * @returns {{ ok: boolean, losses: { category: string, part?: string, detail: string }[] }}
 */
export function compareCarriedZipCd(input, output, dirtyPart) {
	/** @type {{ category: string, part?: string, detail: string }[]} */
	const losses = [];
	let inCd;
	let outCd;
	try {
		inCd = listZipCentralDirectory(input);
		outCd = listZipCentralDirectory(output);
	} catch (e) {
		return {
			ok: false,
			losses: [{ category: 'zip-cd', detail: String(e && e.message ? e.message : e) }],
		};
	}

	const dirty = String(dirtyPart || '').replace(/^\//, '');
	const inCarry = inCd.filter((e) => e.name !== dirty);
	const outCarry = outCd.filter((e) => e.name !== dirty);

	if (inCarry.length !== outCarry.length) {
		losses.push({
			category: 'zip-cd-count',
			detail: `carried entry count in=${inCarry.length} out=${outCarry.length}`,
		});
	}

	const n = Math.min(inCarry.length, outCarry.length);
	for (let i = 0; i < n; i++) {
		const a = inCarry[i];
		const b = outCarry[i];
		if (a.name !== b.name) {
			losses.push({
				category: 'zip-cd-order',
				part: a.name,
				detail: `order mismatch at ${i}: in=${a.name} out=${b.name}`,
			});
			continue;
		}
		if (a.method !== b.method) {
			losses.push({
				category: 'zip-cd-method',
				part: a.name,
				detail: `compression method in=${a.method} out=${b.method}`,
			});
		}
		if (a.crc32 !== b.crc32) {
			losses.push({
				category: 'zip-cd-crc',
				part: a.name,
				detail: `crc32 in=${a.crc32 >>> 0} out=${b.crc32 >>> 0}`,
			});
		}
		if (a.compressedSize !== b.compressedSize) {
			losses.push({
				category: 'zip-cd-csize',
				part: a.name,
				detail: `compressedSize in=${a.compressedSize} out=${b.compressedSize}`,
			});
		}
		if (a.uncompressedSize !== b.uncompressedSize) {
			losses.push({
				category: 'zip-cd-usize',
				part: a.name,
				detail: `uncompressedSize in=${a.uncompressedSize} out=${b.uncompressedSize}`,
			});
		}
	}

	return { ok: losses.length === 0, losses };
}
