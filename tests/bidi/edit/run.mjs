// SPDX-License-Identifier: MIT
/** Bidi editing gate runner (#57). Invoked from scripts/ci/bidi.sh after #44 viewer checks. */

import { writeFileSync, mkdirSync, readFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runEditGate } from './parity.mjs';

const OUT = join(dirname(fileURLToPath(import.meta.url)), 'out');

async function main() {
	mkdirSync(OUT, { recursive: true });
	const edit = await runEditGate();
	console.log(edit.summary);
	writeFileSync(join(OUT, 'gate-summary.txt'), edit.summary + '\n');
	const viewerSummary = join(dirname(OUT), 'out', 'gate-summary.txt');
	let combined = edit.summary;
	if (existsSync(viewerSummary)) {
		combined = readFileSync(viewerSummary, 'utf8').trim() + '\n' + edit.summary;
	}
	writeFileSync(join(OUT, 'combined-gate-summary.txt'), combined + '\n');
	if (!edit.ok) process.exit(1);
}

await main();
