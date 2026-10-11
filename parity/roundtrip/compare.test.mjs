// SPDX-License-Identifier: MIT
// Catches: C14N must treat attribute reordering and xmlns prefix renaming as equal.
import { describe, expect, test } from 'bun:test';
import { canonicalizeXml, parseRels, parseContentTypes } from './compare.mjs';

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
