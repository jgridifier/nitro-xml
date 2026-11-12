// SPDX-License-Identifier: MIT
// Catches: edited consumer saves introducing OOXML errors or failing LibreOffice reopen.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { validateOpenXml, errorSubset, libreOfficeReopen } from '../../parity/roundtrip/validate.mjs';
import { applyAndSave } from '../../parity/edit/apply.mjs';
import { checkPartIdentity, expectedPptxGeometry, checkReopenedModel } from '../../parity/edit/persistence.mjs';
const out = process.argv[2];
const cases = JSON.parse(readFileSync(join(out, 'cases.json')));
const results = [];
for (const entry of cases) {
  const name = `${entry.id}.${entry.format}`;
  const input = join(out, 'originals', name), output = join(out, name);
  execFileSync('unzip', ['-t', output]);
  const before = validateOpenXml(input), after = validateOpenXml(output);
  for (const result of [before, after]) {
    assert.equal(result.skipped, false, JSON.stringify(result));
    assert.ok(!result.errors.some(e => ['validator-fail', 'validator-non-json', 'package-open'].includes(e.id)), JSON.stringify(result));
  }
  const validation = errorSubset(before.errors, after.errors);
  assert.ok(validation.ok, JSON.stringify(validation));
  const lo = libreOfficeReopen(readFileSync(input), readFileSync(output), entry.format);
  assert.ok(!lo.skipped && lo.ok, JSON.stringify(lo));
  const applied = [], undone = [];
  for (const op of entry.ops) {
    if (op.op === 'edit.undo') { undone.push(applied.pop()); continue; }
    if (op.op === 'edit.redo') { applied.push(undone.pop()); continue; }
    applied.push(op.op === 'insert.move-slide-to' ? { ...op, op: 'reorderSlide' } : op);
    undone.length = 0;
  }
  const script = { ...entry, ops: applied };
  let expected = await applyAndSave(readFileSync(input), script);
  if (entry.format === 'pptx') expected = await expectedPptxGeometry(expected, script);
  assert.deepEqual(await checkReopenedModel(entry.format, expected, readFileSync(output)), [], `${name}: edit lost after save/reopen`);
  // Reorder only changes presentation.xml; shape and text edits target their own slides.
  const identityScript = { ...script, ops: script.ops };
  assert.deepEqual(await checkPartIdentity(identityScript, readFileSync(input), readFileSync(output)), [], `${name}: untouched parts changed`);
  results.push({ file: name, inputErrors: before.errors, outputErrors: after.errors, newErrors: validation.extras, libreOffice: lo });
  console.log(`${name}: no new OOXML errors; LibreOffice ${lo.expectCount} → ${lo.count}`);
}
writeFileSync(join(out, 'validation.json'), JSON.stringify(results, null, 2) + '\n');
