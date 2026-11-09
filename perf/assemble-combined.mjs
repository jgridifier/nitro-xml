#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Assemble combined baseline JSONL for the L/XL + xlsx-rerun PR (#10 follow-up).
 * Merges: ts-4451709.jsonl (S/M sweep) + promoted xlsx S/M rerun rows + ts-lxl-<sha>.jsonl (L/XL sweep).
 * Creates: perf/baselines/ts-combined-<sha>.jsonl + .failures.jsonl + .rerun.jsonl
 *
 * Usage: node perf/assemble-combined.mjs <lxl-jsonl> [--warm4x <warm4x-jsonl>]
 *   lxl-jsonl: merged L/XL sweep file (perf/baselines/ts-lxl-<sha>.jsonl)
 *   --warm4x: xlsx rerun warm 4x results JSONL (perf/results/rerun-warm4x/warm-4x-all.jsonl)
 */
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, basename, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');

const args = process.argv.slice(2);
if (!args[0]) throw new Error('usage: node perf/assemble-combined.mjs <lxl.jsonl> [--warm4x <warm4x.jsonl>]');
const lxlFile = resolve(args[0]);
const warm4xIdx = args.indexOf('--warm4x');
const warm4xFile = warm4xIdx >= 0 ? resolve(args[warm4xIdx + 1]) : null;

const readJsonl = (f) => existsSync(f) ? readFileSync(f, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : [];

// S/M sweep (commit 4451709)
const smRows = readJsonl(join(here, 'baselines/ts-4451709.jsonl'));
const smFails = readJsonl(join(here, 'baselines/ts-4451709.failures.jsonl'));
const rerunRows = readJsonl(join(here, 'baselines/ts-4451709.rerun.jsonl'));
const rerunFails = readJsonl(join(here, 'baselines/ts-4451709.rerun-failures.jsonl'));

// L/XL sweep rows
const lxlRows = readJsonl(lxlFile);
const lxlFailFile = lxlFile.replace(/\.jsonl$/, '.failures.jsonl');
const lxlFails = readJsonl(lxlFailFile);

// Warm 4x rows (new measurement for xlsx rerun completeness)
const warm4xRows = warm4xFile ? readJsonl(warm4xFile) : [];

// Include ALL xlsx S/M rerun rows — many fixtures are partial (1 cell) in the main sweep; the rerun adds the missing
// cells. A cell that has both an original and a re-run row is two protocol groups (rerun flag differs): the report
// (baseline-report.mjs, protocol-rows.mjs) keeps them as separate rows and never adds their samples.
const manifest = JSON.parse(readFileSync(join(root, 'fixtures/manifest.json'), 'utf8')).fixtures;
const xlsxSmIds = new Set(manifest.filter((m) => m.format === 'xlsx' && ['S', 'M'].includes(m.tier)).map((m) => m.id));
const promotedRerunRows = rerunRows.filter((r) => xlsxSmIds.has(r.fixture));
const promotedWarm4xRows = warm4xRows.filter((r) => xlsxSmIds.has(r.fixture));

// Combined main JSONL: S/M sweep + promoted xlsx rerun + warm 4x + L/XL
const combinedRows = [...smRows, ...promotedRerunRows, ...promotedWarm4xRows, ...lxlRows];

// Build the set of xlsx S/M fixtures that the rerun measured adequately (cold 1x + cold 4x).
// These fixtures are PROMOTED from "provisional" to the main aggregates; their original S/M
// timeout entries are dropped from combinedFails so the report includes them in the aggregate
// tables. The original ts-4451709.failures.jsonl is never modified — history stays intact.
const rerunCells = new Map();
for (const r of promotedRerunRows) {
  if (!rerunCells.has(r.fixture)) rerunCells.set(r.fixture, new Set());
  rerunCells.get(r.fixture).add(`${r.mode}|${r.cpuThrottle ?? 1}`);
}
const rerunMeasured = new Set([...rerunCells.entries()].filter(([, cs]) => cs.has('cold|1') && cs.has('cold|4')).map(([id]) => id));

// Combined failures: original S/M failures (but skip xlsx fixtures now promoted via rerun)
// + xlsx rerun failures (for fixtures the rerun still failed in some cell)
// + L/XL failures
const combinedFails = [
  ...smFails.filter((f) => !rerunMeasured.has(f.fixture ?? '')),
  ...rerunFails.filter((f) => xlsxSmIds.has(f.fixture ?? '')),
  ...lxlFails,
];

// Rerun JSONL: the non-xlsx rerun rows (docx/pptx S rerun) + their failures
const rerunDocxPptxRows = rerunRows.filter((r) => !xlsxSmIds.has(r.fixture));
const rerunDocxPptxFails = rerunFails.filter((f) => !xlsxSmIds.has(f.fixture ?? ''));

// Extract commit from lxl filename: ts-lxl-<sha>.jsonl -> sha
const lxlSha = /ts-lxl-([0-9a-f]+)\.jsonl$/.exec(basename(lxlFile))?.[1] ?? 'unknown';
const outBase = join(here, `baselines/ts-combined-${lxlSha}`);

writeFileSync(`${outBase}.jsonl`, combinedRows.map((r) => JSON.stringify(r)).join('\n') + '\n');
writeFileSync(`${outBase}.failures.jsonl`, combinedFails.map((r) => JSON.stringify(r)).join('\n') + '\n');
writeFileSync(`${outBase}.rerun.jsonl`, rerunDocxPptxRows.map((r) => JSON.stringify(r)).join('\n') + '\n');
writeFileSync(`${outBase}.rerun-failures.jsonl`, rerunDocxPptxFails.map((r) => JSON.stringify(r)).join('\n') + '\n');
// Warm 4x additions: separate file so rerunFlags can verify the combined layout
const warm4xOut = join(here, `baselines/ts-warm4x-${lxlSha}.jsonl`);
writeFileSync(warm4xOut, promotedWarm4xRows.map((r) => JSON.stringify(r)).join('\n') + (promotedWarm4xRows.length ? '\n' : ''));

const droppedSmFails = smFails.filter((f) => rerunMeasured.has(f.fixture ?? '')).length;
console.log(`assembled ts-combined-${lxlSha}:`);
console.log(`  main:    ${combinedRows.length} rows (${smRows.length} S/M + ${promotedRerunRows.length} promoted-rerun + ${promotedWarm4xRows.length} warm4x + ${lxlRows.length} L/XL)`);
console.log(`  fails:   ${combinedFails.length} (dropped ${droppedSmFails} original S/M timeout(s) for ${rerunMeasured.size} xlsx fixture(s) now promoted via rerun)`);
console.log(`  rerun:   ${rerunDocxPptxRows.length} rows (docx/pptx non-promoted)`);
console.log(`output: ${outBase}.jsonl${promotedWarm4xRows.length ? ` + ${warm4xOut.replace(join(here, '..') + '/', '')} (${promotedWarm4xRows.length} warm4x row(s))` : ''}`);
