// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { attach } from '../lazy/pending-input.js';

test('save barrier includes the final input readback and propagates failure', async () => {
  // Catches: save overtaking a queued keystroke's final frame, or succeeding after input fails.
  let finish;
  let ticks = 0;
  const d = {
    tick: () => ++ticks === 2 ? new Promise(resolve => { finish = resolve; }) : Promise.resolve(),
    editorKey: async () => ({ handled: true }),
  };
  attach({ document: d, localName: 'nitro-pptx' });
  const key = d.editorKey({ key: 'a' });
  let drained = false;
  const drain = d._hostInput.then(() => { drained = true; });
  for (let i = 0; i < 10 && !finish; i++) await Promise.resolve();
  assert.equal(typeof finish, 'function');
  assert.equal(drained, false);
  finish();
  await key;
  await drain;
  assert.equal(drained, true);

  const error = new Error('input failed');
  const broken = { editorKey: async () => { throw error; } };
  attach({ document: broken, localName: 'nitro-xlsx' });
  await assert.rejects(broken.editorKey({ key: 'b' }), error);
  await assert.rejects(broken._hostInput, error);
});
