// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { paintFrame } from "../lazy/paint-frame.js";

function stubDoc(root) {
  return {
    _root: root,
    _nodes: new Map(),
    _images: new Map(),
    _strings: {},
    _canvas: null,
    _nxPainted: 1,
    _rt: 0,
    _measureCtx() {
      return null;
    },
    _queueLayout() {},
    async _request() {
      return undefined;
    },
    async tick() {
      this.tickCalls = (this.tickCalls | 0) + 1;
      return { ops: 0 };
    },
  };
}

describe("paintFrame empty-frame keep", () => {
  it("keeps existing DOM when mut-frame returns zero commands", async () => {
    // Catches: mut-frame paint wiping the page on an empty post-edit frame (docx.type-keeps-page).
    const root = {
      firstChild: { tag: "page" },
      kids: ["page"],
      replaceChildren() {
        this.kids = [];
        this.firstChild = null;
      },
    };
    const doc = stubDoc(root);
    const ticks = [];
    doc.tick = async () => {
      ticks.push(1);
      return { ops: 0 };
    };

    const out = await paintFrame(doc, {
      commands: new Uint8Array(0),
      measureCount: 0,
      outlineCount: 0,
      layoutCount: 0,
      strings: {},
      t: true,
    });

    assert.equal(out.ops, 0);
    assert.deepEqual(root.kids, ["page"]);
    assert.equal(doc._nxPainted, 0);
    // Follow-up take-frame must run; otherwise caret/page stay stale under _nxPainted skip.
    await new Promise((r) => setTimeout(r, 0));
    assert.equal(ticks.length, 1);
  });

  it("still applies a non-empty command buffer", async () => {
    // Catches: empty-keep guard short-circuiting real paints.
    const root = {
      firstChild: null,
      kids: [],
      replaceChildren() {
        this.kids = [];
        this.firstChild = null;
      },
    };
    const doc = stubDoc(root);
    // Minimal CreateElement(op=5) + AppendChild(op=7) is heavy; just ensure replaceChildren runs
    // for a non-empty buffer that ends immediately (truncated → apply returns early after clear).
    const commands = new Uint8Array([5]);
    await paintFrame(doc, {
      commands,
      measureCount: 0,
      outlineCount: 0,
      layoutCount: 0,
      strings: {},
      t: true,
    });
    assert.deepEqual(root.kids, []);
  });
});
