// SPDX-License-Identifier: MIT
/**
 * Crash-recovery gate harness (#30 / #62): kill mid-edit, journal replay,
 * restore unsaved edits + undo stack for xlsx / docx / pptx (TS oracle).
 *
 * Usage:
 *   bun tests/crash/run.mjs [--ci]
 *   NITRO_CRASH_FLAKE_RUNS=10 bun tests/crash/run.mjs --ci
 */

import { createJournal } from "../../packages/nitro-xml/loader/journal.js";
import { loadEditScripts, resolveFixture, fixtureAbs } from "../../parity/edit/scripts.mjs";
import { snapshotXlsx, stableStringify } from "../../parity/undo/snapshot.mjs";
import {
  applyOp,
  canUndo,
  fingerprint,
  openSession,
  saveSession,
  undoOnce,
} from "./editor_formats.mjs";
import { readFixtureBytes } from "./ts_xlsx.mjs";

/** One multi-op script per format (deterministic crash injection). */
const SCRIPT_IDS = [
  "xlsx-multi-sheet-ops",
  "docx-text-and-format",
  "pptx-text-and-style",
];

function crashIndex(seed, opCount, formatSalt) {
  const n = Math.max(1, opCount);
  return (seed + formatSalt) % n;
}

function formatSalt(format) {
  if (format === "xlsx") return 0;
  if (format === "docx") return 17;
  if (format === "pptx") return 29;
  return 0;
}

async function fingerprintAfterOps(format, originalBytes, ops) {
  const ctx = await openSession(format, originalBytes);
  for (const op of ops) await applyOp(ctx, op);
  return fingerprint(ctx);
}

async function runHappyPath(script, originalBytes, crashAt) {
  const prefixOps = script.ops.slice(0, crashAt + 1);
  const expectedFp = await fingerprintAfterOps(script.format, originalBytes, prefixOps);

  const journal = createJournal(originalBytes);
  let live = await openSession(script.format, originalBytes);
  const startFp =
    script.format === "xlsx"
      ? stableStringify(snapshotXlsx(live.session.workbook))
      : fingerprint(live);

  for (let i = 0; i <= crashAt; i++) {
    const op = script.ops[i];
    await applyOp(live, op);
    journal.append(op);
    if (i === Math.max(0, crashAt - 1)) {
      journal.markSave(await saveSession(live));
    }
  }
  // Simulate worker / WASM kill mid-edit: drop the live session.
  live = null;

  const res = await journal.recover({
    openFresh: (bytes) => openSession(script.format, bytes),
    applyOps: (session, op) => applyOp(session, op),
    fingerprint: (session) => fingerprint(session),
    expectedFingerprint: expectedFp,
  });
  if (!res.ok) {
    throw new Error(
      `${script.id}: recover failed after crashAt=${crashAt}: ${res.error}`,
    );
  }

  // Unsaved edits restored.
  const recoveredFp = fingerprint(res.session);
  if (recoveredFp !== expectedFp) {
    throw new Error(`${script.id}: recovered fingerprint mismatch`);
  }

  // Undo stack restored: undo-all returns to the pre-edit start state.
  let undoSteps = 0;
  while (canUndo(res.session)) {
    undoOnce(res.session);
    undoSteps++;
  }
  if (undoSteps < 1) {
    throw new Error(`${script.id}: expected undo stack after replay, got 0 steps`);
  }
  const afterUndoFp = fingerprint(res.session);
  if (afterUndoFp !== startFp) {
    throw new Error(
      `${script.id}: undo-all after recovery did not restore start (steps=${undoSteps})`,
    );
  }

  return journal;
}

async function runNegativeControl(journal, script, originalBytes, crashAt) {
  const prefixOps = script.ops.slice(0, crashAt + 1);
  const expectedFp = await fingerprintAfterOps(script.format, originalBytes, prefixOps);
  const snap = journal.snapshot();
  if (snap.ops.length < 1) throw new Error("negative control needs committed ops");
  const bad = createJournal(snap.originalBytes, {
    ops: snap.ops.slice(0, -1),
    lastGoodSave: snap.lastGoodSave,
  });
  const res = await bad.recover({
    openFresh: (bytes) => openSession(script.format, bytes),
    applyOps: (session, op) => applyOp(session, op),
    fingerprint: (session) => fingerprint(session),
    expectedFingerprint: expectedFp,
  });
  if (res.ok) {
    console.error(`crash-recovery: NEGATIVE CONTROL BROKEN (${script.id})`);
    process.exit(1);
  }
  if (!res.lastGoodSave || res.lastGoodSave.length === 0) {
    console.error(`crash-recovery: NEGATIVE CONTROL BROKEN lastGoodSave (${script.id})`);
    process.exit(1);
  }
}

async function runOnce(seed) {
  for (const id of SCRIPT_IDS) {
    const scripts = loadEditScripts({ id });
    if (scripts.length !== 1) {
      throw new Error(`expected one script ${id}, got ${scripts.length}`);
    }
    const script = scripts[0];
    const row = resolveFixture(script);
    const originalBytes = readFixtureBytes(fixtureAbs(row));
    const crashAt = crashIndex(seed, script.ops.length, formatSalt(script.format));
    console.log(
      `crash-recovery: script=${script.id} format=${script.format} ops=${script.ops.length} crashAfterOpIndex=${crashAt} seed=${seed}`,
    );
    const journal = await runHappyPath(script, originalBytes, crashAt);
    await runNegativeControl(journal, script, originalBytes, crashAt);
    console.log(`crash-recovery: ${script.id} happy path + undo + negative control OK`);
  }
}

async function main() {
  const seed = Number(process.env.NITRO_CRASH_SEED || "62");
  const runs = Math.max(1, Number(process.env.NITRO_CRASH_FLAKE_RUNS || "1") | 0);
  for (let i = 0; i < runs; i++) {
    // Vary the seed per flake iteration while staying deterministic.
    const runSeed = (seed + i * 97) >>> 0;
    if (runs > 1) console.log(`crash-recovery: flake run ${i + 1}/${runs} (seed=${runSeed})`);
    await runOnce(runSeed);
  }
  console.log(
    `crash-recovery: editor journal multi-format OK (formats=3 runs=${runs} baseSeed=${seed})`,
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
