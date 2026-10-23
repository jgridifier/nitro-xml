// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { WorkerDocument } from "../worker-host.js";

describe("defer host originalBytes until ensure", () => {
  it("open leaves _originalBytes null until _ensureOriginalBytes", async () => {
    // Catches: writable open still cloning the zip on the host before transfer to the worker.
    const doc = Object.create(WorkerDocument.prototype);
    doc._originalBytes = null;
    doc._ro = false;
    doc._worker = {
      postMessage() {},
    };
    doc._pending = new Map();
    doc._req = 0;
    let posted = false;
    doc._request = async (expect, send) => {
      assert.equal(expect, "original-bytes");
      send();
      posted = true;
      return { type: "original-bytes", bytes: new Uint8Array([1, 2, 3]) };
    };
    const bytes = await doc._ensureOriginalBytes();
    assert.ok(posted);
    assert.deepEqual([...bytes], [1, 2, 3]);
    assert.deepEqual([...doc._originalBytes], [1, 2, 3]);
    const again = await doc._ensureOriginalBytes();
    assert.equal(again, doc._originalBytes);
  });

  it("_ej does not require pre-set _originalBytes on the host", async () => {
    // Catches: deferred snapshot breaking journal creation on first edit.
    const doc = Object.create(WorkerDocument.prototype);
    doc._ro = false;
    doc._journal = null;
    doc._originalBytes = null;
    doc._ensureOriginalBytes = async () => {
      doc._originalBytes = new Uint8Array([9, 9]);
      return doc._originalBytes;
    };
    await doc._ej();
    assert.ok(doc._journal);
    assert.deepEqual([...doc._journal.originalBytes()], [9, 9]);
  });
});
