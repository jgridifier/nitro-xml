// SPDX-License-Identifier: MIT
/**
 * Complete xlsx audit, including informational hosts and normally skipped print.
 * Build fresh WASM first: bash scripts/build-wasm.sh
 * Usage: bun parity/visual/run-xlsx-audit.mjs
 * Uses the normal gate's hosts, pixel comparison, and TS self-checks.
 */
import { writeFileSync } from 'node:fs';
import { VISUAL_CASES } from './registry.mjs';
import { runVisualGate } from './run.mjs';

if (process.env.VISUAL_XLSX_MAX || process.env.VISUAL_ONLY) {
	throw new Error('The complete xlsx audit requires VISUAL_XLSX_MAX and VISUAL_ONLY to be unset.');
}
const cases = VISUAL_CASES.filter((c) => c.ready &&
	(c.format === 'xlsx' || (c.kind === 'chart-svg' && /\.(xlsx|xlsm|xltx|xltm)$/i.test(c.fixture || ''))));
process.env.VISUAL_ONLY = cases.map((c) => c.id).join(',');
for (const c of cases) c.skipPrint = false;
const result = await runVisualGate();
let complete = true;
const table = cases.map((c) => {
	const rows = result.rows.filter((r) => r.case === c.id);
	const modes = [
		rows.find((r) => r.mode === 'ts-vs-rust' && r.dpr === 1),
		rows.find((r) => r.mode === 'ts-vs-rust' && r.dpr === 2),
		rows.find((r) => r.mode === 'ts-vs-rust-print-pdf'),
	];
	if (modes.some((r) => !r || r.reason)) complete = false;
	return `| ${c.id} | ${c.informational ? 'soft' : 'hard'} | ${modes.map((r) =>
		r ? (r.reason ? `ERROR: ${r.reason.replace(/\|/g, '/')}` : `${(r.ratio * 100).toFixed(4)}%`) : 'MISSING').join(' | ')} |`;
});
const xlsx = cases.filter((c) => c.format === 'xlsx');
const report = [
	'<!-- SPDX-License-Identifier: MIT -->',
	'# xlsx visual audit',
	'',
	`${xlsx.filter((c) => !c.informational).length} hard / ${xlsx.filter((c) => c.informational).length} soft xlsx cases; ${cases.length - xlsx.length} chart SVG cases.`,
	`Measurement complete: ${complete}. Hard gate passed: ${result.ok}.`,
	'',
	'| Case | Gate | DPR1 | DPR2 | Print |',
	'| --- | --- | --- | --- | --- |',
	...table,
	'',
].join('\n');
writeFileSync(new URL('./out/xlsx-audit.md', import.meta.url), report);
console.log(report);
process.exit(result.ok && complete ? 0 : 1);
