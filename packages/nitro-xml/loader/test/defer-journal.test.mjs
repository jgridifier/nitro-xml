// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { WorkerDocument } from "../worker-host.js";

describe("defer journal until first edit", () => {
  it("creates the journal from originalBytes only when ensured", async () => {
    // Catches: writable open still calling createJournal (second zip copy) before first paint.
    const doc = Object.create(WorkerDocument.prototype);
    doc._ro = false;
    doc._originalBytes = new Uint8Array([7, 8, 9]);
    doc._journal = null;
    assert.equal(doc._journal, null);
    await doc._ej();
    assert.ok(doc._journal);
    assert.deepEqual([...doc._journal.originalBytes()], [7, 8, 9]);
    const again = doc._journal;
    await doc._ej();
    assert.equal(doc._journal, again);
  });

  it("skips journal creation when read-only or bytes missing", async () => {
    // Catches: readonly open / recover path allocating a journal with no original bytes.
    const ro = Object.create(WorkerDocument.prototype);
    ro._ro = true;
    ro._originalBytes = new Uint8Array([1]);
    ro._journal = null;
    await ro._ej();
    assert.equal(ro._journal, null);

    const empty = Object.create(WorkerDocument.prototype);
    empty._ro = false;
    empty._originalBytes = null;
    empty._journal = null;
    empty._ensureOriginalBytes = async () => null;
    await empty._ej();
    assert.equal(empty._journal, null);
  });
});
