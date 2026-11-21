// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { trackDirty } from "../lazy/ribbon/dirty.js";
import { attach } from "../lazy/dh.js";

describe("dh.js attach (nitro-dirty + nitro-change dispatch)", () => {
  it("fires nitro-dirty and nitro-change on first edit after attach", async () => {
    // Catches: dh.js only dispatching nitro-change and not adding the nitro-dirty alias.
    const original = new Uint8Array([1, 2, 3]);
    const edited = new Uint8Array([1, 2, 3, 4]);
    const fired = [];
    const el = makeDhEl(original);
    el.addEventListener = (type, fn) => { el._listeners.push({ type, fn }); };
    el.removeEventListener = () => {};
    el.dispatchEvent = (ev) => { fired.push(ev.type); };
    await attach(el);
    // Simulate an edit: change the bytes that the worker returns
    el._d._bytes = edited;
    el._dT.touch();
    await tick(400);
    assert.ok(fired.includes("nitro-dirty"), "nitro-dirty must be dispatched on first edit");
    assert.ok(fired.includes("nitro-change"), "nitro-change must still fire as deprecated alias");
    el._dT.destroy();
  });

  it("nitro-dirty detail.dirty is true on edit, false after save", async () => {
    // Catches: detail payload missing or carrying wrong value.
    const original = new Uint8Array([10, 20]);
    const edited = new Uint8Array([10, 20, 30]);
    const details = [];
    const el = makeDhEl(original);
    el.addEventListener = (type, fn) => { el._listeners.push({ type, fn }); };
    el.removeEventListener = () => {};
    el.dispatchEvent = (ev) => {
      if (ev.type === "nitro-dirty") details.push(ev.detail.dirty);
    };
    await attach(el);
    el._d._bytes = edited;
    el._dT.touch();
    await tick(400);
    assert.deepEqual(details, [true], "detail.dirty must be true after edit");
    await el.save();
    assert.deepEqual(details, [true, false], "detail.dirty must be false after save");
    el._dT.destroy();
  });
});

describe("trackDirty (dirty getter + nitro-change)", () => {
  it("rebase sets baseline; subsequent identical hash does not fire onChange", async () => {
    // Catches: baseline never set so dirty check always skips (base==null branch), making
    // nitro-change never fire after the first user edit.
    const bytes = new Uint8Array([1, 2, 3, 4]);
    let calls = 0;
    const el = makeEl(bytes);
    const tracker = trackDirty(el, () => { calls++; });
    await tracker.rebase();
    // Check after rebase with same bytes → no flip (dirty was already false)
    tracker.touch();
    await tick(400);
    assert.equal(calls, 0, "no event when document unchanged after rebase");
    tracker.destroy();
  });

  it("saved(bytes) resets dirty to false and fires onChange on flip", async () => {
    // Catches: saved() not calling set(false), leaving dirty=true even after a successful save
    const original = new Uint8Array([1, 2, 3]);
    const edited = new Uint8Array([1, 2, 3, 4]);
    const events = [];
    const el = makeEl(original);
    const tracker = trackDirty(el, (d) => events.push(d));
    await tracker.rebase();
    // Simulate edit: worker now returns different bytes
    el._d._bytes = edited;
    tracker.touch();
    await tick(400);
    assert.equal(events.at(-1), true, "first edit must fire dirty=true");
    // Now save with the edited bytes
    tracker.saved(edited);
    assert.equal(events.at(-1), false, "saved() must fire dirty=false");
    tracker.destroy();
  });

  it("nitro-change does not fire for identical repeated flip direction", async () => {
    // Catches: event fired on every timer tick instead of only on flip
    const bytes = new Uint8Array([1, 2, 3]);
    const edited = new Uint8Array([1, 2, 3, 4]);
    const events = [];
    const el = makeEl(bytes);
    const tracker = trackDirty(el, (d) => events.push(d));
    await tracker.rebase();
    el._d._bytes = edited;
    tracker.touch();
    await tick(400);
    tracker.touch();
    await tick(400);
    assert.equal(events.filter(d => d === true).length, 1, "dirty=true fires exactly once");
    tracker.destroy();
  });

  it("rebase() called on re-attach resets dirty without spurious event", async () => {
    // Catches: re-opening a document while dirty still set triggers a stale dirty=true event
    const bytes = new Uint8Array([1, 2, 3]);
    const edited = new Uint8Array([1, 2, 3, 4]);
    const events = [];
    const el = makeEl(bytes);
    const tracker = trackDirty(el, (d) => events.push(d));
    await tracker.rebase();
    el._d._bytes = edited;
    tracker.touch();
    await tick(400);
    assert.equal(events.at(-1), true);
    // Simulate reload: rebase resets base to current bytes (edited), dirty → false
    await tracker.rebase();
    assert.equal(events.at(-1), false, "rebase must reset dirty=false");
    tracker.destroy();
  });
});

// ---- helpers ----

function makeEl(bytes) {
  const el = {
    _d: {
      _bytes: bytes,
      _worker: { postMessage() {} },
      _request: async (expect) => {
        if (expect === "saved") return { type: "saved", bytes: el._d._bytes };
        return {};
      },
    },
    _events: new Map(),
    addEventListener() {},
    removeEventListener() {},
  };
  return el;
}

function makeDhEl(bytes) {
  // dh.js reads proto.save via Object.getPrototypeOf(el).save, so save must be on the prototype.
  const proto = {
    async save() {
      const msg = await this._d._request("saved");
      return new Uint8Array(msg.bytes);
    },
  };
  const el = Object.create(proto);
  el._readonly = false;
  el._listeners = [];
  el._d = {
    _bytes: bytes,
    _worker: { postMessage() {} },
    _request: async (expect) => {
      if (expect === "saved") return { type: "saved", bytes: el._d._bytes };
      return {};
    },
  };
  el.addEventListener = function () {};
  el.removeEventListener = function () {};
  el.dispatchEvent = function () {};
  return el;
}

function tick(ms) {
  return new Promise((r) => setTimeout(r, ms));
}
