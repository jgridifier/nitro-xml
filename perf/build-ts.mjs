#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Builds the TS-engine bundles for the perf hosts (#6): perf/dist/ts/{xlsx,docx,pptx}.js.
 * The bundles come from the published ooxml-ui / pptx-vanilla-viewer builds of the upstream pin
 * (ChristopherVR/ooxml @ e66aaec), never from Rust. Test/bench only; nothing here ships.
 *
 *   node perf/build-ts.mjs [--force]
 *
 * Needs git, bun (https://bun.sh) and network the first time (submodule + bun install). Steps that
 * are already done are skipped. The upstream JS builds are run without their declaration (tsc)
 * steps: the harness needs only JS, and upstream's type-check currently fails on a clean checkout.
 */
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..');
const up = join(repo, 'upstream');
const out = join(here, 'dist', 'ts');
const PIN = 'e66aaec24adc0b68472eae1fc42aa4598e09c352';
const force = process.argv.includes('--force');

function sh(cmd, args, cwd, { tolerate = false } = {}) {
  console.log(`+ ${cmd} ${args.join(' ')}  (${cwd.replace(repo, '.')})`);
  const r = spawnSync(cmd, args, { cwd, stdio: 'inherit' });
  if (r.status !== 0 && !tolerate) throw new Error(`${cmd} ${args.join(' ')} exited ${r.status}`);
  return r.status === 0;
}
const have = (rel) => existsSync(join(up, rel));
const head = () => spawnSync('git', ['-C', up, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim();

if (!have('package.json')) sh('git', ['submodule', 'update', '--init', '--depth', '1', 'upstream'], repo);
if (head() !== PIN) throw new Error(`upstream is at ${head()}, expected pin ${PIN}`);
if (!have('node_modules')) sh('bun', ['install'], up);

// upstream's own build scripts also run tsc and exit non-zero here although the JS is emitted,
// so success is judged by the output files.
if (force || !have('src/core/dist/pptx')) sh('bun', ['run', '--cwd', 'src/core', 'build'], up, { tolerate: true });
if (!have('src/core/dist/pptx')) throw new Error('ooxml-core build produced no dist/pptx');

if (force || !have('src/ui/dist/xlsx.js') || !have('src/ui/dist/docx.js')) {
  sh('npx', ['--no-install', 'tsup', '--config', 'tsup.config.ts'], join(up, 'src/ui'), { tolerate: true });
}
if (force || !have('src/ui/dist/pptx/dom/index.mjs')) {
  sh('npx', ['--no-install', 'tsup', '--config', 'tsup.pptx.config.ts'], join(up, 'src/ui'), { tolerate: true });
}
if (force || !have('viewers/pptx/packages/vanilla/dist/index.js')) {
  sh('bun', ['run', '--filter', 'pptx-viewer-core', 'build'], join(up, 'viewers/pptx'), { tolerate: true });
  sh('bun', ['run', '--filter', 'pptx-vanilla-viewer', 'build'], join(up, 'viewers/pptx'), { tolerate: true });
}
for (const f of ['src/ui/dist/xlsx.js', 'src/ui/dist/docx.js', 'viewers/pptx/packages/vanilla/dist/index.js']) {
  if (!have(f)) throw new Error(`upstream build output missing: ${f}`);
}

mkdirSync(out, { recursive: true });
for (const fmt of ['xlsx', 'docx', 'pptx']) {
  const cwd = fmt === 'pptx' ? join(up, 'viewers/pptx/packages/vanilla') : up;
  sh('bun', ['build', join(here, 'hosts', `ts-${fmt}.mjs`), '--outfile', join(out, `${fmt}.js`), '--format', 'esm', '--target', 'browser', '--minify'], cwd);
  console.log(`  -> perf/dist/ts/${fmt}.js ${(statSync(join(out, `${fmt}.js`)).size / 1024).toFixed(0)} KiB`);
}
