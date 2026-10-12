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

function projectRevision(rev) {
	if (!rev) return null;
	return {
		id: rev.id,
		kind: rev.kind,
		author: rev.author,
		...(rev.move ? { move: rev.move } : {}),
	};
}

function projectRun(r) {
	return {
		text: r.text ?? '',
		bold: !!r.bold,
		italic: !!r.italic,
		underline: !!r.underline,
		fontSize: r.fontSize ?? null,
		color: r.color ?? null,
		commentIds: r.commentIds ?? null,
		revision: projectRevision(r.revision),
		formatRevision: projectRevision(r.formatRevision),
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
						markRevision: projectRevision(p.markRevision),
						formatRevision: projectRevision(p.formatRevision),
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
		markRevision: projectRevision(b.markRevision),
		formatRevision: projectRevision(b.formatRevision),
		runs: (b.runs || []).map(projectRun),
	};
}

function projectComments(comments) {
	return (comments || []).map((c) => ({
		id: c.id ?? null,
		author: c.author ?? null,
		text: c.text ?? null,
		resolved: !!c.resolved,
		parentId: c.parentId ?? null,
	}));
}

const REVISION_OPS = new Set([
	'acceptRevision',
	'rejectRevision',
	'acceptAllRevisions',
	'rejectAllRevisions',
]);

const COMMENT_OPS = new Set([
	'addComment',
	'replyToComment',
	'resolveComment',
	'deleteComment',
]);

function collectTouched(ops, out, blockCount) {
	for (const op of ops) {
		switch (op.op) {
			case 'setParagraphText':
			case 'setRunFormatting':
			case 'setParagraphAlign':
			case 'setTableCellText':
			case 'insertTableRow':
			case 'deleteTableRow':
			case 'insertTableColumn':
			case 'deleteTableColumn':
			case 'addComment':
				out.add(op.blockIndex);
				break;
			case 'acceptAllRevisions':
			case 'rejectAllRevisions':
			case 'deleteComment':
				for (let i = 0; i < blockCount; i++) out.add(i);
				break;
			case 'acceptRevision':
			case 'rejectRevision':
				for (let i = 0; i < blockCount; i++) out.add(i);
				break;
			case 'replyToComment':
			case 'resolveComment':
				break;
			case 'batch':
				collectTouched(op.ops || [], out, blockCount);
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
	collectTouched(script.ops || [], touched, loaded.model.blocks.length);
	applyDocxOps(loaded.model, script.ops);
	const blocks = [...touched]
		.sort((a, b) => a - b)
		.map((i) => projectBlock(i, loaded.model.blocks[i]));
	const out = { format: 'docx', blocks };
	const touchesComments = (script.ops || []).some((op) => COMMENT_OPS.has(op.op));
	if (touchesComments) {
		out.comments = projectComments(loaded.model.comments);
	}
	writeFileSync(outPath, JSON.stringify(out, null, 0) + '\n');
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
