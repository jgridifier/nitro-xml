// SPDX-License-Identifier: MIT
/**
 * Stage-3 selection and caret parity gate (#43).
 */

import { writeFileSync, mkdirSync, appendFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { SELECTION_CASES, deferredCases } from './registry.mjs';
import { runShellChecks } from './shell.mjs';
import { runLiveChecks } from './live.mjs';

const OUT = join(dirname(fileURLToPath(import.meta.url)), 'out');

function formatBreakdown(rows) {
	const by = { xlsx: { ok: 0, n: 0 }, pptx: { ok: 0, n: 0 }, docx: { ok: 0, n: 0 } };
	for (const c of SELECTION_CASES) {
		if (!c.ready) continue;
		by[c.format].n++;
		const row = rows.find((r) => r.case === c.id);
		if (row?.ok) by[c.format].ok++;
	}
	return Object.entries(by)
		.map(([fmt, v]) => `${fmt} ${v.ok}/${v.n} ready`)
		.join(' · ');
}

async function main() {
	mkdirSync(OUT, { recursive: true });
	const shell = await runShellChecks();
	console.log(shell.summary);
	const live = await runLiveChecks();
	console.log(live.summary);

	const deferred = deferredCases().map(
		(c) => `${c.id}: allowList ${c.allowList || '—'} — ${c.todo || ''}`,
	);
	const rows = [...shell.rows, ...live.rows];
	const ok = shell.ok && live.ok;
	const breakdown = formatBreakdown(rows);
	const summary = [
		`selection: ${ok ? 'PASS' : 'FAIL'}`,
		`per-format (ready): ${breakdown}`,
		shell.summary,
		live.summary,
		...(deferred.length
			? ['deferred / allow-listed:', ...deferred.map((d) => `  - ${d}`)]
			: []),
	].join('\n');
	writeFileSync(join(OUT, 'gate-summary.txt'), summary + '\n');
	if (process.env.GITHUB_STEP_SUMMARY) {
		appendFileSync(
			process.env.GITHUB_STEP_SUMMARY,
			`## Selection gate (#43)\n\n\`\`\`\n${summary}\n\`\`\`\n`,
		);
	}
	console.log(ok ? 'selection: PASS' : 'selection: FAIL');
	process.exit(ok ? 0 : 1);
}

await main();
