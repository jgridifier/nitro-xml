// SPDX-License-Identifier: MIT
// Catches: xlsx mut-frame posting CanvasTextMeasure reqs the sticky `_nxPainted`
// grid never re-ticks to apply — a dead worker RT on every novel cell string.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { buildAndPostFrame } from "../lazy/frame-build.js";

describe("frame-build xlsx mut-frame measures", () => {
  it("drops measure payloads on xlsx mut-frame (no host await)", async () => {
    let posted = null;
    let taken = 0;
    const handle = {
      t: true,
      measureCount: 2,
      outlineCount: 0,
      layoutCount: 0,
      documentFormat() {
        return "xlsx";
      },
      prepareFrame() {},
      takeChromeIfDirty() {
        return null;
      },
      takeFormulaChromeIfDirty() {
        return null;
      },
      takeMeasureReqs() {
        taken += 1;
        return [
          new Uint32Array([1, 2]),
          new Float64Array([11, 12]),
          new Uint32Array([21, 22]),
        ];
      },
      exportStrings() {
        return [];
      },
      takeFrameCommands() {
        return new Uint8Array([1]);
      },
    };
    const workerSelf = {
      postMessage(msg) {
        posted = msg;
      },
    };
    await buildAndPostFrame(handle, workerSelf, {
      type: "mut-frame",
      result: "{}",
    });
    assert.equal(taken, 1, "pending measures must be drained");
    assert.equal(posted.measureCount, 0);
    assert.equal(posted.measureReqs, null);
    assert.equal(posted.type, "mut-frame");
  });

  it("still posts measures on non-xlsx mut-frame", async () => {
    let posted = null;
    const handle = {
      t: true,
      measureCount: 1,
      outlineCount: 0,
      layoutCount: 0,
      documentFormat() {
        return "docx";
      },
      prepareFrame() {},
      takeChromeIfDirty() {
        return null;
      },
      takeFormulaChromeIfDirty() {
        return null;
      },
      takeMeasureReqs() {
        return [
          new Uint32Array([1]),
          new Float64Array([11]),
          new Uint32Array([21]),
        ];
      },
      exportStrings() {
        return [];
      },
      takeFrameCommands() {
        return new Uint8Array([1]);
      },
    };
    const workerSelf = {
      postMessage(msg) {
        posted = msg;
      },
    };
    await buildAndPostFrame(handle, workerSelf, {
      type: "mut-frame",
      result: "{}",
    });
    assert.equal(posted.measureCount, 1);
    assert.ok(posted.measureReqs);
  });
});
