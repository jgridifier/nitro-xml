// SPDX-License-Identifier: MIT
/**
 * TS oracle: apply a pptx edits/*.json script and dump the snapshotPptx model (#84).
 *
 * Usage:
 *   bun parity/edit/pptx_model_dump.mjs <fixture.pptx> <script.json> <out.json>
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { applyPptxOps, loadPptxMutators } from './apply.mjs';
import { snapshotPptx, stableStringify } from '../undo/snapshot.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');

function upstreamRoot() {
	return process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
}

function toArrayBuffer(u8) {
	return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
}

async function main() {
	const [, , fixturePath, scriptPath, outPath] = process.argv;
	if (!fixturePath || !scriptPath || !outPath) {
		console.error('Usage: bun pptx_model_dump.mjs <fixture.pptx> <script.json> <out.json>');
		process.exit(2);
	}
	const up = upstreamRoot();
	const { PptxHandler } = await import(`${up}/src/core/pptx/core/PptxHandler.ts`);
	const bytes = new Uint8Array(readFileSync(fixturePath));
	const script = JSON.parse(readFileSync(scriptPath, 'utf8'));
	const handler = new PptxHandler();
	const data = await handler.load(toArrayBuffer(bytes));
	const mut = await loadPptxMutators();
	const slides = applyPptxOps(data.slides, script.ops, mut);
	writeFileSync(outPath, stableStringify(snapshotPptx(slides)) + '\n');
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
