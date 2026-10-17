// SPDX-License-Identifier: MIT
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { writeFileSync } from 'node:fs';
import { pptxVisualCorpus } from './pptx-corpus.mjs';
import { THRESHOLDS, PPTX_TILE_GUARD } from './thresholds.mjs';

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
// Host errors count as failures: a timed-out deck must not read as a pass.
const fail = rows.filter((row) => !row.pass);
writeFileSync(join(VISUAL, 'out/pptx-hard-check.json'), JSON.stringify({ fail, rows }, null, 2));
if (rows.length !== cases.length) {
	console.error(`hard check: only ${rows.length}/${cases.length} decks reported (runner died?)`);
	process.exitCode = 1;
}
console.error(
	`hard check: ${rows.length - fail.length}/${cases.length} pass at ≤${THRESHOLDS.pptx * 100}% + ${PPTX_TILE_GUARD.tile}px tile ≤${PPTX_TILE_GUARD.maxRatioByDpr[1] * 100}%/${PPTX_TILE_GUARD.maxRatioByDpr[2] * 100}% (DPR1/2)`,
);
for (const row of fail.sort((a, b) => Math.max(b.dpr1, b.dpr2) - Math.max(a.dpr1, a.dpr2))) {
	console.error(
		`  FAIL ${row.id} dpr1=${row.dpr1}% dpr2=${row.dpr2}% tile1=${row.tile1}% tile2=${row.tile2}%${row.err ? ` err=${row.err}` : ''}`,
	);
}
if (fail.length) process.exitCode = 1;
