// SPDX-License-Identifier: MIT
// NODE_PATH=/workspace/host-api-fixtures-npm/node_modules node fixtures/agent-generated/generate.cjs
// Dependencies: docx@9.5.1 exceljs@4.4.0 pptxgenjs@4.0.1 (outside repository).
const fs = require('node:fs');
const path = require('node:path');
const d = require('docx');
const ExcelJS = require('exceljs');
const PptxGenJS = require('pptxgenjs');
(async () => {
  const image = fs.readFileSync(path.join(__dirname, 'sample.png'));
  const doc = new d.Document({ sections: [{ children: [
    new d.Paragraph({ text: 'Agent fixture', heading: d.HeadingLevel.TITLE }),
    new d.Paragraph('Editable source text.'),
    new d.Paragraph({ text: 'Generated content', heading: d.HeadingLevel.HEADING_1 }),
    ...['First bullet', 'Second bullet'].map(text => new d.Paragraph({ text, bullet: { level: 0 } })),
    new d.Table({ rows: [['Name', 'Value'], ['Alpha', '42']].map(row => new d.TableRow({ children: row.map(text => new d.TableCell({ children: [new d.Paragraph(text)] })) })) }),
    new d.Paragraph({ children: [new d.ImageRun({ type: 'png', data: image, transformation: { width: 120, height: 60 } })] }),
  ] }] });
  fs.writeFileSync(path.join(__dirname, 'docx.docx'), await d.Packer.toBuffer(doc));
  const w = new ExcelJS.Workbook();
  const s = w.addWorksheet('Summary');
  s.addRows([['Agent fixture', 'Value'], ['Alpha', 42], ['Total', { formula: 'SUM(B2:B2)', result: 42 }]]);
  s.getCell('A1').font = { bold: true, color: { argb: 'FF245BA8' } };
  s.getCell('B2').fill = { type: 'pattern', pattern: 'solid', fgColor: { argb: 'FFF3BD42' } };
  w.addWorksheet('Details').addRow(['Second sheet', 7]);
  await w.xlsx.writeFile(path.join(__dirname, 'exceljs.xlsx'));
  const p = new PptxGenJS();
  const slide = p.addSlide();
  slide.addText('Agent fixture', { x: 1, y: .3, w: 6, h: .5, fontSize: 28 });
  slide.addText([{ text: 'First bullet', options: { bullet: true, breakLine: true } }, { text: 'Second bullet', options: { bullet: true } }], { x: 1, y: 1, w: 5, h: 1 });
  slide.addImage({ path: path.join(__dirname, 'sample.png'), x: 7, y: 1, w: 2, h: 1 });
  slide.addTable([['Name', 'Value'], ['Alpha', '42']], { x: 1, y: 3, w: 5, h: 1 });
  p.addSlide();
  await p.writeFile({ fileName: path.join(__dirname, 'pptxgenjs.pptx') });
  console.log('Generated docx.docx, exceljs.xlsx, pptxgenjs.pptx');
})().catch(e => { console.error(e); process.exitCode = 1; });
