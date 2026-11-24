// SPDX-License-Identifier: MIT
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";
import { checkInputSize } from "../../nitro-xml/loader/formats.js";

const source = readFileSync(new URL("../src/NitroViewer.tsx", import.meta.url), "utf8");
const js = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX },
}).outputText;

async function mount(props = {}) {
  const { file = new ArrayBuffer(0), fileName = "a.docx", editable = true } = props;
  const dirtyEvents = [];
  let handle, cleanup;
  const listeners = new Map();
  const el = {
    _dirty: false,
    get dirty() { return this._dirty; },
    style: {},
    setAttribute() {},
    remove() {},
    addEventListener: (type, fn) => listeners.set(type, fn),
    removeEventListener: (type) => listeners.delete(type),
    loadBuffer: async () => {},
    document: { tick: async () => {} },
    save: async () => new Uint8Array([1, 2, 3]),
  };
  let refCount = 0;
  const react = {
    forwardRef: (fn) => fn,
    useRef: (value) => ({ current: refCount++ === 0 ? { replaceChildren() {} } : value }),
    useEffect: (fn) => { cleanup = fn(); },
    useImperativeHandle: (_, fn) => { handle = fn(); },
  };
  const loader = { checkInputSize, defineNitroXlsx() {}, defineNitroDocx() {}, defineNitroPptx() {} };
  const exports = {};
  const previous = { document: globalThis.document, fetch: globalThis.fetch };
  globalThis.document = { createElement: () => el };
  globalThis.fetch = async () => { throw new Error("no fetch"); };
  try {
    new Function("require", "exports", js)((name) => {
      if (name === "react") return react;
      if (name === "react/jsx-runtime") return { jsx: () => ({}) };
      if (name === "nitro-xml/loader") return loader;
      throw new Error(name);
    }, exports);
    exports.NitroViewer(
      { file, fileName, editable, onDirtyChange: (d) => dirtyEvents.push(d) },
      {},
    );
    await new Promise((r) => setImmediate(r));
    return { el, handle, dirtyEvents, cleanup, listeners };
  } finally {
    Object.assign(globalThis, previous);
  }
}

test("onDirtyChange fires when nitro-dirty event is dispatched", async () => {
  // Catches: onDirtyChange prop not wired to the nitro-dirty listener on the element.
  const { listeners, dirtyEvents, cleanup } = await mount();
  const handler = listeners.get("nitro-dirty");
  assert.ok(handler, "nitro-dirty listener must be registered on the element");
  handler(new CustomEvent("nitro-dirty", { detail: { dirty: true } }));
  assert.deepEqual(dirtyEvents, [true], "onDirtyChange must receive dirty=true");
  handler(new CustomEvent("nitro-dirty", { detail: { dirty: false } }));
  assert.deepEqual(dirtyEvents, [true, false], "onDirtyChange must receive dirty=false");
  cleanup();
});

test("isDirty() reflects the element dirty getter and returns false before load", async () => {
  // Catches: isDirty() not reading from the live element, throwing before load, or missing from the handle.
  const preLoad = await mount({ file: null });
  assert.equal(preLoad.handle.isDirty(), false, "isDirty() must return false before load");
  preLoad.cleanup?.();

  const { el, handle, cleanup } = await mount();
  assert.equal(handle.isDirty(), false, "isDirty() must return false before any edit");
  el._dirty = true;
  assert.equal(handle.isDirty(), true, "isDirty() must return true when element.dirty is true");
  el._dirty = false;
  assert.equal(handle.isDirty(), false, "isDirty() must return false after reset");
  el._dirty = true;
  cleanup();
  assert.equal(handle.isDirty(), false, "isDirty() must return false after unmount cleanup");
});

test("nitro-dirty listener is removed on cleanup", async () => {
  // Catches: memory leak from leaving the nitro-dirty listener on the element after unmount.
  const { listeners, cleanup } = await mount();
  assert.ok(listeners.has("nitro-dirty"), "listener added before cleanup");
  cleanup();
  assert.ok(!listeners.has("nitro-dirty"), "listener removed after cleanup");
});
