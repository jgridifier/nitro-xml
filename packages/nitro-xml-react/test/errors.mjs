// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';
import { checkInputSize } from '../../nitro-xml/loader/formats.js';

const source = readFileSync(new URL('../src/NitroViewer.tsx', import.meta.url), 'utf8');
const js = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX } }).outputText;

async function mount({ openError, paintError, fetchError, file = new ArrayBuffer(0) } = {}) {
  const errors = [];
  let ready = 0, handle, cleanup;
  const listeners = new Map();
  const el = {
    style: {}, setAttribute() {}, remove() {},
    addEventListener: (type, fn) => listeners.set(type, fn),
    removeEventListener: type => listeners.delete(type),
    loadBuffer: async () => {
      if (openError) { listeners.get('error')?.({ detail: openError }); throw openError; }
    },
    document: { tick: async () => { if (paintError) throw paintError; } },
    save: async () => {
      const error = new Error('save failed');
      listeners.get('error')?.({ detail: error });
      throw error;
    },
  };
  let refCount = 0;
  const react = {
    forwardRef: fn => fn,
    useRef: value => ({ current: refCount++ === 0 ? { replaceChildren() {} } : value }),
    useEffect: fn => { cleanup = fn(); },
    useImperativeHandle: (_, fn) => { handle = fn(); },
  };
  const loader = { checkInputSize, defineNitroXlsx() {}, defineNitroDocx() {}, defineNitroPptx() {} };
  const exports = {};
  const previous = { document: globalThis.document, fetch: globalThis.fetch };
  globalThis.document = { createElement: () => el };
  globalThis.fetch = async () => { throw fetchError; };
  try {
    new Function('require', 'exports', js)(name => {
      if (name === 'react') return react;
      if (name === 'react/jsx-runtime') return { jsx: () => ({}) };
      if (name === 'nitro-xml/loader') return loader;
      throw new Error(name);
    }, exports);
    exports.NitroViewer({ file: fetchError ? 'https://example.test/a.docx' : file, fileName: 'a.docx', onError: e => errors.push(e), onReady: () => ready++ }, {});
    await new Promise(resolve => setImmediate(resolve));
    return { errors, ready, handle, cleanup };
  } finally { Object.assign(globalThis, previous); }
}

test('React forwards the original open error once and never signals ready', async () => {
  // Catches: rejected loadBuffer being console-only or reported twice via event + rejection.
  const error = Object.assign(new Error('too large'), { code: 'part_too_large', limit: 25, actual: 26 });
  const result = await mount({ openError: error });
  assert.deepEqual(result.errors, [error]);
  assert.equal(result.ready, 0);
  result.cleanup();
});
test('React reports pre-element fetch and paint failures', async () => {
  // Catches: network failures and first-paint errors being silently swallowed by the wrapper.
  for (const field of ['fetchError', 'paintError']) {
    const error = new Error(field);
    const result = await mount({ [field]: error });
    assert.deepEqual(result.errors, [error]);
    assert.equal(result.ready, 0);
    result.cleanup();
  }
});
test('React imperative save rejects and forwards the error event', async () => {
  // Catches: save rejection never reaching onError on a mounted React embed.
  const result = await mount();
  await assert.rejects(result.handle.save(), /save failed/);
  assert.equal(result.errors.length, 1);
  result.cleanup();
});

test('React rejects oversized Blob before reading', async () => {
  // Catches: React allocating a huge Blob before the custom element can enforce the input cap.
  let read = false;
  const file = new class extends Blob {
    get size() { return 50 * 1024 * 1024 + 1; }
    async arrayBuffer() { read = true; return new ArrayBuffer(0); }
  }();
  const result = await mount({ file });
  assert.equal(read, false);
  assert.equal(result.errors[0].code, 'input_too_large');
  assert.equal(result.ready, 0);
  result.cleanup();
});
