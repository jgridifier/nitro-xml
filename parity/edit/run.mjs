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
import JSZip from 'jszip';
import { comparePackages } from '../roundtrip/compare.mjs';
import { validateOpenXml, errorSubset, libreOfficeReopen } from '../roundtrip/validate.mjs';
import { familyOf, packageKind } from '../roundtrip/corpus.mjs';
import { applyAndSave } from './apply.mjs';
import { loadEditScripts, resolveFixture, fixtureAbs, ROOT } from './scripts.mjs';
import { rustDocxSave } from './rust-docx.mjs';
import { rustXlsxSave } from './rust-xlsx.mjs';
import { snapshotDocx, stableStringify } from '../undo/snapshot.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = join(HERE, 'out');

function parseArgs(argv) {
	const out = {
		format: null,
		id: null,
		ci: false,
		rustDocx: false,
		rustXlsx: false,
		negativeControl: false,
	};
	for (let i = 2; i < argv.length; i++) {
		const a = argv[i];
		if (a === '--format') out.format = argv[++i];
		else if (a === '--id') out.id = argv[++i];
		else if (a === '--ci') out.ci = true;
		else if (a === '--rust-docx') out.rustDocx = true;
		else if (a === '--rust-xlsx') out.rustXlsx = true;
		else if (a === '--negative-control') out.negativeControl = true;
		else if (a === '--help' || a === '-h') {
			console.log(
				'Usage: bun run.mjs [--ci] [--format xlsx|docx|pptx] [--id script-id] [--rust-docx] [--rust-xlsx] [--negative-control]',
			);
			process.exit(0);
		}
	}
	return out;
}

function isDocPropsLoss(loss) {
	if (loss.category === 'docProps') return true;
	const part = loss.part ? String(loss.part) : '';
	return part.startsWith('docProps/') || part.toLowerCase().includes('docprops');
}

async function documentXmlRewritten(inputBytes, outputBytes) {
	const cmp = await comparePackages(inputBytes, outputBytes);
	return (cmp.losses || []).some(
		(l) => l.category === 'xml-rewrite' && l.part === 'word/document.xml',
	);
}

