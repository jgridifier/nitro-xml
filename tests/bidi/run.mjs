// SPDX-License-Identifier: MIT
/**
 * Stage-3 bidi/RTL gate (#44): logical-order copy + visual threshold.
 */

import { writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { BIDI_CASES } from './registry.mjs';
import { runCopyChecks } from './copy.mjs';
import { runVisualChecks } from './visual.mjs';

const OUT = join(dirname(fileURLToPath(import.meta.url)), 'out');

async function main() {
	mkdirSync(OUT, { recursive: true });
	const copy = await runCopyChecks();
	console.log(copy.summary);
	const visual = await runVisualChecks();
	console.log(visual.summary);

	const deferred = BIDI_CASES.filter((c) => !c.ready).map((c) => `${c.id}: ${c.todo}`);
	const ok = copy.ok && visual.ok;
	const summary = [
		`bidi: ${ok ? 'PASS' : 'FAIL'}`,
		copy.summary,
		visual.summary,
		...(deferred.length ? ['deferred cases:', ...deferred.map((d) => `  - ${d}`)] : []),
	].join('\n');
	writeFileSync(join(OUT, 'gate-summary.txt'), summary + '\n');
	if (process.env.GITHUB_STEP_SUMMARY) {
		const { appendFileSync } = await import('node:fs');
		appendFileSync(
			process.env.GITHUB_STEP_SUMMARY,
			`## Bidi gate (#44)\n\n\`\`\`\n${summary}\n\`\`\`\n`,
		);
	}
	console.log(ok ? 'bidi: PASS' : 'bidi: FAIL');
	process.exit(ok ? 0 : 1);
}

await main();
