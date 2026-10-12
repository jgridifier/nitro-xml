// SPDX-License-Identifier: MIT
// Reference side of the #72 calc-graph differential.
// prepare + formulaOrder on nodes sorted by (sheet, row, col), then recalculateAll.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR;
if (!up) {
	console.error('xlsx_calc_ref: UPSTREAM_DIR is not set');
	process.exit(2);
}

const { FormulaError } = await import(`${up}/src/core/xlsx/formula/ast.ts`);
const { createCalcEngine } = await import(`${up}/src/core/xlsx/formula/engine.ts`);
const { formulaOrder } = await import(`${up}/src/core/xlsx/formula/engine-order.ts`);
const { ColumnIndex, AreaIndex } = await import(`${up}/src/core/xlsx/formula/graph-index.ts`);
const { findDefinedName } = await import(`${up}/src/core/xlsx/formula/evaluator.ts`);
const { prefixSheets } = await import(`${up}/src/core/xlsx/formula/references.ts`);
const { loadXlsx } = await import(`${up}/src/core/xlsx/read/load.ts`);

const files = process.argv.slice(2);
const results = [];

for (const file of files) {
	try {
		const bytes = readFileSync(file);
		const workbook = await loadXlsx(bytes);
		const engine = createCalcEngine(workbook);
		engine.prepare();
		const nodes = [];
		for (const map of engine.nodes.values()) {
			for (const node of map.values()) nodes.push(node);
		}
		nodes.sort((a, b) => a.sheet - b.sheet || a.row - b.row || a.col - b.col);
		const columns = new ColumnIndex(nodes);
		const footprints = new AreaIndex();
		for (const node of nodes) {
			const fp = node.spill ?? node.blockedSpill ?? node.arrayRange;
			if (fp) footprints.add({ sheet: node.sheet, range: fp }, node);
		}
		const dirty = new Set(nodes);
		const succCache = new Map();
		const successors = (node) => {
			let out = succCache.get(node);
			if (out) return out;
			const found = [];
			const anchors = new Set();
			for (const dep of node.deps) {
				columns.nodesIn(dep, found);
				footprints.dependents(dep.sheet, dep.range, anchors);
			}
			for (const anchor of anchors) {
				if (anchor.spill ?? anchor.arrayRange) found.push(anchor);
			}
			out = found.length ? [...new Set(found)].filter((n) => dirty.has(n)) : [];
			succCache.set(node, out);
			return out;
		};
		const order = formulaOrder(nodes, successors);
		const ranked = new Map();
		order.components.forEach((component, rank) => {
			for (const member of component.nodes) {
				ranked.set(member, { rank, cyclic: component.cyclic });
			}
		});
		const planned = nodes.map((node) => {
			const info = ranked.get(node) ?? { rank: 0, cyclic: false };
			return {
				sheet: node.sheet,
				row: node.row,
				col: node.col,
				formula: node.formula,
				rank: info.rank,
				cyclic: info.cyclic,
				volatile: !!node.volatile,
				unsupported: !!node.unsupported,
				keepCached: !!node.keepCached,
				hasCall: formulaHasCall(node.ast, engine, node.sheet),
				deps: (node.deps ?? []).map(areaJson),
				dynamicDeps: (node.dynamicDeps ?? []).map(areaJson),
			};
		});
		engine.recalculateAll();
		const circular = new Set(
			engine.circularCells().map((cell) => `${cell.sheet}:${cell.row}:${cell.col}`),
		);
		const cells = planned.map((cell) => {
			const value =
				workbook.sheets[cell.sheet]?.rows.get(cell.row)?.get(cell.col)?.value ?? null;
			return {
				...cell,
				circular: circular.has(`${cell.sheet}:${cell.row}:${cell.col}`),
				value: value === undefined ? null : value,
			};
		});
		results.push({ file, cells });
	} catch (err) {
		const message = err instanceof Error ? err.message : String(err);
		results.push({ file, error: message });
	}
}

console.log(JSON.stringify({ results }));

function areaJson(area) {
	return {
		sheet: area.sheet,
		range: {
			start: { row: area.range.start.row, col: area.range.start.col },
			end: { row: area.range.end.row, col: area.range.end.col },
		},
	};
}

function formulaHasCall(ast, host, sheet) {
	const ctx = { host, sheet, depth: 0, found: false };
	visitCall(ast, ctx);
	return ctx.found;
}

function visitCall(node, ctx) {
	if (!node || ctx.found) return;
	switch (node.type) {
		case 'call':
		case 'invoke':
			ctx.found = true;
			return;
		case 'name': {
			let sheet = ctx.sheet;
			if (node.prefix) {
				const sheets = prefixSheets(ctx.host, node.prefix, ctx.sheet);
				if (!sheets || sheets[0] === undefined) return;
				sheet = sheets[0];
			}
			const defined = findDefinedName(ctx.host, node.name, sheet, node.prefix !== undefined);
			if (!defined) return;
			if (ctx.depth > 16) return;
			const parsed = ctx.host.parse(defined.formula);
			if (parsed instanceof FormulaError) return;
			ctx.depth++;
			visitCall(parsed, ctx);
			ctx.depth--;
			return;
		}
		case 'unary':
		case 'percent':
			visitCall(node.operand, ctx);
			return;
		case 'binary':
			visitCall(node.left, ctx);
			visitCall(node.right, ctx);
			return;
		default:
			return;
	}
}
