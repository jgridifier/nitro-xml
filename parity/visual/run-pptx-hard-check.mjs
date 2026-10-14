// SPDX-License-Identifier: MIT
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { writeFileSync } from 'node:fs';
import { pptxVisualCorpus } from './pptx-corpus.mjs';

const VISUAL = dirname(fileURLToPath(import.meta.url));
const { spawnSync } = await import('node:child_process');

const cases = pptxVisualCorpus().filter((c) => !c.informational);
const ids = cases.map((c) => c.id.replace(/^pptx-/, ''));
const r = spawnSync('bun', ['./run-pptx-focus.mjs', ...ids], {
	cwd: VISUAL,
	encoding: 'utf8',
	env: { ...process.env, PATH: `${process.env.HOME}/.bun/bin:${process.env.PATH}` },
});
const rows = r.stdout
	.split('\n')
	.filter((l) => l.startsWith('{'))
	.map((l) => JSON.parse(l));
const fail = rows.filter((row) => !row.pass && !row.err);
writeFileSync(join(VISUAL, 'out/pptx-hard-check.json'), JSON.stringify({ fail, rows }, null, 2));
console.error(`hard check: ${rows.length - fail.length}/${rows.length} pass at ≤0.1%`);
for (const row of fail.sort((a, b) => Math.max(b.dpr1, b.dpr2) - Math.max(a.dpr1, a.dpr2))) {
	console.error(`  FAIL ${row.id} dpr1=${row.dpr1} dpr2=${row.dpr2}`);
}
