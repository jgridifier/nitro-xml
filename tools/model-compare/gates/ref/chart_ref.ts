// SPDX-License-Identifier: MIT
const up = process.env.UPSTREAM_DIR!;
const { parseChartSpace } = await import(`${up}/src/core/chart/parse-space.ts`);
const { renderChartSpaceSvg } = await import(`${up}/src/core/chart/render/render-space.ts`);

const mode = process.argv[2];
const input = process.argv[3] ?? '';

if (mode === 'model') {
	console.log(JSON.stringify(parseChartSpace(input)));
	process.exit(0);
}

if (mode === 'svg') {
	const { chartSpace } = parseChartSpace(input);
	const svg = renderChartSpaceSvg(chartSpace, { width: 480, height: 320 });
	console.log(svg);
	process.exit(0);
}

console.error('usage: chart_ref.ts model|svg <xml>');
process.exit(2);
