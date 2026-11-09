// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { WorkerDocument } from '../worker-host.js';
import { createJournal } from '../lazy/journal.js';
import { buildAndPostFrame } from '../lazy/frame-build.js';

globalThis.HTMLElement = class extends EventTarget {};
globalThis.CustomEvent ||= class extends Event { constructor(type, opts = {}) { super(type, opts); this.detail = opts.detail; } };
const { ShellViewerElement, ViewerElement } = await import('../element.js');
function doc() {
  const d = Object.create(WorkerDocument.prototype);
  Object.assign(d, { _req: 0, _pending: new Map(), _worker: { postMessage() {}, terminate() { this.terminated = true; } } });
  return d;
}
test('worker size errors retain metadata and reject open/save requests', async () => {
  // Catches: worker serialization losing limit/actual and leaving host promises pending.
  const d = doc();
  const open = d._request('opened', () => {});
  const save = d.save();
  d._onMessage({ type: 'error', code: 'part_too_large', message: 'too large', limit: 25, actual: 26, part: 'word/document.xml' });
  for (const promise of [open, save]) await assert.rejects(promise, { code: 'part_too_large', limit: 25, actual: 26, part: 'word/document.xml' });
  assert.equal(d._pending.size, 0);
});
test('stuck worker terminates at the existing budget; future requests reject', async () => {
  // Catches: a synchronous WASM loop preventing open rejection forever.
  const d = doc();
  const original = globalThis.setTimeout;
  let expire;
  globalThis.setTimeout = (fn, ms) => { assert.equal(ms, 120000); expire = fn; return 0; };
  try {
    const pending = d._request('opened', () => {});
    expire();
    await assert.rejects(pending, { code: 'NITRO_TIMEOUT', limit: 120000 });
    assert.equal(d._worker.terminated, true);
    await assert.rejects(d.save(), { code: 'NITRO_TIMEOUT' });
    assert.equal(d._pending.size, 0);
  } finally { globalThis.setTimeout = original; }
});
test('element open and save reject and emit the same error once', async () => {
  // Catches: element showing an error locally while embedders see success or no event.
  const el = new ShellViewerElement();
  el._show = () => {};
  const failures = [];
  el.addEventListener('error', e => failures.push(e.detail));
  await assert.rejects(el.loadBuffer(new Uint8Array(), 'bad.txt'), { code: 'unsupported_format' });
  const error = Object.assign(new Error('save broke'), { code: 'save_failed' });
  el._readonly = false;
  el._d = { save: async () => { throw error; } };
  await assert.rejects(el.save(), error);
  assert.equal(failures.length, 2);
  assert.equal(failures[1], error);
  assert.equal(el._reportedErrors.has(error), true);
});
test('failed URL reads reject and notify the element host', async () => {
  // Catches: failures before loadBuffer bypassing the host error event.
  const el = new ViewerElement();
  el._show = () => {};
  const error = new Error('offline');
  const original = globalThis.fetch;
  globalThis.fetch = async () => { throw error; };
  let reported;
  el.addEventListener('error', e => { reported = e.detail; });
  try { await assert.rejects(el.loadUrl('https://example.test/a.pptx'), error); }
  finally { globalThis.fetch = original; }
  assert.equal(reported, error);
});
test('geometry commits survive paint failure and replay once in commit order', async () => {
  // Catches: shape drags/resizes/rotates committing in Rust but disappearing after recovery.
  const d = doc();
  d._journal = createJournal(new Uint8Array([1]));
  const specs = ['move', 'resize', 'rotate'].map((kind, i) => JSON.stringify({ command: 'patchShapeGeometry', kind, x: i }));
  const paintError = new Error('paint failed after commit');
  const handle = { prepareFrame() {}, takeGeometryCommits: () => specs, takeGeometryError: () => paintError };
  await assert.rejects(buildAndPostFrame(handle, { postMessage: msg => d._onMessage(msg) }), paintError);
  const replayed = [];
  const result = await d._journal.recover({ openFresh: () => ({}), applyOps: (_, op) => replayed.push(op.spec) });
  assert.equal(result.ok, true);
  assert.deepEqual(replayed, specs);
  d._replayingJournal = true;
  d._onMessage({ type: 'geometry-commits', commits: specs });
  assert.equal(d._journal.ops().length, 3);
});
test('synchronous postMessage failure removes the request and its timer', async () => {
  // Catches: detached-buffer/clone errors leaking a waiter until a spurious timeout kills the worker.
  const d = doc();
  await assert.rejects(d._request('opened', () => { throw new Error('clone'); }), /clone/);
  assert.equal(d._pending.size, 0);
});
test('oversized File rejects before reading its bytes', async () => {
  // Catches: reading a huge Blob into RAM before enforcing the existing compressed-input cap.
  const el = new ViewerElement();
  el._show = () => {};
  let read = false, reported;
  el.addEventListener('error', e => { reported = e.detail; });
  const file = { name: 'large.pptx', size: 50 * 1024 * 1024 + 1, arrayBuffer() { read = true; } };
  await assert.rejects(el.loadFile(file), { code: 'input_too_large', limit: 50 * 1024 * 1024, actual: file.size });
  assert.equal(read, false);
  assert.equal(reported.code, 'input_too_large');
});
test('editable PowerPoint open arms recovery before exposing the document', async () => {
  // Catches: auto-flushed pointer events committing before lazy journal initialization.
  const d = doc();
  d._openBytes = async () => ({ format: 'pptx' });
  d._ensureOriginalBytes = async () => new Uint8Array([1, 2, 3]);
  await d.open(new Uint8Array([1, 2, 3]), 'a.pptx', { readonly: false });
  assert.deepEqual([...d._journal.originalBytes()], [1, 2, 3]);
});
