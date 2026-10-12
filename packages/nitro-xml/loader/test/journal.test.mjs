// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createJournal } from "../journal.js";

describe("edit op journal", () => {
  it("replays committed ops into a fresh session after a simulated crash", async () => {
    // Catches: recover skipping ops or mutating the frozen original bytes snapshot.
    const original = new Uint8Array([1, 2, 3]);
    const journal = createJournal(original);
    const opA = { op: "setCellInput", sheet: 0, row: 0, col: 0, text: "a" };
    const opB = { op: "setCellInput", sheet: 0, row: 0, col: 1, text: "b" };
    journal.append(opA);
    journal.markSave(new Uint8Array([9]));
    journal.append(opB);
    const log = [];
    const res = await journal.recover({
      openFresh: (bytes) => {
        assert.deepEqual([...bytes], [1, 2, 3]);
        return { id: log.length };
      },
      applyOps: (_session, op) => {
        log.push(op.text);
      },
      fingerprint: () => "fp-2",
      expectedFingerprint: "fp-2",
    });
    assert.equal(res.ok, true);
    assert.deepEqual(log, ["a", "b"]);
    assert.deepEqual([...journal.originalBytes()], [1, 2, 3]);
  });

  it("fails closed with lastGoodSave when replay fingerprint does not match", async () => {
    // Catches: corrupted journal still reporting ok:true without offering lastGoodSave.
    const journal = createJournal(new Uint8Array([0]));
    journal.append({ op: "setCellInput", sheet: 0, row: 0, col: 0, text: "x" });
    journal.markSave(new Uint8Array([7, 7]));
    const res = await journal.recover({
      openFresh: () => ({}),
      applyOps: () => {},
      fingerprint: () => "bad",
      expectedFingerprint: "expected",
    });
    assert.equal(res.ok, false);
    assert.deepEqual([...res.lastGoodSave], [7, 7]);
  });
});
