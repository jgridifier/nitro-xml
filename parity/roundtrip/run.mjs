// SPDX-License-Identifier: MIT
/**
 * E0 lossless round-trip harness (#49): TS-vs-TS open → save untouched.
 *
 *   bun parity/roundtrip/run.mjs
 *   bun parity/roundtrip/run.mjs --format xlsx
 *   bun parity/roundtrip/run.mjs --fixture <id>
 *
 * Writes:
 *   parity/roundtrip/out/summary.json
 *   parity/roundtrip/ts-losses.json  (inventory; also committed as the bar for #65)
 */

import { mkdirSync, writeFileSync, readFileSync, appendFileSync } from 'node:fs';
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
import { comparePackages } from './compare.mjs';
import { tsNoopSave } from './ts-save.mjs';
import { validateOpenXml, errorSubset, libreOfficeReopen } from './validate.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = join(HERE, 'out');
const LOSSES_PATH = join(HERE, 'ts-losses.json');

function parseArgs(argv) {
	const out = { format: null, fixture: null, limit: 0, write: false };
	for (let i = 2; i < argv.length; i++) {
		const a = argv[i];
		if (a === '--format') out.format = argv[++i];
		else if (a === '--fixture') out.fixture = argv[++i];
		else if (a === '--limit') out.limit = Number(argv[++i]);
		else if (a === '--ci') out.ci = true;
		else if (a === '--help' || a === '-h') {
			console.log(
				'Usage: bun run.mjs [--ci] [--format xlsx|docx|pptx] [--fixture id] [--limit N]',
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

async function runOne(row) {
	const abs = fixtureAbs(row);
	const kind = packageKind(row);
	const fam = familyOf(kind);
	const input = new Uint8Array(readFileSync(abs));
	let output;
	let saveError = null;
	try {
		output = await tsNoopSave(input, row);
	} catch (e) {
		saveError = String(e && e.message ? e.message : e).slice(0, 300);
	}

	if (saveError) {
		return {
			id: row.id,
			path: row.path,
			format: fam,
			kind,
			tier: row.tier,
			lossless: false,
			byteIdentical: false,
			saveError,
			losses: [{ category: 'save-error', detail: saveError }],
			partCounts: { input: 0, output: 0 },
			validator: { skipped: true },
			libreoffice: { skipped: true },
		};
	}

	const byteIdentical =
		input.length === output.length && input.every((b, i) => b === output[i]);
	const cmp = await comparePackages(input, output);

	// Validator subset (soft-skip without #48 tools)
	const vIn = validateOpenXml(abs);
	let validator = { skipped: vIn.skipped, reason: vIn.reason };
	if (!vIn.skipped) {
		const tmp = join(OUT, `_tmp_${row.id}.${kind}`);
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

	const lo = libreOfficeReopen(output, kind);

	return {
		id: row.id,
		path: row.path,
		format: fam,
		kind,
		tier: row.tier,
		features: row.features || [],
		lossless: cmp.lossless,
		byteIdentical,
		losses: cmp.losses,
		partCounts: cmp.partCounts,
		validator,
		libreoffice: lo,
		hasVba: (row.features || []).includes('vba') || kind.endsWith('m'),
	};
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

	console.log(`roundtrip: ${rows.length} fixtures (TS-vs-TS no-op save)`);

	const results = [];
	let i = 0;
	for (const row of rows) {
		i++;
		const label = `[${i}/${rows.length}] ${row.id}`;
		try {
			const r = await runOne(row);
			results.push(r);
			const tag = r.lossless ? 'LOSSLESS' : r.byteIdentical ? 'BYTE' : 'LOSSY';
			const n = r.losses?.length || 0;
			console.log(`  ${tag} ${label}${n ? ` (${n} losses)` : ''}${r.saveError ? ' SAVE_ERR' : ''}`);
		} catch (e) {
			const err = String(e && e.stack ? e.stack : e).slice(0, 400);
			results.push({
				id: row.id,
				path: row.path,
				format: familyOf(packageKind(row)),
				kind: packageKind(row),
				lossless: false,
				byteIdentical: false,
				losses: [{ category: 'harness-error', detail: err }],
				saveError: err,
			});
			console.log(`  ERROR ${label}: ${err.split('\n')[0]}`);
		}
	}

	const byFormat = { xlsx: { lossless: 0, lossy: 0, n: 0 }, docx: { lossless: 0, lossy: 0, n: 0 }, pptx: { lossless: 0, lossy: 0, n: 0 } };
	for (const r of results) {
		const f = r.format;
		if (!byFormat[f]) continue;
		byFormat[f].n++;
		if (r.lossless) byFormat[f].lossless++;
		else byFormat[f].lossy++;
	}

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
		gate: 'E0-a-ts-vs-ts',
		note: 'TS no-op save losses. Rust writer (#65) must meet or beat this bar. Ratchet down only.',
		counts: {
			total: results.length,
			lossless: results.filter((r) => r.lossless).length,
			lossy: results.filter((r) => !r.lossless).length,
			byteIdentical: results.filter((r) => r.byteIdentical).length,
			byFormat,
			vba: { n: vbaRows.length, preserved: vbaOk.length },
		},
		topLossCategories: categories.slice(0, 20),
		/** Hard requirements: fixtures where TS is already lossless. */
		hardRequirements: results.filter((r) => r.lossless).map((r) => r.id),
		/** Allow-listed losses with reasons (category + part). */
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

	writeFileSync(join(OUT, 'summary.json'), JSON.stringify({ inventory, results }, null, 2));
	writeFileSync(LOSSES_PATH, JSON.stringify(inventory, null, 2) + '\n');

	const summaryLines = [
		`roundtrip: ${inventory.counts.lossy === 0 ? 'ALL LOSSLESS' : 'PASS (inventory recorded)'}`,
		`per-format: xlsx lossless ${byFormat.xlsx.lossless}/${byFormat.xlsx.n} · docx ${byFormat.docx.lossless}/${byFormat.docx.n} · pptx ${byFormat.pptx.lossless}/${byFormat.pptx.n}`,
		`byte-identical: ${inventory.counts.byteIdentical}/${inventory.counts.total}`,
		`vbaProject preserved: ${inventory.counts.vba.preserved}/${inventory.counts.vba.n}`,
		`top loss categories: ${categories
			.slice(0, 8)
			.map((c) => `${c.category}=${c.count}`)
			.join(', ') || '(none)'}`,
		`inventory: parity/roundtrip/ts-losses.json`,
	];
	const summary = summaryLines.join('\n');
	writeFileSync(join(OUT, 'gate-summary.txt'), summary + '\n');
	console.log(summary);

	if (process.env.GITHUB_STEP_SUMMARY) {
		appendFileSync(
			process.env.GITHUB_STEP_SUMMARY,
			`## Round-trip gate (#49)\n\n\`\`\`\n${summary}\n\`\`\`\n`,
		);
	}

	// Gate passes when the harness completes and writes the inventory.
	// Fail only on harness crashes for every fixture, or --ci with zero fixtures.
	if (args.ci && results.length === 0) {
		console.error('roundtrip: no fixtures selected');
		process.exit(1);
	}
	const allHarnessErrors = results.every((r) =>
		(r.losses || []).some((l) => l.category === 'harness-error'),
	);
	if (results.length && allHarnessErrors) {
		console.error('roundtrip: every fixture hit a harness error');
		process.exit(1);
	}
	process.exit(0);
}

await main();
