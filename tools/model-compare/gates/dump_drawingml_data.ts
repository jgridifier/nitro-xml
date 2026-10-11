// SPDX-License-Identifier: MIT
// One-shot dump of upstream preset geometry + text-rect tables for the Rust port (#25).
import { writeFileSync } from 'node:fs';
import { PRESET_SHAPE_GEOMETRY_TABLE } from '../../../upstream/src/core/geometry/preset-shape-definitions-table.ts';
import { ACTION_BUTTON_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-action-buttons.ts';
import { ARROW_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-arrows.ts';
import { BRACE_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-braces.ts';
import { CALLOUT_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-callouts.ts';
import { CIRCULAR_ARROW_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-circular-arrows.ts';
import { FLOWCHART_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-flowchart.ts';
import { MISC_TEXT_RECTS_A } from '../../../upstream/src/core/geometry/preset-text-rect-misc-a.ts';
import { MISC_TEXT_RECTS_B } from '../../../upstream/src/core/geometry/preset-text-rect-misc-b.ts';
import { POLYGON_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-polygons.ts';
import { QUAD_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-quads.ts';
import { RIBBON_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-ribbons.ts';
import { STAR_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-stars.ts';
import { TAB_TEXT_RECTS } from '../../../upstream/src/core/geometry/preset-text-rect-tabs.ts';

const outDir = new URL('../../../rs/crates/nitro-xml-opc/src/drawingml/data/', import.meta.url);

const overrides = {
	...ACTION_BUTTON_TEXT_RECTS,
	...CALLOUT_TEXT_RECTS,
	...FLOWCHART_TEXT_RECTS,
	...STAR_TEXT_RECTS,
	...ARROW_TEXT_RECTS,
	...CIRCULAR_ARROW_TEXT_RECTS,
	...BRACE_TEXT_RECTS,
	...RIBBON_TEXT_RECTS,
	...TAB_TEXT_RECTS,
	...QUAD_TEXT_RECTS,
	...POLYGON_TEXT_RECTS,
	...MISC_TEXT_RECTS_A,
	...MISC_TEXT_RECTS_B,
};

writeFileSync(
	new URL('preset_shape_geometry.json', outDir),
	JSON.stringify(PRESET_SHAPE_GEOMETRY_TABLE),
);
writeFileSync(new URL('preset_text_rect.json', outDir), JSON.stringify(overrides));
console.log(
	JSON.stringify({
		presets: Object.keys(PRESET_SHAPE_GEOMETRY_TABLE).length,
		textRects: Object.keys(overrides).length,
	}),
);
