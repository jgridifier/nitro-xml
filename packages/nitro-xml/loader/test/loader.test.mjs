// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { applyCommandBuffer, Op } from "../apply.js";
import { encodeEventBatch, EventKind, EVENT_VERSION } from "../events.js";
import { flushMeasureBatch, readMeasureReqsFromMemory } from "../measure.js";
import {
  flushLayoutReadBatch,
  LayoutReadKind,
  LAYOUT_RESULT_LANES,
} from "../layout-readback.js";
import {
  isSupportedExtension,
  ACCEPT,
  SUPPORTED_EXTENSIONS,
} from "../formats.js";

/** Test-only encoders (mirrors Rust CommandBuffer writers; not shipped). */
function encodeFillRect(x, y, w, h, color) {
  const u8 = new Uint8Array(21);
  const v = new DataView(u8.buffer);
  u8[0] = Op.FillRect;
  v.setInt32(1, x, true);
  v.setInt32(5, y, true);
  v.setInt32(9, w, true);
  v.setInt32(13, h, true);
  v.setUint32(17, color, true);
  return u8;
}

function encodeStrokeRect(x, y, w, h, color, line) {
  const u8 = new Uint8Array(25);
  const v = new DataView(u8.buffer);
  u8[0] = Op.StrokeRect;
  v.setInt32(1, x, true);
  v.setInt32(5, y, true);
  v.setInt32(9, w, true);
  v.setInt32(13, h, true);
  v.setUint32(17, color, true);
  v.setFloat32(21, line, true);
  return u8;
}

describe("formats allow-list", () => {
  it("rejects legacy and non-OOXML extensions the component must not open", () => {
    // Catches: component accept/file checks drifting to allow .doc/.xls/.csv (PRODUCT_SPEC OUT list).
    assert.equal(isSupportedExtension("report.csv"), false);
    assert.equal(isSupportedExtension("legacy.doc"), false);
    assert.equal(isSupportedExtension("old.xls"), false);
    assert.equal(isSupportedExtension("deck.ppt"), false);
    assert.equal(isSupportedExtension("book.xlsx"), true);
    assert.equal(isSupportedExtension("macro.xlsm"), true);
    assert.equal(isSupportedExtension("tpl.dotm"), true);
    for (const ext of SUPPORTED_EXTENSIONS) {
      assert.ok(ACCEPT.includes(`.${ext}`), `ACCEPT missing .${ext}`);
    }
  });
});

describe("command buffer applier", () => {
  it("stops on truncated StrokeRect instead of reading past the buffer", () => {
    // Catches: short StrokeRect payload causing DataView OOB / wrong subsequent ops.
    const full = encodeStrokeRect(1, 2, 3, 4, 0xff0000ff, 1.5);
    const truncated = full.subarray(0, 10); // tag + partial payload
    const calls = [];
    const ctx = {
      strokeRect(...args) {
        calls.push(args);
      },
      set strokeStyle(v) {
        this._s = v;
      },
      set lineWidth(v) {
        this._l = v;
      },
    };
    const { ops, error } = applyCommandBuffer(truncated, {
      canvas: ctx,
      resolve: () => "",
    });
    assert.equal(ops, 0);
    assert.match(error, /truncated StrokeRect/);
    assert.equal(calls.length, 0);
  });

  it("applies a FillRect in one pass", () => {
    // Catches: FillRect opcode/payload endianness drift vs transport.rs (color/channels ignored → blank frame).
    const bytes = encodeFillRect(10, 20, 30, 40, 0xff112233);
    let fill;
    const ctx = {
      fillRect(x, y, w, h) {
        fill = [x, y, w, h, this.fillStyle];
      },
      set fillStyle(v) {
        this._f = v;
      },
      get fillStyle() {
        return this._f;
      },
    };
    const { ops, error } = applyCommandBuffer(bytes, { canvas: ctx, resolve: () => "" });
    assert.equal(error, undefined);
    assert.equal(ops, 1);
    assert.deepEqual(fill.slice(0, 4), [10, 20, 30, 40]);
    assert.equal(bytes[0], Op.FillRect);
  });
});

