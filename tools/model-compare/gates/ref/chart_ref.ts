// SPDX-License-Identifier: MIT
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR!;
const { parseChartSpace } = await import(`${up}/src/core/chart/parse-space.ts`);
const { renderChartSpaceSvg } = await import(`${up}/src/core/chart/render/render-space.ts`);

function readInput(argv: string[]): string {
	if (argv[0] === '--file') {
		const path = argv[1];
		if (!path) throw new Error('missing path after --file');
		return readFileSync(path, 'utf8');
	}
	if (argv.length === 0) {
		return readFileSync(0, 'utf8');
	}
	return argv.join(' ');
}

const args = process.argv.slice(2);
const mode = args[0];
const input = readInput(args.slice(1));

if (mode === 'model') {
	console.log(JSON.stringify(parseChartSpace(input)));
	process.exit(0);
}

if (mode === 'model-json') {
	const payload = JSON.parse(input);
	const space = payload.chartSpace ?? payload;
	console.log(JSON.stringify({ chartSpace: space, issues: payload.issues ?? [] }));
	process.exit(0);
}

if (mode === 'svg' || mode === 'svg-json') {
	const chartSpace =
		mode === 'svg-json'
			? (JSON.parse(input).chartSpace ?? JSON.parse(input))
			: parseChartSpace(input).chartSpace;
	const svg = renderChartSpaceSvg(chartSpace, { width: 480, height: 320 });
	console.log(svg);
	process.exit(0);
}

console.error('usage: chart_ref.ts model|svg|model-json|svg-json [--file path | payload | stdin]');
process.exit(2);
