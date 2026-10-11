// SPDX-License-Identifier: MIT
// Reference side of the #25 gate: upstream DrawingML colour, fill, theme and preset geometry.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR!;
const {
	parseDrawingColor,
	resolveDrawingColor,
	parseDrawingFill,
	parseDrawingLine,
	parseThemeXml,
	themeDrawingColorTheme,
	THEME_COLOR_SLOTS,
} = await import(`${up}/src/core/drawingml/index.ts`);
const { evaluatePresetShape, lookupPresetShape } = await import(
	`${up}/src/core/geometry/preset-shape-evaluator.ts`,
);
const { parseXml } = await import(`${up}/src/core/xml/index.ts`);

const mode = process.argv[2];

function fail(obj: Record<string, unknown>) {
	console.log(JSON.stringify(obj));
	process.exit(2);
}

if (mode === 'color') {
	const input = JSON.parse(process.argv[3] ?? '');
	const themeMap = input.theme ?? {};
	const theme = {
		scheme(name: string) {
			const v = themeMap[name];
			return v ? (v.startsWith('#') ? v : `#${v}`) : undefined;
		},
	};
	const resolved = resolveDrawingColor(input.color, theme, {
		transformOrder: input.transform_order === 'document' ? 'document' : 'legacy',
		hslRounding: input.hsl_rounding === 'halfDown' ? 'halfDown' : 'nearest',
	});
	if (!resolved) fail({ error: 'unresolved' });
	console.log(JSON.stringify(resolved));
	process.exit(0);
}

if (mode === 'preset') {
	const name = process.argv[3] ?? '';
	const w = Number(process.argv[4]);
	const h = Number(process.argv[5]);
	const adj = process.argv[6] ? JSON.parse(process.argv[6]) : undefined;
	const result = evaluatePresetShape(name, w, h, adj);
	if (!result) fail({ error: 'unknown_preset' });
	console.log(
		JSON.stringify({
			svgPath: result.svgPath,
			textRect: result.textRect,
			paths: result.paths,
			fillNone: result.fillNone,
		}),
	);
	process.exit(0);
}

if (mode === 'fill') {
	const file = process.argv[3]!;
	const xml = readFileSync(file, 'utf8');
	const doc = parseXml(xml, { label: 'fill snippet' });
	const fill = parseDrawingFill(doc.documentElement);
	const line = parseDrawingLine(doc.documentElement);
	console.log(JSON.stringify({ fill, line }));
	process.exit(0);
}

if (mode === 'theme-colors') {
	const file = process.argv[3]!;
	const xml = readFileSync(file, 'utf8');
	const theme = parseThemeXml(xml);
	const colorTheme = themeDrawingColorTheme(theme.colorScheme);
	const slots: Record<string, unknown> = {};
	for (const slot of THEME_COLOR_SLOTS) {
		const color = theme.colorScheme.colors[slot];
		if (!color) continue;
		slots[slot] = {
			color,
			legacy: resolveDrawingColor(color, colorTheme, { transformOrder: 'legacy' }),
			document: resolveDrawingColor(color, colorTheme, { transformOrder: 'document' }),
			base: resolveDrawingColor(color, colorTheme),
		};
	}
	console.log(JSON.stringify(slots));
	process.exit(0);
}

if (mode === 'list-presets') {
	const names = Object.keys(
		(await import(`${up}/src/core/geometry/preset-shape-definitions-table.ts`))
			.PRESET_SHAPE_GEOMETRY_TABLE,
	).sort();
	console.log(JSON.stringify(names));
	process.exit(0);
}

fail({ error: 'usage' });
