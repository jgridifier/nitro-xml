// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { paintFrame } from "../lazy/paint-frame.js";

describe("paint-frame measure ordering", () => {
  it("applies the command buffer before awaiting measure-acked", async () => {
    // Catches: host awaiting measure RT before applyCommandBuffer blocks first paint (#402).
    const seq = [];
    const commands = new Uint8Array(0);
    const doc = {
      _strings: {},
      _measureCount: 0,
      _outlineCount: 0,
      _t: true,
      _canvas: null,
      _root: {
        firstChild: null,
        replaceChildren() {
          seq.push("apply");
        },
      },
      _nodes: new Map(),
      _images: new Map(),
      _rt: 0,
      _queueLayout() {},
      _measureCtx() {
        return { measureText: () => ({ width: 10 }) };
      },
      async _request(expect, send) {
        seq.push(`await-${expect}`);
        send();
        return {};
      },
      tick() {
        return Promise.resolve();
      },
      _worker: { postMessage() {} },
    };
    const frame = {
      commands,
      measureCount: 1,
      measureReqs: {
        fonts: new Uint32Array([0]),
        sizes: new Float32Array([12]),
        texts: new Uint32Array([1]),
      },
      outlineCount: 0,
      layoutCount: 0,
      strings: { 0: "sans-serif", 1: "x" },
      t: true,
    };
    await paintFrame(doc, frame);
    // Apply must win the race with measure; measure refine is async after return.
    assert.equal(seq[0], "apply");
    await new Promise((r) => setTimeout(r, 0));
    assert.ok(seq.includes("await-measure-acked"));
  });
});
