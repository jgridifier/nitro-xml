// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  applyCommandBuffer,
  encodeStrokeRect,
  encodeFillRect,
  Op,
} from "../apply.js";
import { encodeEventBatch, EventKind, EVENT_VERSION } from "../events.js";
import { flushMeasureBatch } from "../measure.js";
import {
  isSupportedExtension,
  ACCEPT,
  SUPPORTED_EXTENSIONS,
} from "../formats.js";

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
