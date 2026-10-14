// SPDX-License-Identifier: MIT
/**
 * Stage-3 accessibility gate (#42): axe + AX-tree + keyboard, differential vs TS.
 */

import { writeFileSync, mkdirSync, appendFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { A11Y_CASES } from './registry.mjs';
import { runAxeChecks } from './axe.mjs';
import { runAxTreeChecks } from './ax-tree.mjs';
import { runKeyboardChecks } from './keyboard.mjs';
import { spawnSync } from 'node:child_process';

const OUT = join(dirname(fileURLToPath(import.meta.url)), 'out');
const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');

function perFormat(results) {
	/** @type {Record<string, {ok:boolean, lines:string[]}>} */
	const by = {};
	for (const r of results) {
		const f = r.format || 'unknown';
		if (!by[f]) by[f] = { ok: true, lines: [] };
		by[f].ok = by[f].ok && r.ok;
		by[f].lines.push(r.summary);
	}
	return by;
}

async function main() {
	mkdirSync(OUT, { recursive: true });
	const axe = await runAxeChecks();
	console.log(axe.summary);
	const ax = await runAxTreeChecks();
	console.log(ax.summary);
	const kb = await runKeyboardChecks();
	console.log(kb.summary);

	let editOk = true;
	let editSummary = 'a11y-edit: skipped';
	const editRunner = join(dirname(fileURLToPath(import.meta.url)), 'edit/run.mjs');
	const editRun = spawnSync('bun', [editRunner], {
		cwd: ROOT,
		encoding: 'utf8',
		maxBuffer: 16 * 1024 * 1024,
	});
	if (editRun.stdout) console.log(editRun.stdout.trimEnd());
	if (editRun.stderr) console.error(editRun.stderr.trimEnd());
	editOk = editRun.status === 0;
	try {
		const { readFileSync } = await import('node:fs');
		editSummary = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'edit/out/gate-summary.txt'), 'utf8').trim();
	} catch {
		editSummary = editOk ? 'a11y-edit: PASS' : 'a11y-edit: FAIL';
	}

	const deferred = A11Y_CASES.filter((c) => !c.ready).map((c) => `${c.id}: ${c.todo}`);
	const ok = axe.ok && ax.ok && kb.ok && editOk;

	const formats = ['xlsx', 'pptx', 'docx'];
	const formatLines = [];
	for (const f of formats) {
		const axeF = perFormat(axe.results)[f];
		const axF = perFormat(ax.results)[f];
		const kbF = perFormat(kb.results)[f];
		if (!axeF && !axF && !kbF) {
			const def = A11Y_CASES.find((c) => c.format === f && !c.ready);
			formatLines.push(`${f}: DEFERRED (${def?.todo || 'not ready'})`);
			continue;
		}
		const fOk = (axeF?.ok ?? true) && (axF?.ok ?? true) && (kbF?.ok ?? true);
		formatLines.push(
			`${f}: ${fOk ? 'PASS' : 'FAIL'} · axe ${axeF?.ok ? 'ok' : 'FAIL'} · ax-tree ${axF?.ok ? 'ok' : 'FAIL'} · keyboard ${kbF?.ok ? 'ok' : 'FAIL'}`,
		);
	}

	const summary = [
		`a11y: ${ok ? 'PASS' : 'FAIL'}`,
		`browsers: axe+keyboard on Chromium/WebKit/Firefox; ax-tree Chromium-only (CDP)`,
		...formatLines,
		axe.summary,
		ax.summary,
		kb.summary,
		editSummary,
		...(deferred.length ? ['deferred cases:', ...deferred.map((d) => `  - ${d}`)] : []),
	].join('\n');
	writeFileSync(join(OUT, 'gate-summary.txt'), summary + '\n');
	if (process.env.GITHUB_STEP_SUMMARY) {
		appendFileSync(
			process.env.GITHUB_STEP_SUMMARY,
			`## Accessibility gate (#42)\n\n\`\`\`\n${summary}\n\`\`\`\n`,
		);
	}
	console.log(ok ? 'a11y: PASS' : 'a11y: FAIL');
	process.exit(ok ? 0 : 1);
}

await main();
