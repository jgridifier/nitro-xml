// SPDX-License-Identifier: MIT
/**
 * Lossless round-trip harness (#49 / #65):
 *   - TS-vs-TS open → save untouched (E0 inventory → ts-losses.json)
 *   - Rust-vs-original no-op PackageWriter (#65) must meet or beat that bar
 *
 *   bun parity/roundtrip/run.mjs
 *   bun parity/roundtrip/run.mjs --engine rust
 *   bun parity/roundtrip/run.mjs --engine both --ci
 *   bun parity/roundtrip/run.mjs --mode touch          # rebuild path (#65 proof)
 *   bun parity/roundtrip/run.mjs --format xlsx
 *   bun parity/roundtrip/run.mjs --fixture <id>
 *
 * Identity note: Rust `--engine rust` uses the identity shortcut (original bytes).
 * `--mode touch` dirties one part and rebuilds — that is the real writer proof.
 *
 * Writes:
 *   parity/roundtrip/out/summary.json
 *   parity/roundtrip/out/rust-summary.json   (when engine includes rust)
 *   parity/roundtrip/out/touch-summary.json  (when --mode touch)
 *   parity/roundtrip/ts-losses.json          (TS inventory; ratchet down only)
 */

import { mkdirSync, writeFileSync, readFileSync, appendFileSync, existsSync } from 'node:fs';
import { execSync } from 'node:child_process';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	ROOT,
	loadManifest,
	selectRoundtripCorpus,
	fixtureAbs,
	fixtureExists,
	packageKind,
	familyOf,
} from './corpus.mjs';
import { comparePackages, compareTouchRebuild } from './compare.mjs';
import { tsNoopSave } from './ts-save.mjs';
import {
	rustNoopSave,
	rustTouchSave,
	ensureOpcNoopSave,
	ensureOpcTouchSave,
} from './rust-save.mjs';
import { validateOpenXml, errorSubset, libreOfficeReopen } from './validate.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = join(HERE, 'out');
const LOSSES_PATH = join(HERE, 'ts-losses.json');

