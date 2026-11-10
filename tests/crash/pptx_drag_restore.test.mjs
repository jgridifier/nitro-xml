// SPDX-License-Identifier: MIT
// Catches: patchShapeGeometry journaled but not replayed to the dragged target position
// on crash-recovery — shape reverts to its pre-drag coordinates after recovery (#30/E4).
// A separate file from run.mjs to avoid conflicts with m1-30-fuzz-crash additions to
// crash_recovery.sh.
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import { createJournal } from "../../packages/nitro-xml/loader/lazy/journal.js";
import { openSession, applyOp, fingerprint } from "./editor_formats.mjs";
import { snapshotPptx } from "../../parity/undo/snapshot.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");
// slides_10.pptx: slide 0 has at least one shape element (confirmed by pptx-move-s0 edit script).
const FIXTURE = join(ROOT, "fixtures/generated/pptx/slides_10.pptx");

// Use the same x/y values as the pptx-move-s0 edit script so the fixture's first element
// is known to have an element at index 0 that accepts this geometry change.
const DRAG_OP = {
  op: "patchShapeGeometry",
  slideIndex: 0,
  elementIndex: 0,
  x: 120,
  y: 80,
};

async function main() {
  const originalBytes = new Uint8Array(readFileSync(FIXTURE));

  // 1. Open a live session and apply the drag, capture expected state.
  const live = await openSession("pptx", originalBytes);
  await applyOp(live, DRAG_OP);
  const expectedFp = fingerprint(live);

  // 2. Build a journal containing only the drag op (simulates the committed journal).
  const journal = createJournal(originalBytes);
  journal.append(DRAG_OP);

  // 3. Simulate crash: live session is dropped; only journal survives.

  // 4. Recover: open original in a fresh session and replay journal ops.
  const res = await journal.recover({
    openFresh: (bytes) => openSession("pptx", bytes),
    applyOps: (session, op) => applyOp(session, op),
    fingerprint: (session) => fingerprint(session),
    expectedFingerprint: expectedFp,
  });

  assert.ok(res.ok, `journal replay failed: ${res.error}`);

  // 5. Verify restored geometry — not just that recovery succeeded, but that the shape
  //    is at the dragged coordinates, not the original position.
  const snap = snapshotPptx(res.session.slides);
  const el = snap.slides[0]?.elements[0];
  assert.ok(el, "slide 0 element 0 must exist after recovery");
  assert.strictEqual(
    el.x,
    DRAG_OP.x,
    `restored x should be ${DRAG_OP.x} (drag target), got ${el.x}`,
  );
  assert.strictEqual(
    el.y,
    DRAG_OP.y,
    `restored y should be ${DRAG_OP.y} (drag target), got ${el.y}`,
  );

  console.log(
    `pptx-drag-restore: OK — element 0 restored to x=${el.x} y=${el.y} after simulated crash`,
  );
}

main().catch((e) => {
  console.error("pptx-drag-restore: FAIL", e);
  process.exit(1);
});
