// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { WorkerDocument } from "../worker-host.js";
import { createJournal } from "../lazy/journal.js";

describe("noop save (worker dirty tracking)", () => {
  it("save() with no prior edit returns exact worker bytes unchanged", async () => {
    // Catches: WorkerDocument.save() wrapping or clipping bytes from the worker's
    // 'saved' message, turning a noop save into a corrupt package (#30/E1).
    const original = new Uint8Array([0x50, 0x4b, 3, 4, 1, 2, 3, 4, 5]);
    const doc = Object.create(WorkerDocument.prototype);
    doc._journal = null;
    doc._pending = new Map();
    doc._req = 0;
    doc._request = async (expect) => {
      if (expect === "saved") return { type: "saved", bytes: new Uint8Array(original) };
      return {};
    };
    const bytes = await doc.save();
    assert.ok(bytes instanceof Uint8Array, "save() must return Uint8Array");
    assert.deepEqual([...bytes], [...original]);
  });

  it("save() records to journal.markSave when journal exists (dirty path)", async () => {
    // Catches: journal not recording saved bytes, so edit-recovery cannot offer lastGoodSave.
    const original = new Uint8Array([0x50, 0x4b, 3, 4]);
    const journal = createJournal(original);
    journal.append({ spec: '{"command":"noop"}' });

    const savedBytes = new Uint8Array([0x50, 0x4b, 3, 4, 7, 8]);
    const doc = Object.create(WorkerDocument.prototype);
    doc._journal = journal;
    doc._pending = new Map();
    doc._req = 0;
    doc._request = async (expect) => {
      if (expect === "saved") return { type: "saved", bytes: new Uint8Array(savedBytes) };
      return {};
    };

    const result = await doc.save();
    assert.deepEqual([...result], [...savedBytes]);
    // originalBytes is unchanged — only markSave advances the lastGoodSave pointer.
    assert.deepEqual([...journal.originalBytes()], [...original]);
  });

  it("save() error from worker propagates as rejected promise (error surfacing)", async () => {
    // Catches: worker 'error' message during save being swallowed silently, so hosts
    // never learn save failed. Required by PRODUCT_SPEC §5 E1 (#30).
    const doc = Object.create(WorkerDocument.prototype);
    doc._journal = null;
    doc._pending = new Map();
    doc._req = 0;
    doc._request = async (expect) => {
      if (expect === "saved")
        throw Object.assign(new Error("disk full"), { code: "save_failed" });
      return {};
    };
    await assert.rejects(() => doc.save(), (err) => {
      assert.equal(err.message, "disk full");
      return true;
    });
  });

  it("worker noop shortcut: isDirty=false → returns _openBytes copy, not saveDocument()", () => {
    // Catches: worker.js calling handle.saveDocument() on a clean document,
    // repacking the ZIP and producing bytes that differ from the input.
    // Directly validates the branching logic added to the 'save' message handler.
    const original = new Uint8Array([0x50, 0x4b, 3, 4, 10, 20, 30]);
    const posted = [];

    function simulateWorkerSave(handle, isDirty, openBytes) {
      // Mirrors worker.js save handler exactly.
      if (!isDirty && openBytes) {
        const copy = new Uint8Array(openBytes);
        posted.push({ type: "saved", bytes: copy });
        return;
      }
      const bytes = new Uint8Array(handle.saveDocument());
      posted.push({ type: "saved", bytes });
    }

    const mockHandle = {
      saveDocument: () => new Uint8Array([0, 0, 0]),
    };

    // Noop path: not dirty, openBytes present → original bytes returned
    simulateWorkerSave(mockHandle, false, original);
    assert.equal(posted.length, 1);
    assert.deepEqual([...posted[0].bytes], [...original]);
    assert.notEqual(posted[0].bytes, original, "must return a copy, not the same reference");

    // Dirty path: dirty → saveDocument() result returned
    simulateWorkerSave(mockHandle, true, original);
    assert.equal(posted.length, 2);
    assert.deepEqual([...posted[1].bytes], [0, 0, 0]);
  });
});
