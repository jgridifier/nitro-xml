// SPDX-License-Identifier: MIT
/**
 * TS oracle helpers for crash-recovery harness (#62). Test-only — not shipped.
 */

import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");

function upstreamRoot() {
  return process.env.UPSTREAM_DIR || join(ROOT, "upstream");
}

let _mods;
async function mods() {
  if (_mods) return _mods;
  const up = upstreamRoot();
  _mods = {
    loadXlsx: (await import(`${up}/src/core/xlsx/read/load.ts`)).loadXlsx,
    saveXlsx: (await import(`${up}/src/core/xlsx/write/save.ts`)).saveXlsx,
    createEditSession: (await import(`${up}/src/core/xlsx/edit/session.ts`)).createEditSession,
    parseRange: (await import(`${up}/src/core/xlsx/address.ts`)).parseRange,
    getCell: (await import(`${up}/src/core/xlsx/cells.ts`)).getCell,
    styleAt: (await import(`${up}/src/core/xlsx/styles.ts`)).styleAt,
  };
  return _mods;
}

function collectTouched(ops, out, parseRange) {
  for (const op of ops) {
    switch (op.op) {
      case "setCellInput":
      case "setCellValue":
        out.add(`${op.sheet},${op.row},${op.col}`);
        break;
      case "applyStyle":
      case "applyCellStyle": {
        const range = parseRange(op.range);
        if (!range) throw new Error(`bad range ${op.range}`);
        const r = {
          start: {
            row: Math.min(range.start.row, range.end.row),
            col: Math.min(range.start.col, range.end.col),
          },
          end: {
            row: Math.max(range.start.row, range.end.row),
            col: Math.max(range.start.col, range.end.col),
          },
        };
        for (let row = r.start.row; row <= r.end.row; row++) {
          for (let col = r.start.col; col <= r.end.col; col++) {
            out.add(`${op.sheet},${row},${col}`);
          }
        }
        break;
      }
      case "batch":
        collectTouched(op.ops || [], out, parseRange);
        break;
      default:
        break;
    }
  }
}

export async function applyXlsxOp(session, op, parseRange) {
  switch (op.op) {
    case "setCellInput":
      session.setCellInput(op.sheet, op.row, op.col, op.text);
      break;
    case "setCellValue":
      session.setCellValue(op.sheet, op.row, op.col, op.value);
      break;
    case "applyStyle": {
      const range = parseRange(op.range);
      if (!range) throw new Error(`bad range ${op.range}`);
      session.applyStyle(op.sheet, [range], op.patch);
      break;
    }
    case "applyCellStyle": {
      const range = parseRange(op.range);
      if (!range) throw new Error(`bad range ${op.range}`);
      session.applyCellStyle(op.sheet, [range], op.name);
      break;
    }
    case "addSheet":
      session.addSheet(op.name, op.at);
      break;
    case "renameSheet":
      session.renameSheet(op.index, op.name);
      break;
    case "deleteSheet":
      session.deleteSheet(op.index);
      break;
    case "insertRows":
      session.insertRows(op.sheet, op.at, op.count);
      break;
    case "deleteRows":
      session.deleteRows(op.sheet, op.at, op.count);
      break;
    case "insertColumns":
      session.insertColumns(op.sheet, op.at, op.count);
      break;
    case "deleteColumns":
      session.deleteColumns(op.sheet, op.at, op.count);
      break;
    case "batch":
      session.batch(op.label || "batch", () =>
        applyXlsxOpsList(session, op.ops || [], parseRange),
      );
      break;
    default:
      throw new Error(`unsupported xlsx op: ${op.op}`);
  }
}

async function applyXlsxOpsList(session, ops, parseRange) {
  for (const op of ops) {
    await applyXlsxOp(session, op, parseRange);
  }
}

export async function openXlsxSession(originalBytes) {
  const m = await mods();
  const workbook = await m.loadXlsx(originalBytes);
  return m.createEditSession(workbook, { recalc: false });
}

export async function fingerprintXlsxSession(session, scriptOps) {
  const m = await mods();
  const touched = new Set();
  collectTouched(scriptOps, touched, m.parseRange);
  const cells = [];
  const keys = [...touched].sort((a, b) => {
    const [as, ar, ac] = a.split(",").map(Number);
    const [bs, br, bc] = b.split(",").map(Number);
    return as - bs || ar - br || ac - bc;
  });
  for (const key of keys) {
    const [sheet, r, c] = key.split(",").map(Number);
    const ws = session.workbook.sheets[sheet];
    const cell = ws ? m.getCell(ws, r, c) : undefined;
    if (!cell) {
      cells.push({ sheet, r, c, v: null });
      continue;
    }
    const entry = {
      sheet,
      r,
      c,
      v: cell.value === undefined ? null : cell.value,
      style: m.styleAt(session.workbook, cell.styleId),
    };
    if (cell.formula !== undefined) entry.f = cell.formula;
    if (cell.styleId !== undefined) entry.s = cell.styleId;
    cells.push(entry);
  }
  const snap = {
    cells,
    stylesCount: session.workbook.styles.length,
    sheets: session.workbook.sheets.map((s) => ({
      name: s.name,
      sheetId: s.sheetId,
      state: s.state,
    })),
    activeSheet: session.workbook.activeSheet,
  };
  return JSON.stringify(snap);
}

export async function saveXlsxSession(session) {
  const m = await mods();
  return new Uint8Array(await m.saveXlsx(session.workbook));
}

export function readFixtureBytes(absPath) {
  return new Uint8Array(readFileSync(absPath));
}

export async function parseRange() {
  const m = await mods();
  return m.parseRange;
}

export async function fingerprintAfterOps(originalBytes, ops) {
  const session = await openXlsxSession(originalBytes);
  const pr = await parseRange();
  for (const op of ops) {
    await applyXlsxOp(session, op, pr);
  }
  return fingerprintXlsxSession(session, ops);
}
