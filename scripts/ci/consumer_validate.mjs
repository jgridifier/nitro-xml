// SPDX-License-Identifier: MIT
// Catches: edited consumer saves introducing OOXML errors or failing LibreOffice reopen.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { validateOpenXml, errorSubset, libreOfficeReopen } from '../../parity/roundtrip/validate.mjs';
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
  // Verify text edits persist in the actual XML, beyond checking ZIP magic.
  const xml = execFileSync('unzip', ['-p', output, '*.xml'], { maxBuffer: 32 * 1024 * 1024 }).toString();
  for (const op of entry.ops) if (typeof op.text === 'string' && !op.text.startsWith('=')) {
    const escaped = op.text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
    assert.ok(xml.includes(escaped), `Saved XML missing ${op.text}`);
  }
  results.push({ file: name, inputErrors: before.errors, outputErrors: after.errors, newErrors: validation.extras, libreOffice: lo });
  console.log(`${name}: no new OOXML errors; LibreOffice ${lo.expectCount} → ${lo.count}`);
}
writeFileSync(join(out, 'validation.json'), JSON.stringify(results, null, 2) + '\n');
