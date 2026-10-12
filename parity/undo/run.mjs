// SPDX-License-Identifier: MIT
/**
 * E0 undo/redo parity harness (#51): do → undo-all → redo-all over edits/*.json.
 *
 *   bun parity/undo/run.mjs
 *   bun parity/undo/run.mjs --format xlsx
 *   bun parity/undo/run.mjs --id xlsx-set-cell-number
 *
 * Writes:
 *   parity/undo/out/summary.json
 *   parity/undo/out/snapshots/<id>.json  (checkpoint sequence for Rust #64)
 *   parity/undo/out/gate-summary.txt
 */

import { mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { execSync } from 'node:child_process';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { comparePackages } from '../roundtrip/compare.mjs';
import { loadEditScripts, resolveFixture, fixtureAbs, ROOT } from '../edit/scripts.mjs';
import { runUndoScript, snapshotsEqual, stableStringify } from './session.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = join(HERE, 'out');

function parseArgs(argv) {
	const out = { format: null, id: null, ci: false };
	for (let i = 2; i < argv.length; i++) {
		const a = argv[i];
		if (a === '--format') out.format = argv[++i];
		else if (a === '--id') out.id = argv[++i];
		else if (a === '--ci') out.ci = true;
		else if (a === '--help' || a === '-h') {
			console.log(
				'Usage: bun run.mjs [--ci] [--format xlsx|docx|pptx] [--id script-id]',
			);
			process.exit(0);
		}
	}
	return out;
}

function assertPin() {
	const up = process.env.UPSTREAM_DIR || join(ROOT, 'upstream');
	let head;
	try {
		head = execSync('git rev-parse HEAD', { cwd: up, encoding: 'utf8' }).trim();
	} catch {
		throw new Error(`upstream missing at ${up}`);
	}
	const pin = 'e66aaec24adc0b68472eae1fc42aa4598e09c352';
	if (head !== pin) throw new Error(`upstream not at pin ${pin} (got ${head})`);
}

/** Package compare allowing known residues (style table after style undo; docProps timestamps). */
function undoSaveOk(cmp) {
	if (cmp.lossless) return { ok: true, allowedResidue: false };
	const allowed = cmp.losses.every(
		(l) =>
			l.category === 'styles' ||
			(l.part && String(l.part).includes('styles')) ||
			l.category === 'docProps' ||
			(l.part && String(l.part).startsWith('docProps/')),
	);
	return { ok: allowed, allowedResidue: allowed };
}

/** Redo/edit package compare: allow docProps timestamp drift from multiple saves. */
function redoSaveOk(cmp) {
	if (cmp.lossless) return true;
	return cmp.losses.every(
		(l) => l.category === 'docProps' || (l.part && String(l.part).startsWith('docProps/')),
	);
}

async function runOne(script) {
	const row = resolveFixture(script);
	const abs = fixtureAbs(row);
	const input = new Uint8Array(readFileSync(abs));

	/** @type {any} */
	let a;
	/** @type {any} */
	let b;
	/** @type {string | null} */
	let error = null;
	try {
		a = await runUndoScript(input, script);
		b = await runUndoScript(input, script);
	} catch (e) {
		error = String(e && e.message ? e.message : e).slice(0, 400);
	}

	if (error || !a || !b) {
		const negative =
			String(script.id).includes('negative') && error && /not a table|rejected/i.test(error);
		return {
			id: script.id,
			format: script.format,
			fixture: script.fixture,
			ok: !!negative,
			error: negative ? null : error,
		};
	}

	const undoRestored = snapshotsEqual(a.start, a.afterUndoAll);
	const redoRestored = snapshotsEqual(a.afterDos, a.afterRedoAll);
	const deterministic =
		stableStringify(a.checkpoints.map((c) => ({ phase: c.phase, label: c.label, snapshot: c.snapshot }))) ===
		stableStringify(b.checkpoints.map((c) => ({ phase: c.phase, label: c.label, snapshot: c.snapshot })));

	let undoVsNoop = { lossless: true, losses: [] };
	let undoPkg = { ok: true, allowedResidue: false };
	let redoPkgOk = true;
	let redoVsEdit = { lossless: true, losses: [] };
	if (!script.modelOnly && !a.modelOnly) {
		undoVsNoop = await comparePackages(a.noopSave, a.undoSave);
		undoPkg = undoSaveOk(undoVsNoop);
		redoVsEdit = await comparePackages(
			a.redoSave,
			await (async () => {
				// redo-all save should match a fresh apply+save from the edit harness path
				const { applyAndSave } = await import('../edit/apply.mjs');
				return applyAndSave(input, script);
			})(),
		);
		redoPkgOk = redoSaveOk(redoVsEdit);
	}

	const ok =
		undoRestored &&
		redoRestored &&
		deterministic &&
		undoPkg.ok &&
		redoPkgOk;

	const snapDir = join(OUT, 'snapshots');
	mkdirSync(snapDir, { recursive: true });
	writeFileSync(
		join(snapDir, `${script.id}.json`),
		stableStringify({
			id: script.id,
			format: script.format,
			fixture: script.fixture,
			checkpoints: a.checkpoints,
		}) + '\n',
	);

	return {
		id: script.id,
		format: script.format,
		fixture: script.fixture,
		ok,
		undoRestored,
		redoRestored,
		deterministic,
		undoSaveVsNoop: {
			lossless: undoVsNoop.lossless,
			allowedResidue: undoPkg.allowedResidue,
			losses: undoVsNoop.losses,
		},
		redoSaveMatchesEdit: redoPkgOk,
		redoSaveLosses: redoVsEdit.lossless ? [] : redoVsEdit.losses,
		checkpointCounts: {
			total: a.checkpoints.length,
			do: a.checkpoints.filter((c) => c.phase === 'do').length,
			undo: a.checkpoints.filter((c) => c.phase === 'undo').length,
			redo: a.checkpoints.filter((c) => c.phase === 'redo').length,
		},
		error: null,
	};
}

async function main() {
	const args = parseArgs(process.argv);
	assertPin();
	mkdirSync(OUT, { recursive: true });

	const scripts = loadEditScripts({
		format: args.format || undefined,
		id: args.id || undefined,
	});
	if (!scripts.length) {
		console.error('undo: no scripts matched');
		process.exit(1);
	}

	/** @type {any[]} */
	const rows = [];
	for (const script of scripts) {
		process.stdout.write(`undo: ${script.id} … `);
		const row = await runOne(script);
		rows.push(row);
		if (row.ok) console.log('ok');
		else {
			const why = [
				row.error,
				!row.undoRestored && 'undo≠start',
				!row.redoRestored && 'redo≠dos',
				!row.deterministic && 'nondeterministic',
				row.undoSaveVsNoop && !row.undoSaveVsNoop.lossless && !row.undoSaveVsNoop.allowedResidue
					? 'undo-save'
					: null,
				row.redoSaveMatchesEdit === false && 'redo-save≠edit',
			]
				.filter(Boolean)
				.join(',');
			console.log(`FAIL ${why}`);
		}
	}

	const byFormat = {};
	for (const f of ['xlsx', 'docx', 'pptx']) {
		const subset = rows.filter((r) => r.format === f);
		byFormat[f] = {
			total: subset.length,
			ok: subset.filter((r) => r.ok).length,
			fail: subset.filter((r) => !r.ok).length,
		};
	}
	const failed = rows.filter((r) => !r.ok);
	const summary = {
		gate: 'undo-parity',
		issue: 51,
		scripts: rows.length,
		ok: rows.length - failed.length,
		fail: failed.length,
		byFormat,
		failedIds: failed.map((r) => r.id),
	};

	writeFileSync(join(OUT, 'summary.json'), JSON.stringify({ summary, rows }, null, 2) + '\n');
	const lines = [
		`undo-parity: ${summary.ok}/${summary.scripts} scripts ok`,
		`  xlsx ${byFormat.xlsx.ok}/${byFormat.xlsx.total}`,
		`  docx ${byFormat.docx.ok}/${byFormat.docx.total}`,
		`  pptx ${byFormat.pptx.ok}/${byFormat.pptx.total}`,
	];
	if (failed.length) lines.push(`  FAIL: ${failed.map((r) => r.id).join(', ')}`);
	const gateText = lines.join('\n') + '\n';
	writeFileSync(join(OUT, 'gate-summary.txt'), gateText);
	process.stdout.write(gateText);

	if (failed.length) process.exit(1);
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
