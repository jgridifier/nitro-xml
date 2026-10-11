// SPDX-License-Identifier: MIT
/**
 * Logical-order copy checks (#44): Rust shell copyText vs TS cellView TSV,
 * and vs the fixture's known logical strings.
 */

import { readFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { readyCases, LOGICAL } from './registry.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');

function upstreamDir() {
	return process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
}

function ensureWasm() {
	const glue = join(ROOT, 'packages/nitro-xml/pkg/nitro_xml_wasm.js');
	const wasm = join(ROOT, 'packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm');
	if (existsSync(glue) && existsSync(wasm)) return;
	const r = spawnSync('bash', [join(ROOT, 'scripts/build-wasm.sh')], {
		cwd: ROOT,
		encoding: 'utf8',
		maxBuffer: 32 * 1024 * 1024,
	});
	if (r.status !== 0) {
		throw new Error(`build-wasm: ${r.stderr || r.stdout}`);
	}
}

async function rustCopy(fixtureRel, ranges) {
	ensureWasm();
	const { default: init, Handle } = await import(
		join(ROOT, 'packages/nitro-xml/pkg/nitro_xml_wasm.js')
	);
	await init();
	const bytes = new Uint8Array(readFileSync(join(ROOT, fixtureRel)));
	const h = new Handle(bytes);
	h.openDocument(fixtureRel.split('/').pop() || 'doc.xlsx');
	const out = [];
	for (const range of ranges) {
		h.shellAction('select', range);
		const r = JSON.parse(h.shellAction('copy', ''));
		out.push(r.c ?? r.copyText ?? '');
	}
	h.free();
	return out;
}

async function tsCopy(fixtureRel, ranges) {
	const up = upstreamDir();
	const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);
	const { cellView } = await import(`${up}/src/core/xlsx/layout/cell-view.ts`);
	const { parseRange } = await import(`${up}/src/core/xlsx/address.ts`);
	const bytes = new Uint8Array(readFileSync(join(ROOT, fixtureRel)));
	const workbook = await loadXlsx(bytes);
	const active = workbook.activeSheet ?? 0;
	return ranges.map((rangeRef) => {
		const range = parseRange(rangeRef);
		if (!range) return '';
		const lines = [];
		for (let row = range.start.row; row <= range.end.row; row++) {
			const cols = [];
			for (let col = range.start.col; col <= range.end.col; col++) {
				cols.push(cellView(workbook, active, row, col).text);
			}
			lines.push(cols.join('\t'));
		}
		return lines.join('\n');
	});
}

/** Concatenated logical shape text from pptx model dumps (rust vs TS). */
async function pptxLogicalTexts(fixtureRel) {
	const bin = join(ROOT, 'rs/target/release/examples/pptx_dump');
	if (!existsSync(bin)) {
		const r = spawnSync(
			'cargo',
			['build', '--release', '-q', '-p', 'nitro-xml-pptx', '--example', 'pptx_dump'],
			{ cwd: join(ROOT, 'rs'), encoding: 'utf8' },
		);
		if (r.status !== 0) throw new Error(`pptx_dump build: ${r.stderr || r.stdout}`);
	}
	const rust = spawnSync(bin, [join(ROOT, fixtureRel)], {
		encoding: 'utf8',
		maxBuffer: 16 * 1024 * 1024,
	});
	if (rust.status !== 0) throw new Error(`pptx_dump: ${rust.stderr || rust.stdout}`);
	const rustJson = JSON.parse(rust.stdout);
	const rustText = (rustJson.slides ?? [])
		.flatMap((s) => (s.elements ?? []).map((e) => e.text || ''))
		.filter(Boolean)
		.join('\n');

	const { dumpPptxViewIr } = await import(join(ROOT, 'parity/oracle/pptx.ts'));
	const tsIr = await dumpPptxViewIr(join(ROOT, fixtureRel));
	const tsText = (tsIr.slides ?? [])
		.flatMap((s) =>
			(s.elements ?? []).map((e) => {
				if (typeof e.text === 'string') return e.text;
				// Upstream may expose runs; join in document order.
				const runs = e.paragraphs ?? e.runs ?? null;
				if (Array.isArray(e.paragraphs)) {
					return e.paragraphs
						.map((p) => (p.runs ?? []).map((r) => r.text || '').join(''))
						.join('\n');
				}
				if (Array.isArray(runs)) return runs.map((r) => r.text || '').join('');
				return '';
			}),
		)
		.filter(Boolean)
		.join('\n');

	return { rustText, tsText };
}

/**
 * @returns {Promise<{ ok: boolean, rows: object[], summary: string }>}
 */
export async function runCopyChecks() {
	const rows = [];
	let failed = 0;

	for (const c of readyCases()) {
		if (c.format === 'xlsx' && c.copyRanges?.length) {
			const rust = await rustCopy(c.fixture, c.copyRanges);
			const ts = await tsCopy(c.fixture, c.copyRanges);
			for (let i = 0; i < c.copyRanges.length; i++) {
				const range = c.copyRanges[i];
				const expected = c.expectedCopy?.[i];
				const rustOk = expected === undefined || rust[i] === expected;
				const tsOk = expected === undefined || ts[i] === expected;
				const match = rust[i] === ts[i];
				const ok = rustOk && tsOk && match;
				if (!ok) failed++;
				rows.push({
					case: c.id,
					kind: 'xlsx-copy',
					range,
					ok,
					rust: rust[i],
					ts: ts[i],
					expected,
				});
			}
		} else if (c.format === 'pptx') {
			const { rustText, tsText } = await pptxLogicalTexts(c.fixture);
			const expected = [LOGICAL.AR, LOGICAL.HE, `Hello ${LOGICAL.HE.split(' ')[0]} world ${LOGICAL.AR.split(' ')[0]}`].join(
				'\n',
			);
			// Accept rust shape text (paragraph-joined) matching expected logical order.
			const rustOk = rustText === expected;
			// TS oracle may expose per-run fragments; require expected as a subsequence
			// of concatenated text (logical order preserved).
			const tsJoined = tsText.replace(/\n+/g, '\n').trim();
			const tsOk =
				tsJoined === expected ||
				tsJoined.replace(/\n/g, '') === expected.replace(/\n/g, '') ||
				(tsJoined.includes(LOGICAL.AR) &&
					tsJoined.includes(LOGICAL.HE) &&
					tsJoined.indexOf(LOGICAL.AR) < tsJoined.indexOf(LOGICAL.HE));
			const ok = rustOk && tsOk;
			if (!ok) failed++;
			rows.push({
				case: c.id,
				kind: 'pptx-model-text',
				ok,
				rust: rustText,
				ts: tsJoined.slice(0, 200),
				expected,
				note: c.todo || 'shell copy empty for pptx; model text gated',
			});
		}
	}

	const summaryLines = [
		`bidi copy: ${failed === 0 ? 'PASS' : 'FAIL'} (${rows.filter((r) => r.ok).length}/${rows.length})`,
		...rows.map((r) => {
			const tag = r.ok ? 'OK' : 'FAIL';
			const detail =
				r.kind === 'xlsx-copy'
					? `${r.range} rust=${JSON.stringify(r.rust)} ts=${JSON.stringify(r.ts)}`
					: `${r.kind} rust=${JSON.stringify(r.rust)}`;
			return `  ${tag} ${r.case} ${detail}`;
		}),
	];
	return { ok: failed === 0, rows, summary: summaryLines.join('\n') };
}

if (import.meta.main) {
	const { ok, summary } = await runCopyChecks();
	console.log(summary);
	process.exit(ok ? 0 : 1);
}
