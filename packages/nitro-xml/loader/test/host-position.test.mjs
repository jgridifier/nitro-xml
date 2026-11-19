// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { tabIndex } from '../lazy/host-position.js';
test('sheet names resolve to engine IDs, including non-contiguous visible sheets', () => {
  // Catches: a hidden sheet making name/index navigation select the wrong engine sheet.
  const tabs = [{index:0,name:'One'}, {index:2,name:'Two'}];
  assert.equal(tabIndex(tabs,'Two'),2);
  assert.equal(tabIndex(tabs,999),2);
  assert.equal(tabIndex(tabs,-4),0);
  assert.throws(()=>tabIndex(tabs,'Missing'),RangeError);
});
test('invalid numeric inputs cannot become a worker NaN or an unsigned giant index', () => {
  // Catches: non-finite host input bypassing bounds and failing in the WASM shell parser.
  for (const value of [NaN,Infinity,-Infinity,undefined]) assert.throws(()=>tabIndex([{index:0}],value),TypeError);
  assert.equal(tabIndex([],4),null);
});

test('reload copies a caller-owned byte view before transferring it to the worker', async () => {
  // Catches: reload detaching the host's buffer, or forwarding unrelated bytes around a subarray.
  const { reload } = await import('../lazy/host-position.js');
  const input = new Uint8Array([99, 1, 2, 88]);
  const sub = input.subarray(1, 3);
  let transferred;
  const d = {
    _name:'test.xlsx', _nodes:new Map(), _images:new Map(),
    _events:{flushNow(){}}, _worker:{postMessage(){}},
    tick:async()=>{d._positionReady=true;},
    open:async(bytes)=>{ transferred=structuredClone(bytes,{transfer:[bytes.buffer]}); },
    _request:async()=>({result:JSON.stringify({ui:{tabs:[{index:0,name:'Sheet1'}]}})}),
  };
  const el = {document:d, localName:'nitro-xlsx',readonly:true,_v:{style:{}},_ch:{},
    getPosition:()=>({sheet:0,scrollTop:0,scrollLeft:0}),_ao:async()=>{}};
  await reload(el,sub);
  assert.deepEqual([...input],[99,1,2,88]);
  assert.deepEqual([...transferred],[1,2]);
  assert.equal(el.document,d);
});

test('readonly can initialize editing without replacing the worker or reopening bytes', async () => {
  // Catches: mode toggling reopening original bytes and discarding edits made since open.
  const { setReadonly } = await import('../lazy/host-position.js');
  const messages=[];
  const d={_ro:true,_events:{flushNow(){}},tick:async()=>{},
    _worker:{postMessage:m=>messages.push(m)},
    _request:async(type,send)=>{send();return {result:JSON.stringify({ui:{tabs:[]}})}},
    open:()=>assert.fail('mode must not reopen'),
  };
  const events=[];
  const el={document:d,readonly:false,localName:'nitro-pptx',_v:{style:{}},_r:{},_ch:{},
    shadowRoot:{querySelector:()=>null},getPosition:()=>({scrollTop:0,scrollLeft:0}),
    _ao:async()=>{},dispatchEvent:e=>events.push(e)};
  await setReadonly(el);
  assert.equal(d._ro,false);
  assert.deepEqual(messages.find(m=>m.type==='host-mode'),{type:'host-mode',readonly:false});
  assert.equal(events[0].detail.readonly,false);
});
