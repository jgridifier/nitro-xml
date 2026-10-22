// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { buildAndPostFrame } from "../lazy/frame-build.js";

describe("frame-build pending editor chrome", () => {
  it("posts the first frame before soft-share unlock and edit-ready", async () => {
    // Catches: await setReadOnly before postMessage so embed first-paint L (~1s)
    // includes ensure_edit; paint must post first, then unlock + edit-ready.
    const calls = [];
    const handle = {
      t: true,
      measureCount: 0,
      outlineCount: 0,
      layoutCount: 0,
      prepareFrame() {
        calls.push(["prepareFrame"]);
      },
      takeChromeIfDirty() {
        return null;
      },
      takeFormulaChromeIfDirty() {
        return null;
      },
      exportStrings() {
        return [];
      },
      takeFrameCommands() {
        return new Uint8Array(0);
      },
      setReadOnly(v) {
        calls.push(["setReadOnly", v]);
      },
      loadEditorChrome() {
        calls.push(["loadEditorChrome"]);
      },
    };
    const workerSelf = {
      _pendingEditorChrome: 1,
      postMessage(msg) {
        calls.push(["postMessage", msg && msg.type]);
      },
    };
    await buildAndPostFrame(handle, workerSelf, { type: "frame" });
    assert.equal(workerSelf._pendingEditorChrome, 0);
    assert.deepEqual(calls, [
      ["prepareFrame"],
      ["postMessage", "frame"],
      ["setReadOnly", false],
      ["loadEditorChrome"],
      ["postMessage", "edit-ready"],
    ]);
  });
});
