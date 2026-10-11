// SPDX-License-Identifier: MIT
/**
 * TS oracle: pptx view IR = per-slide PptxElement trees (ported element kinds).
 * Volatile ids are stripped by normalize.ts.
 */

import { readFileSync } from 'node:fs';
import { upstreamDir } from '../paths.ts';

/** Element kinds the Rust port is expected to cover (extend as #24/#25 land). */
const PORTED_KINDS = new Set([
	'shape',
	'text',
	'image',
	'table',
	'chart',
	'group',
	'connector',
	'ole',
	'video',
	'audio',
	'smartArt',
	'model3d',
	'ink',
	'unknown',
]);

function pickElement(el: Record<string, unknown>): Record<string, unknown> {
	const type = String(el.type ?? 'unknown');
	if (!PORTED_KINDS.has(type) && type !== 'picture') {
		// Keep discriminant + geometry so missing kinds are still visible as gaps.
		return {
			type,
			name: el.name,
			hidden: el.hidden,
			x: el.x,
			y: el.y,
			width: el.width,
			height: el.height,
			rotation: el.rotation,
			_unported: true,
		};
	}
	// `picture` in some loaders aliases `image`.
	const out: Record<string, unknown> = { ...el };
	if (type === 'picture') out.type = 'image';
	delete out.rawXml;
	delete out.rawTiming;
	return out;
}

export async function dumpPptxViewIr(fixturePath: string): Promise<unknown> {
	const up = upstreamDir();
	const { PptxHandler } = await import(`${up}/src/core/pptx/core/PptxHandler.ts`);

	const bytes = readFileSync(fixturePath);
	const handler = new PptxHandler();
	const data = await handler.load(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));

	const slides = (data.slides ?? []).map((slide: any, index: number) => ({
		index,
		name: slide.name,
		hidden: !!slide.hidden,
		width: data.width,
		height: data.height,
		backgroundColor: slide.backgroundColor,
		elements: (slide.elements ?? []).map((el: Record<string, unknown>) => pickElement(el)),
	}));

	return {
		format: 'pptx',
		slideCount: slides.length,
		slides,
		warnings: data.warnings ?? [],
	};
}
