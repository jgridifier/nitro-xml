// SPDX-License-Identifier: MIT
// Catches: missing runtime exports or accidental source/test/secret files in npm tarballs.
import assert from 'node:assert/strict';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';

const out = resolve(process.argv[2] || '/tmp/nitro-pack-check');
mkdirSync(out, { recursive: true });
for (const dir of ['nitro-xml', 'nitro-xml-react']) {
  const cwd = resolve('packages', dir);
  const pkg = JSON.parse(readFileSync(`${cwd}/package.json`));
  const raw = execFileSync('npm', ['pack', '--dry-run', '--json'], { cwd, encoding: 'utf8' });
  const [pack] = JSON.parse(raw);
  const files = new Set(pack.files.map(f => f.path));
  const allowed = dir === 'nitro-xml'
    ? /^(?:package\.json|index\.(?:js|d\.ts)|README\.md|LICENSE(?:-THIRD-PARTY)?|NOTICE|pkg\/nitro_xml_wasm(?:_bg\.wasm)?\.(?:js|d\.ts)|pkg\/nitro_xml_wasm_bg\.wasm|loader\/(?:[\w/-]+\.(?:js|d\.ts)|(?:lazy\/)?README\.md))$/
    : /^(?:package\.json|README\.md|LICENSE|dist\/[\w-]+\.(?:js|d\.ts))$/;
  for (const file of files) {
    assert.ok(allowed.test(file), `Unexpected package file: ${file}`);
    assert.ok(!/(^|\/)(test|tests|fixtures|src|node_modules)(\/|$)|\.map$|\.env|\.pem$|\.key$/i.test(file), file);
  }
  const check = value => {
    if (typeof value === 'string' && value.startsWith('./') && !value.includes('*'))
      assert.ok(files.has(value.slice(2)), `Missing export ${value}`);
    else if (value && typeof value === 'object') Object.values(value).forEach(check);
  };
  check(pkg.exports);
  for (const key of ['main', 'module', 'types']) check(pkg[key]);
  for (const file of ['README.md', 'LICENSE']) assert.ok(files.has(file), file);
  assert.equal(pkg.version, '0.1.0');
  if (dir === 'nitro-xml') {
    for (const file of ['NOTICE', 'LICENSE-THIRD-PARTY', 'pkg/nitro_xml_wasm_bg.wasm']) assert.ok(files.has(file), file);
    assert.ok(pack.unpackedSize < 15 * 1024 * 1024, 'Unexpected package growth (pack sanity check, not loader budget)');
  } else {
    assert.equal(pkg.dependencies['nitro-xml'], '^0.1.0');
    assert.equal(pkg.peerDependencies.react, '>=18');
    assert.equal(pkg.peerDependencies['react-dom'], '>=18');
    assert.ok(pack.unpackedSize < 100 * 1024, 'Unexpected React package growth');
  }
  writeFileSync(`${out}/${dir}.json`, JSON.stringify(pack, null, 2) + '\n');
  console.log(`${pkg.name}: ${pack.size} B packed; ${pack.unpackedSize} B unpacked; ${files.size} files`);
  console.log([...files].join('\n'));
}
