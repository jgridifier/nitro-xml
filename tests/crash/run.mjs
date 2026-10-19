// SPDX-License-Identifier: MIT
/**
 * Crash-recovery gate harness (#62): journal replay vs TS edit oracle with injection.
 *
 * Usage: bun tests/crash/run.mjs [--ci]
 */

import { createJournal } from "../../packages/nitro-xml/loader/lazy/journal.js";
import { loadEditScripts, resolveFixture, fixtureAbs } from "../../parity/edit/scripts.mjs";
import {
  applyXlsxOp,
  fingerprintAfterOps,
  fingerprintXlsxSession,
  openXlsxSession,
  parseRange,
  readFixtureBytes,
  saveXlsxSession,
} from "./ts_xlsx.mjs";

const SCRIPT_ID = "xlsx-multi-sheet-ops";

function crashIndex(seed, opCount) {
  return seed % opCount;
}

async function runHappyPath(script, originalBytes, crashAt, pr) {
  const prefixOps = script.ops.slice(0, crashAt + 1);
  const expectedFp = await fingerprintAfterOps(originalBytes, prefixOps);

  const journal = createJournal(originalBytes);
  let live = await openXlsxSession(originalBytes);
  for (let i = 0; i <= crashAt; i++) {
    const op = script.ops[i];
    await applyXlsxOp(live, op, pr);
    journal.append(op);
    if (i === Math.max(0, crashAt - 1)) {
      journal.markSave(await saveXlsxSession(live));
    }
  }
  live = null;

  const res = await journal.recover({
    openFresh: openXlsxSession,
    applyOps: (session, op) => applyXlsxOp(session, op, pr),
    fingerprint: (session) => fingerprintXlsxSession(session, prefixOps),
    expectedFingerprint: expectedFp,
  });
  if (!res.ok) {
    throw new Error(`recover failed after crashAt=${crashAt}: ${res.error}`);
  }
  return journal;
}

async function runNegativeControl(journal, script, originalBytes, crashAt, pr) {
  const prefixOps = script.ops.slice(0, crashAt + 1);
  const expectedFp = await fingerprintAfterOps(originalBytes, prefixOps);

  const snap = journal.snapshot();
  if (snap.ops.length < 1) throw new Error("negative control needs committed ops");
  const corruptedOps = snap.ops.slice(0, -1);
  const bad = createJournal(snap.originalBytes, {
    ops: corruptedOps,
    lastGoodSave: snap.lastGoodSave,
  });
  const res = await bad.recover({
    openFresh: openXlsxSession,
    applyOps: (session, op) => applyXlsxOp(session, op, pr),
    fingerprint: (session) => fingerprintXlsxSession(session, prefixOps),
    expectedFingerprint: expectedFp,
  });
  if (res.ok) {
    console.error("crash-recovery: NEGATIVE CONTROL BROKEN (dropped op still recovered)");
    process.exit(1);
  }
  if (!res.lastGoodSave || res.lastGoodSave.length === 0) {
    console.error("crash-recovery: NEGATIVE CONTROL BROKEN (lastGoodSave missing)");
    process.exit(1);
  }
}

async function main() {
  const scripts = loadEditScripts({ id: SCRIPT_ID });
  if (scripts.length !== 1) {
    throw new Error(`expected one script ${SCRIPT_ID}, got ${scripts.length}`);
  }
  const script = scripts[0];
  if (script.format !== "xlsx") throw new Error("harness expects xlsx script");
  const row = resolveFixture(script);
  const originalBytes = readFixtureBytes(fixtureAbs(row));
  const seed = Number(process.env.NITRO_CRASH_SEED || "62");
  const crashAt = crashIndex(seed, script.ops.length);
  const pr = await parseRange();
  console.log(
    `crash-recovery: script=${script.id} ops=${script.ops.length} crashAfterOpIndex=${crashAt} seed=${seed}`,
  );

  const journal = await runHappyPath(script, originalBytes, crashAt, pr);
  await runNegativeControl(journal, script, originalBytes, crashAt, pr);
  console.log("crash-recovery: editor journal happy path + negative control OK");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
