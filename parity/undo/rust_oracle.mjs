// SPDX-License-Identifier: MIT
/**
 * Dump TS undo-parity checkpoints for one edits/*.json script (#64 Rust compare).
 *
 * Usage:
 *   bun parity/undo/rust_oracle.mjs <fixture.xlsx> <script.json> <out.json>
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { runUndoScript } from './session.mjs';
import { stableStringify } from './snapshot.mjs';

async function main() {
	const [, , fixturePath, scriptPath, outPath] = process.argv;
	if (!fixturePath || !scriptPath || !outPath) {
		console.error(
			'Usage: bun rust_oracle.mjs <fixture.xlsx> <script.json> <out.json>',
		);
		process.exit(2);
	}
	const bytes = new Uint8Array(readFileSync(fixturePath));
	const script = JSON.parse(readFileSync(scriptPath, 'utf8'));
	const result = await runUndoScript(bytes, script);
	writeFileSync(
		outPath,
		stableStringify({
			id: script.id,
			format: script.format,
			checkpoints: result.checkpoints,
		}) + '\n',
	);
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
