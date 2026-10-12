// SPDX-License-Identifier: MIT
// Catches: C14N must treat attribute reordering and xmlns prefix renaming as equal.
import { describe, expect, test } from 'bun:test';
import { canonicalizeXml, parseRels, parseContentTypes, compareTouchRebuild } from './compare.mjs';
import { listZipCentralDirectory, compareCarriedZipCd } from './zip-cd.mjs';
import { rustTouchSave, ensureOpcTouchSave } from './rust-save.mjs';

/** Minimal stored zip (local headers only — enough for CD parse after ZipWriter). */
async function makeStoredZip(parts) {
	const { default: JSZip } = await import('jszip');
	const z = new JSZip();
	for (const [name, data] of parts) {
		z.file(name, data, { compression: 'STORE' });
	}
	const buf = await z.generateAsync({ type: 'uint8array', compression: 'STORE' });
	return buf;
}

describe('canonicalizeXml', () => {
	test('attribute order ignored', () => {
		const a = canonicalizeXml('<r xmlns="urn:t" b="1" a="2"/>');
		const b = canonicalizeXml('<r xmlns="urn:t" a="2" b="1"/>');
		expect(a).toBe(b);
	});

	test('prefix rename ignored when namespaces match', () => {
		const a = canonicalizeXml('<w:p xmlns:w="urn:word"><w:r/></w:p>');
		const b = canonicalizeXml('<a:p xmlns:a="urn:word"><a:r/></a:p>');
		expect(a).toBe(b);
	});
});

describe('parseRels', () => {
	test('compares on Id Type Target TargetMode', () => {
		const xml = `<?xml version="1.0"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId2" Type="http://t/b" Target="b.xml"/>
  <Relationship Id="rId1" Type="http://t/a" Target="a.xml" TargetMode="External"/>
</Relationships>`;
		const rels = parseRels(xml);
		expect(rels.map((r) => r.id)).toEqual(['rId1', 'rId2']);
		expect(rels[0].targetMode).toBe('External');
	});
});

describe('parseContentTypes', () => {
	test('defaults and overrides sorted', () => {
		const xml = `<?xml version="1.0"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Override PartName="/xl/workbook.xml" ContentType="application/workbook"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>`;
		const ct = parseContentTypes(xml);
		expect(ct.defaults[0]).toContain('Default:rels:');
		expect(ct.overrides[0]).toContain('Override:/xl/workbook.xml:');
	});
});

describe('zip central directory', () => {
	// Catches: CD reader missing entries or scrambling order for carry checks.
	test('lists order, method, crc, sizes', async () => {
		const bytes = await makeStoredZip([
			['[Content_Types].xml', '<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>'],
			['docProps/core.xml', '<cp:coreProperties/>'],
			['xl/workbook.xml', '<workbook/>'],
		]);
		const cd = listZipCentralDirectory(bytes);
		expect(cd.map((e) => e.name)).toEqual([
			'[Content_Types].xml',
			'docProps/core.xml',
			'xl/workbook.xml',
		]);
		expect(cd.every((e) => e.method === 0)).toBe(true);
		expect(cd[1].uncompressedSize).toBe('<cp:coreProperties/>'.length);
	});
});

describe('touch-rebuild negative control', () => {
	// Catches: harness treating flipped-compression carry as lossless.
	test('corrupt carry fails zip-cd check', async () => {
		ensureOpcTouchSave();
		const original = await makeStoredZip([
			[
				'[Content_Types].xml',
				'<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>',
			],
			['docProps/core.xml', '<cp:coreProperties xmlns:cp="http://t"/>'],
			['xl/worksheets/sheet1.xml', '<worksheet pad="xxxxxxxxxxxxxxxxxxxxxxxx"/>'],
		]);
		const good = rustTouchSave(original);
		const goodCmp = await compareTouchRebuild(original, good.bytes, good.dirtyPart);
		expect(goodCmp.lossless).toBe(true);
		expect(good.bytes.length === original.length && good.bytes.every((b, i) => b === original[i])).toBe(
			false,
		);

		const bad = rustTouchSave(original, { corruptCarry: 'xl/worksheets/sheet1.xml' });
		const badCmp = await compareTouchRebuild(original, bad.bytes, bad.dirtyPart);
		expect(badCmp.lossless).toBe(false);
		expect(
			badCmp.losses.some(
				(l) =>
					l.category === 'zip-cd-method' ||
					l.category === 'zip-cd-crc' ||
					l.category === 'zip-cd-csize',
			),
		).toBe(true);

		// Revert: good path still passes after the corrupt probe.
		const again = rustTouchSave(original);
		const againCmp = await compareTouchRebuild(original, again.bytes, again.dirtyPart);
		expect(againCmp.lossless).toBe(true);
		const cd = compareCarriedZipCd(original, again.bytes, again.dirtyPart);
		expect(cd.ok).toBe(true);
	});
});
