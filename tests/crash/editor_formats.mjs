// SPDX-License-Identifier: MIT
/**
 * Multi-format editor crash helpers (#30): open / apply / fingerprint / undo-all
 * against the pinned TS oracle. Test-only — not shipped.
 */

import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { applyDocxOp, applyPptxOps, loadPptxMutators, withStableRandom } from "../../parity/edit/apply.mjs";
import {
  snapshotXlsx,
  snapshotDocx,
  snapshotPptx,
  stableStringify,
  snapshotsEqual,
} from "../../parity/undo/snapshot.mjs";
import {
  applyXlsxOp,
  openXlsxSession,
  parseRange,
  saveXlsxSession,
} from "./ts_xlsx.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");

function upstreamRoot() {
  return process.env.UPSTREAM_DIR || join(ROOT, "upstream");
}

function toArrayBuffer(u8) {
  return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
}

function cloneModel(v) {
  return structuredClone(v);
}

function restoreDocxModel(model, snapModel) {
  const keys = new Set([...Object.keys(model), ...Object.keys(snapModel)]);
  for (const k of keys) {
    if (k in snapModel) model[k] = cloneModel(snapModel[k]);
    else delete model[k];
  }
}

export async function openSession(format, originalBytes) {
  if (format === "xlsx") {
    return { format, session: await openXlsxSession(originalBytes), pr: await parseRange() };
  }
  if (format === "docx") {
    const up = upstreamRoot();
    const { loadDocx } = await import(`${up}/src/core/docx/parse.ts`);
    const loaded = await loadDocx(originalBytes);
    return {
      format,
      loaded,
      model: loaded.model,
      past: [],
      startSnap: snapshotDocx(loaded.model),
    };
  }
  if (format === "pptx") {
    const up = upstreamRoot();
    const { PptxHandler } = await import(`${up}/src/core/pptx/core/PptxHandler.ts`);
    const { cloneSlides } = await import(`${up}/src/core/pptx/editor/render/editor-mutations.ts`);
    const { EditorHistory } = await import(`${up}/src/core/pptx/editor/render/editor-history.ts`);
    const mut = await loadPptxMutators();
    const handler = new PptxHandler();
    const data = await handler.load(toArrayBuffer(originalBytes));
    const history = new EditorHistory({ maxDepth: 200 });
    return {
      format,
      handler,
      data,
      slides: data.slides,
      history,
      mut,
      cloneSlides,
      startSnap: snapshotPptx(data.slides),
    };
  }
  throw new Error(`unsupported format ${format}`);
}

export async function applyOp(ctx, op) {
  if (ctx.format === "xlsx") {
    await applyXlsxOp(ctx.session, op, ctx.pr);
    return;
  }
  if (ctx.format === "docx") {
    ctx.past.push({ model: cloneModel(ctx.model), label: op.op });
    applyDocxOp(ctx.model, op);
    return;
  }
  if (ctx.format === "pptx") {
    ctx.history.record(ctx.cloneSlides(ctx.slides), op.op || "edit");
    ctx.slides = applyPptxOps(ctx.slides, [op], ctx.mut);
    return;
  }
  throw new Error(`unsupported format ${ctx.format}`);
}

export function fingerprint(ctx) {
  if (ctx.format === "xlsx") return stableStringify(snapshotXlsx(ctx.session.workbook));
  if (ctx.format === "docx") return stableStringify(snapshotDocx(ctx.model));
  if (ctx.format === "pptx") return stableStringify(snapshotPptx(ctx.slides));
  throw new Error(`unsupported format ${ctx.format}`);
}

export function canUndo(ctx) {
  if (ctx.format === "xlsx") {
    return typeof ctx.session.canUndo === "function"
      ? ctx.session.canUndo()
      : !!ctx.session.canUndo;
  }
  if (ctx.format === "docx") return ctx.past.length > 0;
  if (ctx.format === "pptx") return !!ctx.history.canUndo;
  return false;
}

export function undoOnce(ctx) {
  if (ctx.format === "xlsx") {
    ctx.session.undo();
    return;
  }
  if (ctx.format === "docx") {
    const entry = ctx.past.pop();
    if (!entry) throw new Error("docx undo underflow");
    restoreDocxModel(ctx.model, entry.model);
    return;
  }
  if (ctx.format === "pptx") {
    const r = ctx.history.undo(ctx.cloneSlides(ctx.slides));
    if (!r) throw new Error("pptx undo underflow");
    ctx.slides = r.snapshot;
    return;
  }
  throw new Error(`unsupported format ${ctx.format}`);
}

export async function saveSession(ctx) {
  if (ctx.format === "xlsx") return saveXlsxSession(ctx.session);
  if (ctx.format === "docx") {
    if (!ctx.loaded?.save) return new Uint8Array([1]);
    return new Uint8Array(await ctx.loaded.save(ctx.model));
  }
  if (ctx.format === "pptx") {
    return new Uint8Array(
      await withStableRandom(() =>
        ctx.handler.save(ctx.slides, {
          headerFooter: ctx.data.headerFooter,
          tableStyles: ctx.data.tableStyleMap,
        }),
      ),
    );
  }
  throw new Error(`unsupported format ${ctx.format}`);
}

export function startFingerprint(ctx) {
  if (ctx.format === "xlsx") {
    // Captured after open, before ops — caller should snapshot first.
    return null;
  }
  if (ctx.format === "docx") return stableStringify(ctx.startSnap);
  if (ctx.format === "pptx") return stableStringify(ctx.startSnap);
  return null;
}

export { snapshotsEqual, snapshotXlsx };
