// SPDX-License-Identifier: MIT
/**
 * E0 edit-script harness (#50): TS-vs-TS apply → save over edits/*.json.
 *
 *   bun parity/edit/run.mjs
 *   bun parity/edit/run.mjs --format xlsx
 *   bun parity/edit/run.mjs --id xlsx-set-cell-number
 *
 * Writes:
 *   parity/edit/out/summary.json
 *   parity/edit/out/expected-diffs.json  (input→output part losses for Rust #63+)
 *   parity/edit/out/gate-summary.txt
 */

import { mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { execSync } from 'node:child_process';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { comparePackages } from '../roundtrip/compare.mjs';
import { validateOpenXml, errorSubset, libreOfficeReopen } from '../roundtrip/validate.mjs';
import { familyOf, packageKind } from '../roundtrip/corpus.mjs';
import { applyAndSave } from './apply.mjs';
import { loadEditScripts, resolveFixture, fixtureAbs, ROOT } from './scripts.mjs';

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

/**
 * Fingerprint of part-level losses for the Rust editor to match.
 * @param {import('../roundtrip/compare.mjs').CompareResult} cmp
 */
function diffFingerprint(cmp) {
	return {
		lossless: cmp.lossless,
		partCounts: cmp.partCounts,
		losses: (cmp.losses || []).map((l) => ({
			category: l.category,
			part: l.part,
			detail: l.detail,
		})),
	};
}

async function runOne(script) {
	const row = resolveFixture(script);
	const abs = fixtureAbs(row);
	const kind = packageKind(row);
	const fam = familyOf(kind);
	const input = new Uint8Array(readFileSync(abs));

	/** @type {Uint8Array | null} */
	let out1 = null;
	/** @type {Uint8Array | null} */
	let out2 = null;
	/** @type {string | null} */
	let applyError = null;

	try {
		out1 = await applyAndSave(input, script);
		out2 = await applyAndSave(input, script);
	} catch (e) {
		applyError = String(e && e.message ? e.message : e).slice(0, 400);
	}

	if (applyError || !out1 || !out2) {
		return {
			id: script.id,
			format: script.format,
			fixture: script.fixture,
			ok: false,
			deterministic: false,
			applyError,
			expectedDiff: null,
			validator: { skipped: true },
			libreoffice: { skipped: true },
		};
	}

	const det = await comparePackages(out1, out2);
	// Double-save can drift only in docProps timestamps (dcterms:modified).
	const deterministic =
		det.lossless ||
		det.losses.every(
			(l) => l.category === 'docProps' || (l.part && String(l.part).startsWith('docProps/')),
		);
	const vsInput = await comparePackages(input, out1);

	const vIn = validateOpenXml(abs);
	let validator = { skipped: vIn.skipped, reason: vIn.reason };
	if (!vIn.skipped) {
		const tmp = join(OUT, `_tmp_${script.id}.${kind || script.format}`);
		writeFileSync(tmp, out1);
		const vOut = validateOpenXml(tmp);
		const sub = errorSubset(vIn.errors, vOut.errors || []);
		validator = {
			skipped: false,
			ok: sub.ok,
			inputErrors: (vIn.errors || []).length,
			outputErrors: (vOut.errors || []).length,
			extras: sub.extras.length,
		};
	}

	let libreoffice = { skipped: true };
	if (kind) {
		const lo = libreOfficeReopen(out1, kind);
		libreoffice = lo;
	}

	const ok = deterministic && (validator.skipped || validator.ok !== false);

	return {
		id: script.id,
		format: script.format,
		fixture: script.fixture,
		description: script.description,
		ok,
		deterministic,
		determinismLosses: deterministic ? [] : det.losses,
		expectedDiff: diffFingerprint(vsInput),
		outputBytes: out1.length,
		inputBytes: input.length,
		validator,
		libreoffice: {
			skipped: !!libreoffice.skipped,
			ok: libreoffice.ok,
			reason: libreoffice.reason,
		},
		family: fam,
		kind,
	};
}

async function main() {
	const args = parseArgs(process.argv);
	assertPin();
	mkdirSync(OUT, { recursive: true });

	const scripts = loadEditScripts({ format: args.format || undefined, id: args.id || undefined });
	if (!scripts.length) {
		console.error('edit: no scripts matched');
		process.exit(1);
	}

	/** @type {any[]} */
	const rows = [];
	for (const script of scripts) {
		process.stdout.write(`edit: ${script.id} … `);
		const row = await runOne(script);
		rows.push(row);
		console.log(row.ok ? 'ok' : `FAIL${row.applyError ? ' ' + row.applyError : ''}`);
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
		gate: 'edit-parity',
		issue: 50,
		scripts: rows.length,
		ok: rows.length - failed.length,
		fail: failed.length,
		byFormat,
		failedIds: failed.map((r) => r.id),
	};

	const expectedDiffs = Object.fromEntries(
		rows.filter((r) => r.expectedDiff).map((r) => [r.id, r.expectedDiff]),
	);

	writeFileSync(join(OUT, 'summary.json'), JSON.stringify({ summary, rows }, null, 2) + '\n');
	writeFileSync(join(OUT, 'expected-diffs.json'), JSON.stringify(expectedDiffs, null, 2) + '\n');

	const lines = [
		`edit-parity: ${summary.ok}/${summary.scripts} scripts ok`,
		`  xlsx ${byFormat.xlsx.ok}/${byFormat.xlsx.total}`,
		`  docx ${byFormat.docx.ok}/${byFormat.docx.total}`,
		`  pptx ${byFormat.pptx.ok}/${byFormat.pptx.total}`,
	];
	if (failed.length) {
		lines.push(`  FAIL: ${failed.map((r) => r.id).join(', ')}`);
	}
	const gateText = lines.join('\n') + '\n';
	writeFileSync(join(OUT, 'gate-summary.txt'), gateText);
	process.stdout.write(gateText);

	if (args.ci && failed.length) process.exit(1);
	if (!args.ci && failed.length) process.exit(1);
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
