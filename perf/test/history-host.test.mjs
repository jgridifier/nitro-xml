// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { toHistoryEntries } from '../aggregate.mjs';
import { evaluate, baselineFor, protocolId, unknownFields } from '../gate.mjs';
import { isUnknown } from '../unknown.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const th = { baseline_n: 5, classes: [{ name: 'time', match: '_ms$', rel: 0.1, abs: 5 }] };
const SRC = { kind: 'box', id: 'box-source', os: 'Debian', arch: 'x64', cpu_model: 'Xeon', cores: 8, memory_gb: 16, chrome_version: '156', node: 'v20', concurrent_runs: 1 };
const IMPORTER = { kind: 'box', id: 'importer-vm', os: 'linux 6', cpu: 'Other', cores: 4 };
const row = (host) => ({ fixture: 'f', engine: 'ts', mode: 'cold', cpuThrottle: 1, chrome: '156', ...(host ? { host } : {}), metrics: { first_paint_ms: 100 } });
const meta = { commit: 'c1', date: 'd', branch: 'main', runner: 'local', host: IMPORTER };

test('the importer host never overwrites the source run host', () => {
  // Catches: L/XL history entries carrying the importing machine (grok-bot-vm-...) instead of box-4467396174
  const [e] = toHistoryEntries([row(SRC)], meta);
  assert.equal(e.host.id, 'box-source');
  assert.deepEqual(e.host, { kind: 'box', id: 'box-source', os: 'Debian', arch: 'x64', cpu: 'Xeon', cores: 8, memory_gb: 16, chrome_version: '156', node: 'v20', concurrent_runs: 1 });
  assert.equal(e.host_import_recorded.id, 'importer-vm'); // kept for audit, never used as the host
  assert.equal(e.chrome_version, '156');
});

test('a missing source host is unknown, never the importer host', () => {
  // Catches: filling unrecorded fields from the machine that ran the import
  const [none] = toHistoryEntries([row(null)], meta);
  for (const f of ['kind', 'id', 'os', 'arch', 'cpu', 'cores', 'memory_gb', 'node', 'concurrent_runs']) assert.ok(isUnknown(none.host[f]), f);
  assert.notEqual(none.host.id, 'importer-vm');
  assert.notEqual(none.host.cores, 4);
  const [partial] = toHistoryEntries([row({ ...SRC, arch: undefined, node: undefined })], meta);
  assert.ok(isUnknown(partial.host.arch) && isUnknown(partial.host.node));
  assert.equal(partial.host.id, 'box-source');
  assert.deepEqual(unknownFields(partial).sort(), ['host.arch', 'host.node']);
});

test('unknown protocol fields never match anything, including another unknown', () => {
  // Catches: two entries that both lack the same host field being treated as the same protocol
  const mk = (commit, med) => ({ ...toHistoryEntries([{ ...row({ ...SRC, arch: undefined }), metrics: { first_paint_ms: med } }], { ...meta, commit })[0] });
  const a = mk('c1', 100), b = mk('c2', 100);
  assert.ok(unknownFields(a).length);
  const { comparable } = baselineFor(b, [a], 5);
  assert.equal(comparable.length, 0);
  assert.equal(evaluate([b], [a], th)[0].status, 'no-comparable-baseline');
  assert.match(evaluate([b], [a], th)[0].note, /host\.arch/);
  // a fully known current entry does not use an unknown-field entry as a baseline either
  const known = toHistoryEntries([row(SRC)], { ...meta, commit: 'c3' })[0];
  assert.equal(baselineFor(known, [a], 5).comparable.length, 0);
  assert.notEqual(protocolId(a), protocolId(known));
});

test('the 42 L/XL history entries carry the measuring host and the correction is already applied', () => {
  // Catches: the corrected history being reverted, or re-running the correction changing lines again
  const r = spawnSync('node', [join(here, '../correct-lxl-history-host.mjs'), '--check'], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stdout + r.stderr);
});
