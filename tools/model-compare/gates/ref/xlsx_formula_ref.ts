// SPDX-License-Identifier: MIT
// Reference side of the #71 formula parser differential.
// Reads a JSON array of formula strings and prints parseFormula dumps
// (AST, referenced ranges, reference spans, token join) for Sheet1.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR;
if (!up) {
	console.error('xlsx_formula_ref: UPSTREAM_DIR is not set');
	process.exit(2);
}

const { FormulaError } = await import(`${up}/src/core/xlsx/formula/ast.ts`);
const { parseTokens } = await import(`${up}/src/core/xlsx/formula/parser.ts`);
const { tokenize, joinTokens } = await import(`${up}/src/core/xlsx/formula/tokenizer.ts`);
const { referencedRanges, referenceSpans } = await import(`${up}/src/core/xlsx/formula/transform.ts`);

const SHEET = 'Sheet1';

function errorJson(err: unknown): { message: string; position: number; code?: string } | null {
	if (!(err instanceof FormulaError)) throw err;
	const out: { message: string; position: number; code?: string } = {
		message: err.message,
		position: err.position,
	};
	if (err.code) out.code = err.code;
	return out;
}

export function dumpFormula(formula: string): unknown {
	const out: Record<string, unknown> = {
		formula,
		join: null,
		parseError: null,
		ast: null,
		refs: [],
		spans: [],
	};
	try {
		const tokens = tokenize(formula);
		out.join = joinTokens(tokens);
		out.refs = referencedRanges(formula, SHEET);
		out.spans = referenceSpans(formula, SHEET);
		out.ast = parseTokens(tokens);
	} catch (err) {
		out.parseError = errorJson(err);
	}
	return out;
}

const file = process.argv[2];
if (!file) {
	console.error('usage: xlsx_formula_ref.ts <formulas.json>');
	process.exit(2);
}
const formulas = JSON.parse(readFileSync(file, 'utf8')) as string[];
process.stdout.write(JSON.stringify(formulas.map((formula) => dumpFormula(formula))));
process.stdout.write('\n');