function parseArgs(argv) {
	const out = {
		format: null,
		fixture: null,
		limit: 0,
		ci: false,
		engine: 'ts',
		mode: 'identity',
		negativeControl: false,
	};
	for (let i = 2; i < argv.length; i++) {
		const a = argv[i];
		if (a === '--format') out.format = argv[++i];
		else if (a === '--fixture') out.fixture = argv[++i];
		else if (a === '--limit') out.limit = Number(argv[++i]);
		else if (a === '--engine') out.engine = String(argv[++i] || 'ts').toLowerCase();
		else if (a === '--mode') out.mode = String(argv[++i] || 'identity').toLowerCase();
		else if (a === '--negative-control') out.negativeControl = true;
		else if (a === '--ci') out.ci = true;
		else if (a === '--help' || a === '-h') {
			console.log(
				'Usage: bun run.mjs [--ci] [--engine ts|rust|both] [--mode identity|touch] [--negative-control] [--format xlsx|docx|pptx] [--fixture id] [--limit N]',
			);
			process.exit(0);
		}
	}
	if (!['ts', 'rust', 'both'].includes(out.engine)) {
		console.error(`unknown --engine ${out.engine} (want ts|rust|both)`);
		process.exit(2);
	}
	if (!['identity', 'touch'].includes(out.mode)) {
		console.error(`unknown --mode ${out.mode} (want identity|touch)`);
		process.exit(2);
	}
	if (out.mode === 'touch') out.engine = 'rust';
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

function tallyCategories(rows) {
	const cat = new Map();
	for (const r of rows) {
		if (r.lossless) continue;
		for (const loss of r.losses || []) {
			cat.set(loss.category, (cat.get(loss.category) || 0) + 1);
		}
	}
	return [...cat.entries()]
		.sort((a, b) => b[1] - a[1])
		.map(([category, count]) => ({ category, count }));
}

function emptyByFormat() {
	return {
		xlsx: { lossless: 0, lossy: 0, n: 0 },
		docx: { lossless: 0, lossy: 0, n: 0 },
		pptx: { lossless: 0, lossy: 0, n: 0 },
	};
}

function countByFormat(results) {
	const byFormat = emptyByFormat();
	for (const r of results) {
		const f = r.format;
		if (!byFormat[f]) continue;
		byFormat[f].n++;
		if (r.lossless) byFormat[f].lossless++;
		else byFormat[f].lossy++;
	}
	return byFormat;
}

async function saveWithEngine(engine, input, row) {
	if (engine === 'ts') return await tsNoopSave(input, row);
	if (engine === 'rust') return rustNoopSave(input);
	throw new Error(`unknown engine ${engine}`);
}

async function runOne(row, engine) {
	const abs = fixtureAbs(row);
	const kind = packageKind(row);
	const fam = familyOf(kind);
	const input = new Uint8Array(readFileSync(abs));
	let output;
	let saveError = null;
	try {
		output = await saveWithEngine(engine, input, row);
	} catch (e) {
		saveError = String(e && e.message ? e.message : e).slice(0, 300);
	}

	if (saveError) {
		const vIn = validateOpenXml(abs);
		return {
			id: row.id,
			path: row.path,
			format: fam,
			kind,
			tier: row.tier,
			engine,
			lossless: false,
			byteIdentical: false,
			saveError,
			losses: [{ category: 'save-error', detail: saveError }],
			partCounts: { input: 0, output: 0 },
			validator: vIn.skipped
				? { skipped: true, reason: vIn.reason, ok: false }
				: {
						skipped: false,
						ok: true,
						inputErrors: (vIn.errors || []).length,
						outputErrors: 0,
						extras: 0,
						saveError: true,
					},
			libreoffice: { skipped: false, ok: true, saveError: true, count: null },
			hasVba: (row.features || []).includes('vba') || kind.endsWith('m'),
		};
	}

	const byteIdentical =
		input.length === output.length && input.every((b, i) => b === output[i]);
	// Identity save is trivially lossless even when the bytes are not a zip
	// (JSZip/compare would throw on corrupt EOCD fixtures).
	let cmp;
	if (byteIdentical) {
		cmp = { lossless: true, losses: [], partCounts: { input: 0, output: 0 } };
	} else {
		cmp = await comparePackages(input, output);
	}

	const vIn = validateOpenXml(abs);
	let validator = { skipped: vIn.skipped, reason: vIn.reason, ok: vIn.skipped };
	if (!vIn.skipped) {
		const tmp = join(OUT, `_tmp_${engine}_${row.id}.${kind}`);
		writeFileSync(tmp, output);
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

	let lo = libreOfficeReopen(input, output, kind);
	// Identity save cannot worsen LibreOffice reopen: if the input itself is
	// unreadable (encrypted / non-zip), treat the equal-fate output as OK.
	if (byteIdentical && lo && !lo.skipped && lo.ok === false && String(lo.detail || '').startsWith('input:')) {
		lo = { ...lo, ok: true, identityInputUnreadable: true };
	}

	const losses = [...cmp.losses];
	if (!validator.skipped && validator.ok === false) {
		losses.push({
			category: 'validator',
			detail: `Open XML SDK new errors: ${validator.extras}`,
		});
	}
	if (!lo.skipped && lo.ok === false) {
		losses.push({
			category: 'libreoffice',
			detail: lo.detail || `count ${lo.count} != ${lo.expectCount}`,
		});
	}

	return {
		id: row.id,
		path: row.path,
		format: fam,
		kind,
		tier: row.tier,
		features: row.features || [],
		engine,
		lossless: cmp.lossless && validator.ok !== false && lo.ok !== false,
		byteIdentical,
		losses,
		partCounts: cmp.partCounts,
		validator,
		libreoffice: lo,
		hasVba: (row.features || []).includes('vba') || kind.endsWith('m'),
	};
}

function buildInventory(results, gate, note, writeHardReqs) {
	const byFormat = countByFormat(results);
	const categories = tallyCategories(results);
	const vbaRows = results.filter((r) => r.hasVba);
	const vbaOk = vbaRows.filter((r) => {
		const vbaLoss = (r.losses || []).find(
			(l) => (l.part && /vbaproject/i.test(l.part)) || l.category === 'vba',
		);
		return !vbaLoss;
	});

	const inventory = {
		generatedAt: new Date().toISOString(),
		upstreamPin: 'e66aaec24adc0b68472eae1fc42aa4598e09c352',
		gate,
		note,
		counts: {
			total: results.length,
			lossless: results.filter((r) => r.lossless).length,
			lossy: results.filter((r) => !r.lossless).length,
			byteIdentical: results.filter((r) => r.byteIdentical).length,
			byFormat,
			vba: { n: vbaRows.length, preserved: vbaOk.length },
		},
		topLossCategories: categories.slice(0, 20),
		hardRequirements: writeHardReqs
			? results.filter((r) => r.lossless).map((r) => r.id)
			: undefined,
		allowList: results
			.filter((r) => !r.lossless)
			.map((r) => ({
				id: r.id,
				path: r.path,
				format: r.format,
				kind: r.kind,
				byteIdentical: r.byteIdentical,
				saveError: r.saveError || null,
				losses: r.losses,
			})),
	};
	if (!writeHardReqs) delete inventory.hardRequirements;
	return { inventory, categories, byFormat };
}

function formatSummary(label, inventory, byFormat, categories, results) {
	const validatorSkipped = results.filter((r) => r.validator?.skipped);
	const validatorFailed = results.filter(
		(r) => r.validator && !r.validator.skipped && r.validator.ok === false,
	);
	const loSkipped = results.filter((r) => r.libreoffice?.skipped);
	const loFailed = results.filter(
		(r) => r.libreoffice && !r.libreoffice.skipped && r.libreoffice.ok === false,
	);
	return [
		`${label}: ${inventory.counts.lossy === 0 ? 'ALL LOSSLESS' : `lossless ${inventory.counts.lossless}/${inventory.counts.total}`}`,
		`per-format: xlsx lossless ${byFormat.xlsx.lossless}/${byFormat.xlsx.n} · docx ${byFormat.docx.lossless}/${byFormat.docx.n} · pptx ${byFormat.pptx.lossless}/${byFormat.pptx.n}`,
		`byte-identical: ${inventory.counts.byteIdentical}/${inventory.counts.total}`,
		`vbaProject preserved: ${inventory.counts.vba.preserved}/${inventory.counts.vba.n}`,
		`validators (#48): openxml fail ${validatorFailed.length}/${results.length} · skipped ${validatorSkipped.length}; libreoffice fail ${loFailed.length}/${results.length} · skipped ${loSkipped.length}`,
		`top loss categories: ${
			categories
				.slice(0, 8)
				.map((c) => `${c.category}=${c.count}`)
				.join(', ') || '(none)'
		}`,
	].join('\n');
}

async function runOneTouch(row) {
	const abs = fixtureAbs(row);
	const kind = packageKind(row);
	const fam = familyOf(kind);
	const input = new Uint8Array(readFileSync(abs));
	let touched;
	let saveError = null;
	try {
		touched = rustTouchSave(input);
	} catch (e) {
		saveError = String(e && e.message ? e.message : e).slice(0, 300);
	}

	if (saveError) {
		// Non-zip / encrypted packages cannot rebuild — skip, not a writer regression.
		const skip =
			/invalid Zip|EOCD|Could not find EOCD|encrypted|MissingPart/i.test(saveError);
		return {
			id: row.id,
			path: row.path,
			format: fam,
			kind,
			engine: 'rust-touch',
			skipped: skip,
			lossless: false,
			byteIdentical: false,
			saveError,
			dirtyPart: null,
			losses: [{ category: skip ? 'skip-not-opc' : 'save-error', detail: saveError }],
			partCounts: { input: 0, output: 0 },
			validator: { skipped: true, ok: true },
			libreoffice: { skipped: true, ok: true },
			hasVba: (row.features || []).includes('vba') || kind.endsWith('m'),
		};
	}

	const output = touched.bytes;
	const dirtyPart = touched.dirtyPart;
	const byteIdentical =
		input.length === output.length && input.every((b, i) => b === output[i]);
	const cmp = await compareTouchRebuild(input, output, dirtyPart);

	const vIn = validateOpenXml(abs);
	let validator = { skipped: vIn.skipped, reason: vIn.reason, ok: vIn.skipped };
	if (!vIn.skipped) {
		const tmp = join(OUT, `_tmp_touch_${row.id}.${kind}`);
		writeFileSync(tmp, output);
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

	let lo = libreOfficeReopen(input, output, kind);
	if (lo && !lo.skipped && lo.ok === false && String(lo.detail || '').startsWith('input:')) {
		lo = { ...lo, ok: true, identityInputUnreadable: true };
	}

	const losses = [...cmp.losses];
	if (!validator.skipped && validator.ok === false) {
		losses.push({
			category: 'validator',
			detail: `Open XML SDK new errors: ${validator.extras}`,
		});
	}
	if (!lo.skipped && lo.ok === false) {
		losses.push({
			category: 'libreoffice',
			detail: lo.detail || `count ${lo.count} != ${lo.expectCount}`,
		});
	}
	if (byteIdentical) {
		losses.push({
			category: 'identity-shortcut',
			detail: 'touch save returned original bytes — rebuild path not exercised',
		});
	}

	return {
		id: row.id,
		path: row.path,
		format: fam,
		kind,
		tier: row.tier,
		features: row.features || [],
		engine: 'rust-touch',
		skipped: false,
		lossless: losses.length === 0 && cmp.lossless && validator.ok !== false && lo.ok !== false,
		byteIdentical,
		dirtyPart,
		losses,
		partCounts: cmp.partCounts,
		validator,
		libreoffice: lo,
		hasVba: (row.features || []).includes('vba') || kind.endsWith('m'),
	};
}

async function runTouchMode(rows, args) {
	console.log(
		`roundtrip Rust touch-rebuild (dirty one part + raw_copy carry): ${rows.length} fixtures`,
	);
	console.log(
		'note: identity no-op uses a copy shortcut; this mode proves PackageWriter::rebuild',
	);

	const results = [];
	let i = 0;
	for (const row of rows) {
		i++;
		const tagLabel = `[${i}/${rows.length}] ${row.id}`;
		try {
			const r = await runOneTouch(row);
			results.push(r);
			if (r.skipped) {
				console.log(`  SKIP ${tagLabel} (${r.saveError?.slice(0, 60) || 'not opc'})`);
				continue;
			}
			const tag = r.lossless ? 'LOSSLESS' : 'LOSSY';
			const n = r.losses?.length || 0;
			console.log(
				`  ${tag} ${tagLabel} dirty=${r.dirtyPart || '?'}${n ? ` (${n} losses)` : ''}`,
			);
		} catch (e) {
			const err = String(e && e.stack ? e.stack : e).slice(0, 400);
			results.push({
				id: row.id,
				path: row.path,
				format: familyOf(packageKind(row)),
				kind: packageKind(row),
				engine: 'rust-touch',
				skipped: false,
				lossless: false,
				byteIdentical: false,
				losses: [{ category: 'harness-error', detail: err }],
				saveError: err,
			});
			console.log(`  ERROR ${tagLabel}: ${err.split('\n')[0]}`);
		}
	}

	const eligible = results.filter((r) => !r.skipped);
	const { inventory, categories, byFormat } = buildInventory(
		eligible,
		'E1-a-rust-touch-rebuild',
		'Rust PackageWriter touch-rebuild: one dirty part rewritten; others raw_copy (CD crc/method/size/order). Proves rebuild path (identity shortcut is separate).',
		false,
	);
	inventory.counts.skipped = results.length - eligible.length;
	inventory.counts.totalFixtures = results.length;

	writeFileSync(
		join(OUT, 'touch-summary.json'),
		JSON.stringify({ inventory, results }, null, 2),
	);

	const summary = [
		formatSummary('roundtrip-touch', inventory, byFormat, categories, eligible),
		`skipped (not opc): ${inventory.counts.skipped}`,
		`identity shortcut used: ${eligible.filter((r) => r.byteIdentical).length} (must be 0)`,
	].join('\n');
	writeFileSync(join(OUT, 'touch-gate-summary.txt'), summary + '\n');
	console.log(summary);

	if (args.negativeControl && eligible.length) {
		const sample = eligible.find((r) => r.lossless) || eligible[0];
		const abs = fixtureAbs(rows.find((r) => r.id === sample.id));
		const input = new Uint8Array(readFileSync(abs));
		// Pick a carry victim: first part that isn't the dirty one.
		const { listZipCentralDirectory } = await import('./zip-cd.mjs');
		let corruptName = null;
		try {
			const cd = listZipCentralDirectory(input);
			corruptName = cd.find((e) => e.name !== sample.dirtyPart)?.name || null;
		} catch {
			corruptName = null;
		}
		if (!corruptName) {
			console.error('negative-control: could not pick a carry part');
			return { results, inventory, byFormat, categories, summary, exitCode: 1 };
		}
		console.log(
			`negative-control: corrupt carry of ${corruptName} on ${sample.id} (dirty=${sample.dirtyPart})`,
		);
		const bad = rustTouchSave(input, { corruptCarry: corruptName });
		const cmp = await compareTouchRebuild(input, bad.bytes, bad.dirtyPart);
		if (cmp.lossless) {
			console.error(
				'negative-control: FAIL — expected check to detect flipped compression, but compare was lossless',
			);
			return { results, inventory, byFormat, categories, summary, exitCode: 1 };
		}
		const methodLoss = cmp.losses.some(
			(l) => l.category === 'zip-cd-method' || l.category === 'zip-cd-crc' || l.category === 'zip-cd-csize',
		);
		if (!methodLoss) {
			console.error(
				`negative-control: FAIL — losses did not include zip-cd-* metadata: ${JSON.stringify(cmp.losses.slice(0, 5))}`,
			);
			return { results, inventory, byFormat, categories, summary, exitCode: 1 };
		}
		console.log(
			`negative-control: OK — check failed as expected (${cmp.losses.length} losses, e.g. ${cmp.losses[0].category} on ${cmp.losses[0].part || '?'})`,
		);
		console.log('negative-control: reverted (corrupt path only used for this probe)');
	}

	let exitCode = 0;
	if (args.ci && eligible.length === 0) {
		console.error('roundtrip (touch): no eligible fixtures');
		exitCode = 1;
	}
	if (eligible.length && eligible.every((r) => (r.losses || []).some((l) => l.category === 'harness-error'))) {
		console.error('roundtrip (touch): every fixture hit a harness error');
		exitCode = 1;
	}
	const lossy = eligible.filter((r) => !r.lossless);
	if (args.ci && lossy.length) {
		console.error(`roundtrip (touch): ${lossy.length} lossy fixtures`);
		exitCode = 1;
	}

	const requireValidators =
		args.ci ||
		process.env.NITRO_REQUIRE_VALIDATORS === '1' ||
		process.env.NITRO_REQUIRE_VALIDATORS === 'true';
	if (requireValidators) {
		const validatorSkipped = eligible.filter((r) => r.validator?.skipped);
		const loSkipped = eligible.filter((r) => r.libreoffice?.skipped);
		if (validatorSkipped.length || loSkipped.length) {
			console.error(
				`roundtrip (touch): validators required but skipped (openxml=${validatorSkipped.length}, lo=${loSkipped.length})`,
			);
			exitCode = 1;
		}
	}

	return { results, inventory, byFormat, categories, summary, exitCode };
}

async function runEngine(engine, rows, args) {
	const label =
		engine === 'ts' ? 'roundtrip TS-vs-TS no-op save' : 'roundtrip Rust-vs-original no-op save (identity shortcut)';
	console.log(`${label}: ${rows.length} fixtures`);

	const results = [];
	let i = 0;
	for (const row of rows) {
		i++;
		const tagLabel = `[${i}/${rows.length}] ${row.id}`;
		try {
			const r = await runOne(row, engine);
			results.push(r);
			const tag = r.lossless ? 'LOSSLESS' : r.byteIdentical ? 'BYTE' : 'LOSSY';
			const n = r.losses?.length || 0;
			console.log(
				`  ${tag} ${tagLabel}${n ? ` (${n} losses)` : ''}${r.saveError ? ' SAVE_ERR' : ''}`,
			);
		} catch (e) {
			const err = String(e && e.stack ? e.stack : e).slice(0, 400);
			results.push({
				id: row.id,
				path: row.path,
				format: familyOf(packageKind(row)),
				kind: packageKind(row),
				engine,
				lossless: false,
				byteIdentical: false,
				losses: [{ category: 'harness-error', detail: err }],
				saveError: err,
			});
			console.log(`  ERROR ${tagLabel}: ${err.split('\n')[0]}`);
		}
	}

	const gate = engine === 'ts' ? 'E0-a-ts-vs-ts' : 'E1-a-rust-vs-original';
	const note =
		engine === 'ts'
			? 'TS no-op save losses. Rust writer (#65) must meet or beat this bar. Ratchet down only.'
			: 'Rust PackageWriter no-op save vs original package bytes (#65). Must meet or beat ts-losses.json.';
	const { inventory, categories, byFormat } = buildInventory(
		results,
		gate,
		note,
		engine === 'ts',
	);

	const outName = engine === 'ts' ? 'summary.json' : 'rust-summary.json';
	writeFileSync(join(OUT, outName), JSON.stringify({ inventory, results }, null, 2));
	if (engine === 'ts') {
		writeFileSync(LOSSES_PATH, JSON.stringify(inventory, null, 2) + '\n');
	}

	const summary = formatSummary(
		engine === 'ts' ? 'roundtrip-ts' : 'roundtrip-rust',
		inventory,
		byFormat,
		categories,
		results,
	);
	const summaryFile = engine === 'ts' ? 'gate-summary.txt' : 'rust-gate-summary.txt';
	writeFileSync(join(OUT, summaryFile), summary + '\n');
	console.log(summary);
	if (engine === 'ts') {
		console.log('inventory: parity/roundtrip/ts-losses.json');
	}

	return { results, inventory, byFormat, categories, summary };
}

function loadTsBar() {
	if (!existsSync(LOSSES_PATH)) return null;
	try {
		return JSON.parse(readFileSync(LOSSES_PATH, 'utf8'));
	} catch {
		return null;
	}
}

/**
 * Rust must not regress vs the committed TS bar:
 * - every hardRequirement id in this run must stay lossless under Rust
 * - per-format / total lossless counts must be ≥ TS when the run covers that format fully
 */
function assertBeatsTsBar(rustInventory, rustResults, tsBar) {
	const failures = [];
	const rustById = new Map(rustResults.map((r) => [r.id, r]));
	const partial = rustResults.length < (tsBar.counts?.total || 0);
	const hard = tsBar.hardRequirements || [];
	for (const id of hard) {
		const r = rustById.get(id);
		if (!r) {
			if (partial) continue;
			failures.push(`missing hardRequirement fixture ${id}`);
			continue;
		}
		if (!r.lossless) {
			failures.push(`hardRequirement ${id} not lossless under Rust`);
		}
	}

	const tsCounts = tsBar.counts || {};
	const tsFmt = tsCounts.byFormat || {};
	const rustFmt = rustInventory.counts.byFormat;
	for (const fam of ['xlsx', 'docx', 'pptx']) {
		const tsN = tsFmt[fam]?.lossless ?? 0;
		const rustN = rustFmt[fam]?.lossless ?? 0;
		const tsTotal = tsFmt[fam]?.n ?? 0;
		const rustTotal = rustFmt[fam]?.n ?? 0;
		// Only compare counts when this run covers the same fixture set for the format.
		if (rustTotal !== tsTotal) continue;
		if (rustN < tsN) {
			failures.push(`${fam}: Rust lossless ${rustN} < TS ${tsN}`);
		}
	}
	if (!partial && (rustInventory.counts.lossless || 0) < (tsCounts.lossless || 0)) {
		failures.push(
			`total: Rust lossless ${rustInventory.counts.lossless} < TS ${tsCounts.lossless}`,
		);
	}

	return failures;
}

function compareByteIdenticalBar(rustResults, tsBar) {
	const failures = [];
	// Reconstruct TS byte-identical set from allowList (lossy but byte-identical — rare)
	// and from note that hardRequirements for docx are typically byte-identical.
	// The committed inventory does not list per-id byteIdentical for lossless rows.
	// Enforce using a companion field if present; otherwise skip.
	if (!Array.isArray(tsBar.byteIdenticalIds)) return failures;
	const rustById = new Map(rustResults.map((r) => [r.id, r]));
	for (const id of tsBar.byteIdenticalIds) {
		const r = rustById.get(id);
		if (r && !r.byteIdentical) {
			failures.push(`TS byte-identical ${id} not byte-identical under Rust`);
		}
	}
	return failures;
}

async function main() {
	const args = parseArgs(process.argv);
	assertPin();
	mkdirSync(OUT, { recursive: true });

	const manifest = loadManifest();
	let rows = selectRoundtripCorpus(manifest, { format: args.format || undefined });
	if (args.fixture) {
		rows = rows.filter((r) => r.id === args.fixture);
		if (!rows.length) {
			console.error(`fixture not found or not eligible: ${args.fixture}`);
			process.exit(2);
		}
	}
	rows = rows.filter((r) => fixtureExists(r));
	if (args.limit > 0) rows = rows.slice(0, args.limit);

	if (args.mode === 'touch') {
		ensureOpcTouchSave();
		const touch = await runTouchMode(rows, args);
		writeFileSync(join(OUT, 'gate-summary.txt'), touch.summary + '\n');
		if (process.env.GITHUB_STEP_SUMMARY) {
			appendFileSync(
				process.env.GITHUB_STEP_SUMMARY,
				`## Round-trip touch-rebuild (#65)\n\n\`\`\`\n${touch.summary}\n\`\`\`\n`,
			);
		}
		process.exit(touch.exitCode || 0);
	}

	if (args.engine === 'rust' || args.engine === 'both') {
		ensureOpcNoopSave();
	}

	const engines =
		args.engine === 'both' ? ['ts', 'rust'] : args.engine === 'rust' ? ['rust'] : ['ts'];

	/** @type {Record<string, Awaited<ReturnType<typeof runEngine>>>} */
	const runs = {};
	for (const engine of engines) {
		runs[engine] = await runEngine(engine, rows, args);
	}

	const requireValidators =
		args.ci ||
		process.env.NITRO_REQUIRE_VALIDATORS === '1' ||
		process.env.NITRO_REQUIRE_VALIDATORS === 'true';

	let exitCode = 0;
	const combinedLines = [];

	for (const engine of engines) {
		const { results, inventory, byFormat, summary } = runs[engine];
		combinedLines.push(summary);

		if (args.ci && results.length === 0) {
			console.error(`roundtrip (${engine}): no fixtures selected`);
			exitCode = 1;
		}
		const allHarnessErrors =
			results.length > 0 &&
			results.every((r) => (r.losses || []).some((l) => l.category === 'harness-error'));
		if (allHarnessErrors) {
			console.error(`roundtrip (${engine}): every fixture hit a harness error`);
			exitCode = 1;
		}

		const validatorSkipped = results.filter((r) => r.validator?.skipped);
		const validatorFailed = results.filter(
			(r) => r.validator && !r.validator.skipped && r.validator.ok === false,
		);
		const loSkipped = results.filter((r) => r.libreoffice?.skipped);
		const loFailed = results.filter(
			(r) => r.libreoffice && !r.libreoffice.skipped && r.libreoffice.ok === false,
		);

		if (requireValidators) {
			if (validatorSkipped.length || loSkipped.length) {
				console.error(
					`roundtrip (${engine}): validators required but skipped (openxml=${validatorSkipped.length}, lo=${loSkipped.length}). Run scripts/ci/setup-validators.sh.`,
				);
				exitCode = 1;
			}
			const strict =
				process.env.NITRO_VALIDATORS_STRICT === '1' ||
				process.env.NITRO_VALIDATORS_STRICT === 'true';
			if (strict && (validatorFailed.length || loFailed.length)) {
				console.error(
					`roundtrip (${engine}): STRICT validator failures openxml=${validatorFailed.length} libreoffice=${loFailed.length}`,
				);
				exitCode = 1;
			}
		}

		if (engine === 'rust') {
			const tsBar = loadTsBar();
			if (!tsBar) {
				console.error('roundtrip (rust): missing parity/roundtrip/ts-losses.json bar');
				exitCode = 1;
			} else {
				const fail = [
					...assertBeatsTsBar(inventory, results, tsBar),
					...compareByteIdenticalBar(results, tsBar),
				];
				const cmpLine = [
					`Rust vs TS lossless: xlsx ${byFormat.xlsx.lossless}/${byFormat.xlsx.n} vs ${tsBar.counts?.byFormat?.xlsx?.lossless ?? '?'}/${tsBar.counts?.byFormat?.xlsx?.n ?? '?'}`,
					`docx ${byFormat.docx.lossless}/${byFormat.docx.n} vs ${tsBar.counts?.byFormat?.docx?.lossless ?? '?'}/${tsBar.counts?.byFormat?.docx?.n ?? '?'}`,
					`pptx ${byFormat.pptx.lossless}/${byFormat.pptx.n} vs ${tsBar.counts?.byFormat?.pptx?.lossless ?? '?'}/${tsBar.counts?.byFormat?.pptx?.n ?? '?'}`,
					`total ${inventory.counts.lossless}/${inventory.counts.total} vs ${tsBar.counts?.lossless ?? '?'}/${tsBar.counts?.total ?? '?'}`,
				].join(' · ');
				console.log(cmpLine);
				combinedLines.push(cmpLine);
				if (fail.length) {
					console.error(`roundtrip (rust): worse than ts-losses.json:\n  - ${fail.join('\n  - ')}`);
					exitCode = 1;
				} else {
					console.log('roundtrip (rust): meets or beats ts-losses.json');
				}
			}
		}
	}

	if (engines.includes('ts') && engines.includes('rust')) {
		const ts = runs.ts.byFormat;
		const rust = runs.rust.byFormat;
		const head = [
			'per-format lossless Rust vs TS:',
			`  xlsx  Rust ${rust.xlsx.lossless}/${rust.xlsx.n}  TS ${ts.xlsx.lossless}/${ts.xlsx.n}`,
			`  docx  Rust ${rust.docx.lossless}/${rust.docx.n}  TS ${ts.docx.lossless}/${ts.docx.n}`,
			`  pptx  Rust ${rust.pptx.lossless}/${rust.pptx.n}  TS ${ts.pptx.lossless}/${ts.pptx.n}`,
		].join('\n');
		console.log(head);
		combinedLines.push(head);
		writeFileSync(join(OUT, 'comparison.txt'), combinedLines.join('\n\n') + '\n');
	}

	writeFileSync(join(OUT, 'gate-summary.txt'), combinedLines.join('\n\n') + '\n');

	if (process.env.GITHUB_STEP_SUMMARY) {
		appendFileSync(
			process.env.GITHUB_STEP_SUMMARY,
			`## Round-trip gate (#49 / #65 / #48)\n\n\`\`\`\n${combinedLines.join('\n\n')}\n\`\`\`\n`,
		);
	}

	process.exit(exitCode);
}

await main();
