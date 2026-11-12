// SPDX-License-Identifier: MIT
// Per-part byte identity and save/reopen assertions for the edit corpus.
import JSZip from 'jszip';
import { DOMParser, XMLSerializer } from '@xmldom/xmldom';
import { posix } from 'node:path';
import { canonicalizeXml } from '../roundtrip/compare.mjs';

const X = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main';
const A = 'http://schemas.openxmlformats.org/drawingml/2006/main';
const P = 'http://schemas.openxmlformats.org/presentationml/2006/main';
const R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const parse = text => new DOMParser().parseFromString(text, 'text/xml');
const elements = (node, ns, name) => Array.from(node.getElementsByTagNameNS(ns, name));
export const flattenOps = ops => ops.flatMap(op => op.op === 'batch' ? flattenOps(op.ops) : [op]);

async function indexedParts(zip, format) {
    const parent = format === 'xlsx' ? 'xl/workbook.xml' : 'ppt/presentation.xml';
    const relPath = posix.join(posix.dirname(parent), '_rels', posix.basename(parent) + '.rels');
    const rels = parse(await zip.file(relPath).async('string'));
    const targets = new Map(Array.from(rels.getElementsByTagName('Relationship')).map(r => [
        r.getAttribute('Id'), posix.normalize(posix.join(posix.dirname(parent), r.getAttribute('Target'))).replace(/^\//, ''),
    ]));
    const doc = parse(await zip.file(parent).async('string'));
    return elements(doc, format === 'xlsx' ? X : P, format === 'xlsx' ? 'sheet' : 'sldId')
        .map(n => targets.get(n.getAttributeNS(R, 'id')));
}

export async function checkPartIdentity(script, input, output) {
    const before = await JSZip.loadAsync(input), after = await JSZip.loadAsync(output);
    const ops = flattenOps(script.ops || []);
    const allowed = new Set();
    if (script.format === 'docx') allowed.add('word/document.xml');
    if (script.format === 'pptx') {
        const slides = await indexedParts(before, 'pptx');
        if (ops.some(o => o.op === 'reorderSlide')) allowed.add('ppt/presentation.xml');
        for (const op of ops) {
            if (Number.isInteger(op.slideIndex)) allowed.add(slides[op.slideIndex]);
        }
    }
    if (script.format === 'xlsx') {
        const sheets = await indexedParts(before, 'xlsx');
        const structure = ops.some(o => ['addSheet', 'deleteSheet', 'renameSheet'].includes(o.op));
        const values = ops.some(o => ['setCellInput', 'setCellValue', 'insertRows', 'deleteRows', 'insertColumns', 'deleteColumns'].includes(o.op));
        for (const op of ops) if (Number.isInteger(op.sheet)) allowed.add(sheets[op.sheet]);
        if (structure) {
            allowed.add('xl/workbook.xml');
            if (ops.some(o => ['addSheet', 'deleteSheet'].includes(o.op))) {
                allowed.add('xl/_rels/workbook.xml.rels');
                allowed.add('[Content_Types].xml');
                const newSheets = await indexedParts(after, 'xlsx');
                for (const part of new Set([...sheets, ...newSheets])) {
                    if (!sheets.includes(part) || !newSheets.includes(part)) allowed.add(part);
                }
            }
        }
        // Formula invalidation can dirty sheets beyond the directly edited one.
        let hasFormula = false;
        for (const part of sheets) {
            const xml = await before.file(part).async('string');
            if (elements(parse(xml), X, 'f').length && (values || structure)) {
                hasFormula = true;
                allowed.add(part);
            }
        }
        if (hasFormula || ops.some(o => o.op === 'setCellInput' && o.text.startsWith('='))) {
            allowed.add('xl/workbook.xml');
            allowed.add('xl/calcChain.xml');
            if (before.file('xl/calcChain.xml')) {
                allowed.add('xl/_rels/workbook.xml.rels');
                allowed.add('[Content_Types].xml');
            }
        }
        const styleCount = async zip => {
            const part = zip.file('xl/styles.xml');
            if (!part) return 0;
            const xfs = elements(parse(await part.async('string')), X, 'cellXfs')[0];
            return xfs ? elements(xfs, X, 'xf').length : 0;
        };
        if (await styleCount(after) > await styleCount(before)) allowed.add('xl/styles.xml');
        // Existing indices must survive; only an appended string may change the table.
        const sst = 'xl/sharedStrings.xml';
        const oldStrings = before.file(sst) ? elements(parse(await before.file(sst).async('string')), X, 'si').map(n => canonicalizeXml(new XMLSerializer().serializeToString(n))) : [];
        const newStrings = after.file(sst) ? elements(parse(await after.file(sst).async('string')), X, 'si').map(n => canonicalizeXml(new XMLSerializer().serializeToString(n))) : [];
        if (newStrings.length > oldStrings.length && oldStrings.every((v, i) => v === newStrings[i])) {
            allowed.add(sst);
            if (!before.file(sst)) {
                allowed.add('xl/_rels/workbook.xml.rels');
                allowed.add('[Content_Types].xml');
            }
        }
        if (ops.some(o => ['insertRows', 'deleteRows', 'insertColumns', 'deleteColumns'].includes(o.op))) {
            allowed.add('xl/metadata.xml');
            if (!before.file('xl/metadata.xml') && after.file('xl/metadata.xml')) {
                allowed.add('xl/_rels/workbook.xml.rels');
                allowed.add('[Content_Types].xml');
            }
        }
    }
    const losses = [];
    for (const part of new Set([...Object.keys(before.files), ...Object.keys(after.files)])) {
        if (before.files[part]?.dir || after.files[part]?.dir || allowed.has(part)) continue;
        const a = before.file(part), b = after.file(part);
        if (!a || !b || !Buffer.from(await a.async('uint8array')).equals(Buffer.from(await b.async('uint8array')))) {
            losses.push({ category: 'part-identity', part, detail: !a ? 'unexpected added part' : !b ? 'unexpected removed part' : 'untouched part is not byte-identical' });
        }
    }
    return losses;
}

// Upstream drops geometry edits on shapes without xfrm. Supply only that missing
// expected transform, then compare the entire slide normally (no slide-wide waiver).
export async function expectedPptxGeometry(bytes, script) {
    const zip = await JSZip.loadAsync(bytes);
    const slides = await indexedParts(zip, 'pptx');
    for (const op of flattenOps(script.ops || [])) {
        if (op.op !== 'patchShapeGeometry') continue;
        const part = slides[op.slideIndex];
        const doc = parse(await zip.file(part).async('string'));
        const tree = elements(doc, P, 'spTree')[0];
        const shapes = Array.from(tree.childNodes).filter(n => n.nodeType === 1 && ['sp', 'pic', 'cxnSp', 'graphicFrame', 'grpSp'].includes(n.localName));
        const shape = shapes[op.elementIndex];
        if (!shape) throw new Error('geometry target missing after reopen');
        const props = elements(shape, P, 'spPr')[0];
        if (!props || elements(props, A, 'xfrm').length) continue;
        const xfrm = doc.createElementNS(A, 'a:xfrm');
        xfrm.setAttribute('rot', String(Math.round((op.rotation || 0) * 60000)));
        const off = doc.createElementNS(A, 'a:off'), ext = doc.createElementNS(A, 'a:ext');
        for (const [key, node, attr] of [['x', off, 'x'], ['y', off, 'y'], ['width', ext, 'cx'], ['height', ext, 'cy']]) {
            node.setAttribute(attr, String(Math.round((op[key] || 0) * 9525)));
        }
        xfrm.appendChild(off); xfrm.appendChild(ext); props.insertBefore(xfrm, props.firstChild);
        zip.file(part, new XMLSerializer().serializeToString(doc));
    }
    return zip.generateAsync({ type: 'uint8array' });
}

export async function checkReopenedModel(format, expected, actual) {
    const { snapshotXlsx, snapshotDocx, snapshotPptx, stableStringify } = await import('../undo/snapshot.mjs');
    async function snapshot(bytes) {
        if (format === 'xlsx') {
            const { loadXlsx } = await import('../../upstream/src/core/xlsx/read/load.ts');
            return snapshotXlsx(await loadXlsx(bytes));
        }
        if (format === 'docx') {
            const { loadDocx } = await import('../../upstream/src/core/docx/parse.ts');
            return snapshotDocx((await loadDocx(bytes)).model);
        }
        const { PptxHandler } = await import('../../upstream/src/core/pptx/core/PptxHandler.ts');
        const data = await new PptxHandler().load(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
        return snapshotPptx(data.slides);
    }
    const a = stableStringify(await snapshot(expected));
    const b = stableStringify(await snapshot(actual));
    if (a === b) return [];
    let offset = 0;
    while (offset < Math.min(a.length, b.length) && a[offset] === b[offset]) offset++;
    return [{ category: 'reopen-model', detail: `saved model differs at ${offset}: expected ${a.slice(offset, offset + 140)}; got ${b.slice(offset, offset + 140)}` }];
}
