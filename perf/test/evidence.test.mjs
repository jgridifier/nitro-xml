// SPDX-License-Identifier: MIT
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const script = join(here, '../verify-issue10-evidence.mjs');
const src = join(here, '../baselines/issue-10-evidence');
const run = (dir) => spawnSync('node', [script], { env: { ...process.env, ISSUE10_EVIDENCE_DIR: dir }, encoding: 'utf8' });

test('committed evidence subset verifies and re-derives the status totals', () => {
  // Catches: a classified cell whose status is not backed by evidence in the committed subset
  const r = run(src);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, /by status: \{\s*observed_timeout: 227,\s*observed_error: 22,\s*operator_aborted: 4,\s*not_run: 19,\s*unknown: 0\s*\}/);
});

test('verifier rejects a changed file and a claim the evidence does not support', () => {
  // Catches: editing evidence or flipping a status (e.g. calling a never-run cell a timeout) without the check failing
  const d = mkdtempSync(join(tmpdir(), 'ev-'));
  cpSync(src, d, { recursive: true });
  const ce = JSON.parse(readFileSync(join(d, 'cell-evidence.json'), 'utf8'));
  ce.cells.find((c) => c.status === 'not_run').status = 'observed_timeout';
  writeFileSync(join(d, 'cell-evidence.json'), JSON.stringify(ce));
  const r = run(d);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /sha256 mismatch cell-evidence\.json/);
  assert.match(r.stderr, /claimed observed_timeout, evidence derives not_run/);
});
