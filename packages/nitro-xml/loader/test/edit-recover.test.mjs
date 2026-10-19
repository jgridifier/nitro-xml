// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { createJournal } from "../lazy/journal.js";
import {
  EDITOR_RECOVERY_FAILED_MESSAGE,
  EDITOR_RECOVERY_LAST_SAVE_MESSAGE,
  viewerOpenErrorMessage,
} from "../formats.js";

describe("edit recover (journal into fresh worker)", () => {
  it("replays committed applyEdit specs after a simulated worker kill", async () => {
    // Catches: recover skipping journaled applyEdit specs or mutating original bytes.
    const original = new Uint8Array([1, 2, 3]);
    const journal = createJournal(original);
    journal.append({ spec: '{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"a"}' });
    journal.markSave(new Uint8Array([9]));
    journal.append({ spec: '{"command":"setCellInput","sheet":0,"row":0,"col":1,"text":"b"}' });

    const applied = [];
    let opened = null;
    const res = await journal.recover({
      openFresh: async (bytes) => {
        opened = bytes;
        return { id: "fresh" };
      },
      applyOps: async (_session, op) => {
        applied.push(op.spec);
      },
    });
    assert.equal(res.ok, true);
    assert.deepEqual([...opened], [1, 2, 3]);
    assert.equal(applied.length, 2);
    assert.match(applied[0], /"text":"a"/);
    assert.match(applied[1], /"text":"b"/);
    assert.deepEqual([...journal.originalBytes()], [1, 2, 3]);
  });

  it("maps failed replay with lastGoodSave to the graceful editor message", () => {
    // Catches: blank/hung viewer when journal replay fails but a prior save exists (#30).
    assert.equal(
      viewerOpenErrorMessage({ code: "editor_recovery_last_save" }),
      EDITOR_RECOVERY_LAST_SAVE_MESSAGE,
    );
    assert.equal(
      viewerOpenErrorMessage({ code: "editor_recovery_failed" }),
      EDITOR_RECOVERY_FAILED_MESSAGE,
    );
    assert.equal(
      viewerOpenErrorMessage({ code: "editor_recovery_exhausted" }),
      EDITOR_RECOVERY_FAILED_MESSAGE,
    );
  });
});