async function workbookHasFullCalc(bytes) {
	const zip = await JSZip.loadAsync(bytes);
	const file = zip.file('xl/workbook.xml');
	if (!file) return false;
	const xml = await file.async('string');
	return /fullCalcOnLoad\s*=\s*"1"/.test(xml);
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

	// Model-only scripts (#82 revisions/comments): skip package save when TS/Rust
	// package rewrite cannot round-trip (poi-delins / comments.xml).
	if (script.modelOnly) {
		/** @type {string | null} */
		let applyError = null;
		/** @type {any} */
		let snap1 = null;
		/** @type {any} */
		let snap2 = null;
		try {
			snap1 = snapshotDocx(await applyDocxModel(input, script));
			snap2 = snapshotDocx(await applyDocxModel(input, script));
		} catch (e) {
			applyError = String(e && e.message ? e.message : e).slice(0, 400);
		}
		const deterministic =
			!applyError && stableStringify(snap1) === stableStringify(snap2);
		return {
			id: script.id,
			format: script.format,
			fixture: script.fixture,
			description: script.description,
			ok: deterministic,
			modelOnly: true,
			deterministic,
			applyError,
			expectedDiff: null,
			validator: { skipped: true, reason: 'modelOnly' },
			libreoffice: { skipped: true, reason: 'modelOnly' },
			family: fam,
			kind,
		};
	}

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
		const lo = libreOfficeReopen(input, out1, kind);
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

	/**
	 * @param {string} label
	 * @param {typeof scripts} subset
	 * @param {(input: Uint8Array, scriptPath: string) => Uint8Array} saveFn
	 */
	async function compareRust(label, subset, saveFn) {
		/** @type {{ id: string, losses: object[] }[]} */
		const failures = [];
		for (const script of subset) {
			process.stdout.write(`${label}: ${script.id} … `);
			try {
				const input = new Uint8Array(readFileSync(fixtureAbs(resolveFixture(script))));
				const tsOut = await applyAndSave(input, script);
				const rustOut = saveFn(input, script.path);
				const cmp = await comparePackages(tsOut, rustOut);
				const losses = (cmp.losses || []).filter((l) => !isDocPropsLoss(l));
				if (losses.length) {
					failures.push({ id: script.id, losses });
					const sample = losses
						.slice(0, 3)
						.map((l) => `${l.category}:${l.part || ''} ${l.detail}`)
						.join(' | ');
					console.log(`FAIL ${losses.length} (${sample})`);
				} else {
					console.log('ok');
				}
			} catch (e) {
				const message = String(e && e.message ? e.message : e).slice(0, 300);
				failures.push({ id: script.id, losses: [{ category: 'error', detail: message }] });
				console.log(`FAIL ${message}`);
			}
		}
		return failures;
	}

	const rustDocxFailures = args.rustDocx
		? await compareRust('rust-docx', scripts.filter((s) => s.format === 'docx' && !s.modelOnly), (input, scriptPath) =>
				rustDocxSave(input, scriptPath, { skipDocumentRewrite: false }),
			)
		: [];
	const rustXlsxFailures = args.rustXlsx
		? await compareRust('rust-xlsx', scripts.filter((s) => s.format === 'xlsx'), (input, scriptPath) =>
				rustXlsxSave(input, scriptPath, { omitFullCalc: false }),
			)
		: [];

	// Each writer's negative control runs when that writer is selected. With
	// neither --rust-docx nor --rust-xlsx, both controls run.
	const runDocxControl = args.negativeControl && (args.rustDocx || !args.rustXlsx);
	const runXlsxControl = args.negativeControl && (args.rustXlsx || !args.rustDocx);

	let negativeControlDocx = null;
	if (runDocxControl) {
		const script =
			scripts.find((s) => s.id === 'docx-set-para0-text') ||
			scripts.find((s) => s.format === 'docx');
		if (!script) {
			console.error('negative-control docx: FAIL (no docx script)');
			process.exit(1);
		}
		const input = new Uint8Array(readFileSync(fixtureAbs(resolveFixture(script))));
		const dirty = rustDocxSave(input, script.path, { skipDocumentRewrite: false });
		const skipped = rustDocxSave(input, script.path, { skipDocumentRewrite: true });
		const dirtyRewrote = await documentXmlRewritten(input, dirty);
		const skipRewrote = await documentXmlRewritten(input, skipped);
		if (!dirtyRewrote) {
			negativeControlDocx = 'fail-missing-on-dirty';
			console.error(
				'negative-control docx: FAIL (dirty save did not rewrite word/document.xml; detector had nothing to reject)',
			);
		} else if (skipRewrote) {
			negativeControlDocx = 'fail-skip-ignored';
			console.error(
				'negative-control docx: FAIL (--skip-document-rewrite still changed word/document.xml)',
			);
		} else {
			negativeControlDocx = 'pass';
			console.log(
				'negative-control docx: detector FAIL (document.xml unchanged) — expected for --skip-document-rewrite',
			);
			console.log('negative-control docx: PASS');
		}
	}

	let negativeControlXlsx = null;
	if (runXlsxControl) {
		const script =
			scripts.find((s) => s.id === 'xlsx-set-cell-number') ||
			scripts.find((s) => s.format === 'xlsx');
		if (!script) {
			console.error('negative-control xlsx: FAIL (no xlsx script)');
			process.exit(1);
		}
		const input = new Uint8Array(readFileSync(fixtureAbs(resolveFixture(script))));
		const dirty = rustXlsxSave(input, script.path, { omitFullCalc: false });
		const omitted = rustXlsxSave(input, script.path, { omitFullCalc: true });
		const dirtyHas = await workbookHasFullCalc(dirty);
		const omitHas = await workbookHasFullCalc(omitted);
		if (!dirtyHas) {
			negativeControlXlsx = 'fail-missing-on-dirty';
			console.error(
				'negative-control xlsx: FAIL (dirty save missing fullCalcOnLoad="1"; detector had nothing to reject)',
			);
		} else if (omitHas) {
			negativeControlXlsx = 'fail-omit-ignored';
			console.error(
				'negative-control xlsx: FAIL (omit-full-calc still wrote fullCalcOnLoad="1")',
			);
		} else {
			negativeControlXlsx = 'pass';
			console.log(
				'negative-control xlsx: detector FAIL (fullCalcOnLoad absent) — expected for --omit-full-calc',
			);
			console.log('negative-control xlsx: PASS');
		}
	}

	const negativeControl =
		negativeControlDocx || negativeControlXlsx
			? { docx: negativeControlDocx, xlsx: negativeControlXlsx }
			: null;

	const failed = rows.filter((r) => !r.ok);
	const summary = {
		gate: 'edit-parity',
		issue: 50,
		scripts: rows.length,
		ok: rows.length - failed.length,
		fail: failed.length,
		byFormat,
		failedIds: failed.map((r) => r.id),
		rustDocx: args.rustDocx
			? {
					compared: scripts.filter((s) => s.format === 'docx' && !s.modelOnly).length,
					fail: rustDocxFailures.length,
				}
			: null,
		rustXlsx: args.rustXlsx
			? {
					compared: scripts.filter((s) => s.format === 'xlsx').length,
					fail: rustXlsxFailures.length,
				}
			: null,
		negativeControl,
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
	if (args.rustDocx) {
		const compared = scripts.filter((s) => s.format === 'docx' && !s.modelOnly).length;
		lines.push(
			`  rust-docx ${compared - rustDocxFailures.length}/${compared} match TS (docProps allowed)`,
		);
		if (rustDocxFailures.length) {
			lines.push(`  rust-docx FAIL: ${rustDocxFailures.map((r) => r.id).join(', ')}`);
		}
	}
	if (args.rustXlsx) {
		const compared = scripts.filter((s) => s.format === 'xlsx').length;
		lines.push(
			`  rust-xlsx ${compared - rustXlsxFailures.length}/${compared} match TS (docProps allowed)`,
		);
		if (rustXlsxFailures.length) {
			lines.push(`  rust-xlsx FAIL: ${rustXlsxFailures.map((r) => r.id).join(', ')}`);
		}
	}
	if (negativeControlDocx) {
		lines.push(`  negative-control docx: ${negativeControlDocx}`);
	}
	if (negativeControlXlsx) {
		lines.push(`  negative-control xlsx: ${negativeControlXlsx}`);
	}
	const gateText = lines.join('\n') + '\n';
	writeFileSync(join(OUT, 'gate-summary.txt'), gateText);
	process.stdout.write(gateText);

	const rustBad = rustDocxFailures.length > 0 || rustXlsxFailures.length > 0;
	const controlBad =
		(negativeControlDocx && negativeControlDocx !== 'pass') ||
		(negativeControlXlsx && negativeControlXlsx !== 'pass');
	if (failed.length || rustBad || controlBad) process.exit(1);
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