describe("event batch encoder", () => {
  it("packs a pointer event to the events.rs little-endian layout", () => {
    // Catches: loader encoder skew vs Rust decode_batch (wrong pad/time/payload → event_batch_* errors).
    const bytes = encodeEventBatch([
      {
        kind: EventKind.Pointer,
        time_ms: 1,
        x: 10,
        y: 20,
        type: 0,
        button: 0,
        buttons: 1,
      },
    ]);
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    assert.equal(view.getUint16(0, true), EVENT_VERSION);
    assert.equal(view.getUint16(2, true), 1);
    assert.equal(bytes[4], EventKind.Pointer);
    assert.equal(bytes.byteLength, 4 + 12 + 12); // header + event header + pointer payload
    assert.equal(view.getFloat32(16, true), 10);
    assert.equal(view.getFloat32(20, true), 20);
    assert.equal(bytes[24], 0); // type
    assert.equal(bytes[25], 0); // button
    assert.equal(view.getUint16(26, true), 1); // buttons
  });
});

describe("measure batch read-back", () => {
  it("invokes measure*Ptr methods when reading SoA from wasm memory", () => {
    // Catches: treating wasm-bindgen measureFontPtr() as a property (byteOffset → 0 → empty strings).
    const buf = new ArrayBuffer(48);
    new Uint32Array(buf, 0, 2).set([11, 12]);
    new Float32Array(buf, 16, 2).set([13, 14]);
    new Uint32Array(buf, 32, 2).set([21, 22]);
    let fontCalls = 0;
    const handle = {
      measureCount: 2,
      measureFontPtr() {
        fontCalls++;
        return 0;
      },
      measureSizePtr() {
        return 16;
      },
      measureTextPtr() {
        return 32;
      },
    };
    const reqs = readMeasureReqsFromMemory(handle, { buffer: buf });
    assert.equal(fontCalls, 1);
    assert.equal(reqs.length, 2);
    assert.equal(reqs[0].fontId, 11);
    assert.equal(reqs[0].sizePx, 13);
    assert.equal(reqs[0].textId, 21);
    assert.equal(reqs[1].fontId, 12);
    assert.equal(reqs[1].textId, 22);
  });

  it("writes all pending widths in a single setMeasureResults call", () => {
    // Catches: per-string setMeasureResults (violates HoP batched read-back / no per-element calls).
    const reqs = [
      { fontId: 1, sizePx: 12, textId: 2 },
      { fontId: 1, sizePx: 12, textId: 3 },
      { fontId: 1, sizePx: 14, textId: 4 },
    ];
    let calls = 0;
    let got;
    const handle = {
      measureCount: 3,
      resolve: (id) => (id === 1 ? "sans-serif" : `t${id}`),
      takeMeasureReqs: () => reqs,
      setMeasureResults(widths) {
        calls++;
        got = widths;
      },
    };
    const ctx = {
      measureText(text) {
        return { width: text.length * 10 };
      },
      set font(_v) {},
    };
    const n = flushMeasureBatch(handle, ctx);
    assert.equal(n, 3);
    assert.equal(calls, 1);
    assert.equal(got.length, 3);
    assert.equal(got[0], 20); // "t2".length * 10
    assert.equal(got[1], 20);
    assert.equal(got[2], 20);
  });
});

describe("post-apply layout read-back", () => {
  it("answers every pending node-id query in a single setLayoutResults call", () => {
    // Catches: per-element layout read-back (violates HoP next-frame batch / no per-element calls).
    const reqs = [
      { nodeId: 1, kind: LayoutReadKind.ScrollHeight },
      { nodeId: 2, kind: LayoutReadKind.NaturalWidth },
      { nodeId: 3, kind: LayoutReadKind.ComputedStyle, propId: 9 },
    ];
    let calls = 0;
    let got;
    const handle = {
      layoutCount: 3,
      resolve: (id) => (id === 9 ? "font-size" : ""),
      takeLayoutReqs: () => reqs,
      setLayoutResults(values) {
        calls++;
        got = values;
      },
    };
    const nodes = new Map([
      [1, { scrollHeight: 120 }],
      [2, { naturalWidth: 64, naturalHeight: 48 }],
      [
        3,
        {
          /* element placeholder; getComputedStyle stubbed below */
        },
      ],
    ]);
    const prev = globalThis.getComputedStyle;
    globalThis.getComputedStyle = () => ({
      getPropertyValue: (prop) => (prop === "font-size" ? "14px" : ""),
    });
    try {
      const n = flushLayoutReadBatch(handle, nodes);
      assert.equal(n, 3);
      assert.equal(calls, 1);
      assert.equal(got.length, 3 * LAYOUT_RESULT_LANES);
      assert.equal(got[0], 120);
      assert.equal(got[LAYOUT_RESULT_LANES], 64);
      assert.equal(got[LAYOUT_RESULT_LANES + 1], 48);
      assert.equal(got[2 * LAYOUT_RESULT_LANES], 14);
    } finally {
      if (prev) globalThis.getComputedStyle = prev;
      else delete globalThis.getComputedStyle;
    }
  });
});
