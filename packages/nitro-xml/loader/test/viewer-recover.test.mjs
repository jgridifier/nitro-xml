// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  recoverViewer,
  VIEWER_RECOVERY_FAILED_MESSAGE,
} from "./viewer-recover-lib.mjs";

describe("viewer recover (reload once)", () => {
  it("opens frozen originals in a fresh session when reload has not been tried yet", async () => {
    // Catches: recoverViewer mutating the retained snapshot or skipping openFresh after a crash.
    const original = new Uint8Array([4, 5, 6]);
    let opens = 0;
    const res = await recoverViewer({
      originalBytes: original,
      name: "book.xlsx",
      opts: { readonly: true },
      attempted: false,
      openFresh: async (bytes, name, opts) => {
        opens++;
        assert.deepEqual([...bytes], [4, 5, 6]);
        assert.equal(name, "book.xlsx");
        assert.equal(opts.readonly, true);
        return { id: "fresh" };
      },
    });
    assert.equal(res.ok, true);
    assert.equal(opens, 1);
    assert.equal(res.session.id, "fresh");
    assert.equal(res.attempted, true);
  });

  it("does not open a third time after one reload attempt fails (negative control)", async () => {
    // Catches: infinite reload loop when the fresh worker also crashes.
    let opens = 0;
    const openFresh = async () => {
      opens++;
      throw new Error("still crashing");
    };
    const first = await recoverViewer({
      originalBytes: new Uint8Array([1]),
      attempted: false,
      openFresh,
    });
    assert.equal(first.ok, false);
    assert.equal(first.attempted, true);
    assert.equal(opens, 1);

    const second = await recoverViewer({
      originalBytes: new Uint8Array([1]),
      attempted: true,
      openFresh,
    });
    assert.equal(second.ok, false);
    assert.equal(second.error.code, "viewer_recovery_exhausted");
    assert.equal(second.error.message, VIEWER_RECOVERY_FAILED_MESSAGE);
    assert.equal(opens, 1);
  });
});
