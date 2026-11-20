// SPDX-License-Identifier: MIT
const ExcelJS = require('exceljs');
(async () => {
  const workbook = new ExcelJS.Workbook();
  await workbook.xlsx.readFile(process.argv[2]);
  let found = false;
  workbook.eachSheet(sheet => sheet.eachRow(row => row.eachCell(cell => { if (cell.text.includes(process.argv[3])) found = true; })));
  if (!found) throw new Error('Edited text missing after ExcelJS reopen');
  console.log('PASS ExcelJS source-library reopen');
})().catch(error => { console.error(error); process.exitCode = 1; });
