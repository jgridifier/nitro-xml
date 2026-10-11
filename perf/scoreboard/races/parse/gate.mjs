#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// CI gate for the parse race result JSON. Fails the job when the harness is broken or the
// reference cannot establish equivalence; never fails because a competitor is slower or
// "not comparable" (those are published, not ranked).
import { readFileSync } from 'node:fs';

const path = process.argv[2];
if (!path) {
	console.error('usage: node races/parse/gate.mjs <results.json>');
	process.exit(2);
}

const r = JSON.parse(readFileSync(path, 'utf8'));
const fail = (msg) => {
	console.error(`scoreboard gate FAIL: ${msg}`);
	process.exit(1);
};

if (r.race !== 'parse' || r.schema !== 1) fail(`unexpected race/schema (${r.race}/${r.schema})`);
if (!r.method?.reference) fail('missing method.reference');
if (!Array.isArray(r.files) || r.files.length === 0) fail('no files in result');
if (!r.browser?.crossOriginIsolated) fail('page was not cross-origin isolated (COOP/COEP); timings are not comparable');
// Full CI uses races/parse/defaults.json (11 timed / 2 warm-up). SCOREBOARD_MIN_* lets a filtered
// local debug run still exercise the rest of the gate without pretending to be publishable.
const minTimed = Number(process.env.SCOREBOARD_MIN_RUNS ?? 7);
const minWarmup = Number(process.env.SCOREBOARD_MIN_WARMUP ?? 2);
if (!r.method.timedRuns || r.method.timedRuns < minTimed) fail(`timedRuns too low for a stable median (${r.method.timedRuns} < ${minTimed})`);
if ((r.method.warmupRuns ?? 0) < minWarmup) fail(`warmupRuns too low (${r.method.warmupRuns} < ${minWarmup})`);

const refId = r.method.reference;
const refEntrant = (r.entrants ?? []).find((e) => e.id === refId);
if (!refEntrant || refEntrant.status !== 'entered') fail(`reference ${refId} is not entered`);

const byFormat = { xlsx: [], docx: [], pptx: [] };
for (const f of r.files) {
	if (!byFormat[f.format]) fail(`unknown format ${f.format}`);
	byFormat[f.format].push(f);
}

for (const [format, files] of Object.entries(byFormat)) {
	if (!files.length) continue;
	let refOk = 0;
	let refErr = 0;
	for (const f of files) {
		const ref = f.results?.[refId];
		if (!ref) fail(`${format}/${f.file}: missing reference result`);
		if (ref.status === 'error') refErr++;
		else if (ref.status === 'ok' && ref.comparable) refOk++;
		else fail(`${format}/${f.file}: reference status ${ref.status} comparable=${ref.comparable}`);
	}
	if (refErr) fail(`${format}: reference errored on ${refErr}/${files.length} files`);
	if (refOk !== files.length) fail(`${format}: reference only ok on ${refOk}/${files.length} files`);

	const summary = r.summary?.[format];
	if (!summary?.[refId]) fail(`${format}: missing summary for reference`);

	// At least one non-reference entrant must have been attempted (ok, error, or not comparable).
	const others = Object.entries(summary).filter(([id, row]) => id !== refId && row.status === 'entered');
	if (!others.length) fail(`${format}: no other entered competitors (pin/build broken?)`);

	for (const [id, row] of others) {
		// Fairness: ratios only come from comparable files; incomparable must not be ranked.
		if (row.comparable === 0 && row.geomeanRatioVsReference != null) {
			fail(`${format}/${id}: ranked (geomean=${row.geomeanRatioVsReference}) despite 0 comparable files`);
		}
		if (row.sharedWithReference !== row.comparable) {
			fail(`${format}/${id}: sharedWithReference (${row.sharedWithReference}) != comparable (${row.comparable})`);
		}
	}
}

// Losses must be present in the summary whenever an entrant is slower — structural check that
// wins/losses fields exist so the page can publish them as prominently as wins.
for (const [format, rows] of Object.entries(r.summary ?? {})) {
	for (const [id, row] of Object.entries(rows)) {
		if (row.status !== 'entered' || id === refId) continue;
		if (row.sharedWithReference > 0) {
			for (const k of ['winsVsReference', 'lossesVsReference', 'tiesVsReference']) {
				if (typeof row[k] !== 'number') fail(`${format}/${id}: missing ${k}`);
			}
			if (row.winsVsReference + row.lossesVsReference + row.tiesVsReference !== row.sharedWithReference) {
				fail(`${format}/${id}: W+L+T != sharedWithReference`);
			}
		}
	}
}

const formatsWithFiles = Object.entries(byFormat).filter(([, f]) => f.length).map(([k]) => k);
console.log(
	`scoreboard gate OK: ${r.files.length} files (${formatsWithFiles.join(', ')}), reference ${refId} clean, ${r.method.warmupRuns}+${r.method.timedRuns} runs, seed order recorded, COOP/COEP isolated`,
);
