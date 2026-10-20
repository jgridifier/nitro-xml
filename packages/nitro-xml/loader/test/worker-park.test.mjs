// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { applyCommandBuffer, Op } from "../apply.js";
import { takePrewarmedWorker } from "../bootstrap.js";
import { patch } from "../lazy/wp.js";

function u32(n) {
  const b = new Uint8Array(4);
  new DataView(b.buffer).setUint32(0, n, true);
  return b;
}
function i32(n) {
  const b = new Uint8Array(4);
  new DataView(b.buffer).setInt32(0, n, true);
  return b;
}
function concat(parts) {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}
/** Page line plus a caret whose left edge is `caretX` px. */
function paintedPage(caretX) {
  return concat([
    Uint8Array.of(Op.CreateElement), u32(1), u32(1), u32(0),
    Uint8Array.of(Op.CreateElement), u32(2), u32(1), u32(0),
    Uint8Array.of(Op.SetAttr), u32(2), u32(2), u32(3),
    Uint8Array.of(Op.PlaceBox), u32(2), i32(caretX), i32(20), i32(1), i32(16),
  ]);
}
function mockElement() {
  return {
    style: {},
    attrs: {},
    children: [],
    setAttribute(k, v) {
      this.attrs[k] = v;
    },
    appendChild(c) {
      this.children.push(c);
      return c;
    },
    replaceChildren() {
      this.children.length = 0;
    },
  };
}

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

<<<<<<< HEAD
  it("keeps the docx page painted and advances the caret after typing", async () => {
    // Catches: a Word keystroke painting the pre-measure frame and skipping the next tick, so the page stays blank and the caret never moves.
    const prevDocument = globalThis.document;
    globalThis.document = { createElement: () => mockElement() };
    try {
      const strings = { 1: "div", 2: "class", 3: "nxc" };
      class KeyDoc {
        constructor() {
          this._worker = { postMessage() {} };
          this._name = "pages.docx";
          this._root = mockElement();
          this._root.appendChild(mockElement());
          this._nodes = new Map();
          this._strings = strings;
        }
        _request(_type, send) {
          send();
          return Promise.resolve({
            result: "{}",
            commands: new Uint8Array(0),
            layoutCount: 1,
            strings,
          });
        }
        _queueLayout() {}
        tick() {
          applyCommandBuffer(paintedPage(140), {
            root: this._root,
            nodes: this._nodes,
            resolve: (id) => strings[id] || "",
          });
          return Promise.resolve({ ops: [1] });
        }
        editorKey() {
          return Promise.resolve("{}");
        }
        shellAction() {}
        applyEdit() {}
      }
      patch(KeyDoc);
      const d = new KeyDoc();
      const before = 100;
      d._root.children[0].style.left = `${before}px`;
      let recaret = 0;
      d._nxReCaret = () => {
        recaret++;
      };
      await d.editorKey({ key: "Z" }, false);
      await d.tick();
      assert.ok(recaret >= 1, "measured frame did not re-place the caret");
      assert.ok(d._root.children.length > 0, "follow-up frame did not repaint the page");
      const caret = d._root.children.find((el) => el.attrs.class === "nxc");
      assert.ok(caret, "caret missing after typing");
      assert.ok(parseFloat(caret.style.left) > before, `caret did not advance (${before} -> ${caret.style.left})`);
    } finally {
      globalThis.document = prevDocument;
    }
=======
  it("uses mut-frame for spreadsheet applyEdit when a worker is present", async () => {
    // Catches: xlsx staying on apply+tick (two RTs) after sync/history were fixed —
    // mut-frame must run for spreadsheets too (#341 left them on the slow path).
    let mutOps = 0;
    class FakeDoc {
      applyEdit() {
        return Promise.resolve("direct");
      }
    }
    patch(FakeDoc);
    const d = new FakeDoc();
    d._worker = { postMessage() {} };
    d._request = async (type, send) => {
      if (type === "mut-frame") {
        mutOps++;
        send();
        // No `commands` → skip paintFrame import in wp.js.
        return { type: "mut-frame", result: '{"h":""}' };
      }
      return {};
    };
    const out = await d.applyEdit(
      '{"command":"setCellInput","sheet":0,"row":0,"col":0,"text":"z"}',
    );
    assert.equal(mutOps, 1);
    assert.equal(out, '{"h":""}');
>>>>>>> f63cea0 (Part of #30: xlsx applyEdit mut-frame + defer paint to tick)
  });
});
