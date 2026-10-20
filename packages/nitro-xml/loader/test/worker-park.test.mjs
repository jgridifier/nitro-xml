// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { takePrewarmedWorker } from "../bootstrap.js";
import { patch } from "../lazy/wp.js";

describe("worker park / edit coalesce", () => {
  it("parks one worker and hands it back on takePrewarmedWorker", () => {
    // Catches: remount terminating the worker so React remount pays spin-up again.
    const fake = { terminate() { this.dead = true; } };
    takePrewarmedWorker(fake);
    assert.equal(takePrewarmedWorker(), fake);
    assert.equal(takePrewarmedWorker(), null);
  });

  it("patches applyEdit to coalesce setCellInput by cell key", async () => {
    // Catches: typing a cell firing one worker round-trip per keystroke in the same frame.
    if (typeof requestAnimationFrame !== "function") {
      globalThis.requestAnimationFrame = (fn) => setTimeout(fn, 0);
    }
    let calls = 0;
    class FakeDoc {
      applyEdit(spec) {
        calls++;
        return Promise.resolve(spec);
      }
    }
    patch(FakeDoc);
    const d = new FakeDoc();
    const a = d.applyEdit('{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"a"}');
    const b = d.applyEdit('{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"ab"}');
    const [ra, rb] = await Promise.all([a, b]);
    assert.equal(calls, 1);
    assert.equal(ra, rb);
    assert.match(rb, /ab/);
  });
});
