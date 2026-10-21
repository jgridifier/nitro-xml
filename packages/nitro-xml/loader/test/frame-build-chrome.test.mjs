// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { buildAndPostFrame } from "../lazy/frame-build.js";

describe("frame-build pending editor chrome", () => {
  it("unlocks editable mode on the first posted frame", async () => {
    // Catches: pendingEditorChrome stuck at 2 so ensure_edit runs on the first
    // mut-frame unlockChrome instead of finishing before the timed edit sample.
    const calls = [];
    const handle = {
      t: true,
      measureCount: 0,
      outlineCount: 0,
      layoutCount: 0,
      prepareFrame() {},
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
      postMessage() {},
    };
    await buildAndPostFrame(handle, workerSelf, { type: "frame" });
    assert.equal(workerSelf._pendingEditorChrome, 0);
    assert.deepEqual(calls, [
      ["setReadOnly", false],
      ["loadEditorChrome"],
    ]);
  });
});
