// SPDX-License-Identifier: MIT
/**
 * TS oracle: apply a docx edits/*.json script and dump touched block model state (#78).
 *
 * Usage:
 *   bun parity/edit/docx_model_dump.mjs <fixture.docx> <script.json> <out.json>
 *
 * Snapshot shape (matches Rust DocxHost::model_snapshot):
 *   { format: 'docx', blocks: [{ index, type, align, runs: [...] }...] }
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { applyDocxOps } from './apply.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');

function upstreamRoot() {
	return process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
}

function projectRun(r) {
	return {
		text: r.text ?? '',
		bold: !!r.bold,
		italic: !!r.italic,
		underline: !!r.underline,
		fontSize: r.fontSize ?? null,
		color: r.color ?? null,
	};
}

function projectBlock(index, b) {
	if (b.type === 'table') {
		return {
			index,
			type: 'table',
			rows: (b.rows || []).map((row) =>
				(row || []).map((cell) => ({
					paragraphs: (cell.paragraphs || []).map((p) => ({
						align: p.align ?? null,
						runs: (p.runs || []).map(projectRun),
					})),
				})),
			),
		};
	}
	return {
		index,
		type: b.type || 'paragraph',
		align: b.align ?? null,
		runs: (b.runs || []).map(projectRun),
	};
}

function collectTouched(ops, out) {
	for (const op of ops) {
		switch (op.op) {
			case 'setParagraphText':
			case 'setRunFormatting':
			case 'setParagraphAlign':
				out.add(op.blockIndex);
				break;
			case 'batch':
				collectTouched(op.ops || [], out);
				break;
			default:
				break;
		}
	}
}

async function main() {
	const [, , fixturePath, scriptPath, outPath] = process.argv;
	if (!fixturePath || !scriptPath || !outPath) {
		console.error(
			'Usage: bun docx_model_dump.mjs <fixture.docx> <script.json> <out.json>',
		);
		process.exit(2);
	}
	const up = upstreamRoot();
	const { loadDocx } = await import(`${up}/src/core/docx/parse.ts`);
	const bytes = new Uint8Array(readFileSync(fixturePath));
	const script = JSON.parse(readFileSync(scriptPath, 'utf8'));
	const loaded = await loadDocx(bytes);
	const touched = new Set();
	collectTouched(script.ops || [], touched);
	applyDocxOps(loaded.model, script.ops);
	const blocks = [...touched]
		.sort((a, b) => a - b)
		.map((i) => projectBlock(i, loaded.model.blocks[i]));
	writeFileSync(
		outPath,
		JSON.stringify({ format: 'docx', blocks }, null, 0) + '\n',
	);
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
