#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Bundle and binary sizes per engine (#7): raw, gzip (level 9) and brotli (quality 11) bytes.
 *   ts    perf/dist/ts/<format>.js (the upstream builds the TS engine ships in the baseline)
 *   wasm  packages/nitro-xml/pkg/*.wasm + the wasm-bindgen glue, and the loader JS (loader/*.js, lazy/*.js)
 * Run `node perf/build-ts.mjs` / `bash scripts/build-wasm.sh` first. Missing inputs are reported, not zeroed.
 *
 *   node perf/sizes.mjs [--out sizes.json]
 */
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { brotliCompressSync, constants, gzipSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..');

export function sizeOf(file) {
  const b = readFileSync(file);
  return {
    file: relative(repo, file),
    raw: b.length,
    gzip: gzipSync(b, { level: 9 }).length,
    brotli: brotliCompressSync(b, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }).length,
  };
}

const jsIn = (dir) => (existsSync(dir) ? readdirSync(dir).filter((f) => f.endsWith('.js')).map((f) => join(dir, f)) : []);

export function collectSizes() {
  const pkg = join(repo, 'packages/nitro-xml');
  const groups = {
    ts: ['xlsx', 'docx', 'pptx'].map((f) => join(here, 'dist/ts', `${f}.js`)),
    wasm: [join(pkg, 'pkg/nitro_xml_wasm_bg.wasm'), join(pkg, 'pkg/nitro_xml_wasm.js')],
    wasm_loader: [...jsIn(join(pkg, 'loader')), ...jsIn(join(pkg, 'loader/lazy'))],
  };
  const out = {};
  const missing = [];
  for (const [engine, files] of Object.entries(groups)) {
    const rows = [];
    for (const f of files) (existsSync(f) ? rows.push(sizeOf(f)) : missing.push(relative(repo, f)));
    const total = rows.reduce((t, r) => ({ raw: t.raw + r.raw, gzip: t.gzip + r.gzip, brotli: t.brotli + r.brotli }), { raw: 0, gzip: 0, brotli: 0 });
    out[engine] = { files: rows, total };
  }
  return { sizes: out, missing };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const { sizes, missing } = collectSizes();
  console.log('group         file                                               raw      gzip    brotli');
  for (const [g, { files, total }] of Object.entries(sizes)) {
    for (const r of files) console.log(`${g.padEnd(13)} ${r.file.padEnd(46)} ${String(r.raw).padStart(9)} ${String(r.gzip).padStart(9)} ${String(r.brotli).padStart(9)}`);
    if (files.length) console.log(`${g.padEnd(13)} ${'TOTAL'.padEnd(46)} ${String(total.raw).padStart(9)} ${String(total.gzip).padStart(9)} ${String(total.brotli).padStart(9)}`);
  }
  if (missing.length) console.warn(`missing (not built): ${missing.join(', ')}`);
  const i = process.argv.indexOf('--out');
  if (i > 0) writeFileSync(process.argv[i + 1], JSON.stringify({ sizes, missing }, null, 2));
}
