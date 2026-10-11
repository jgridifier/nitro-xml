// SPDX-License-Identifier: MIT
// Reference side of the #24 gate: upstream PptxHandler load → issue-24 dump JSON.
import { readFileSync } from 'node:fs';

const up = process.env.UPSTREAM_DIR!;
const { PptxHandler } = await import(`${up}/src/core/pptx/core/PptxHandler.ts`);

/** Kinds this issue ports; dump filters to these for the ≥60 fixture parity bar. */
const PORTED = new Set([
	'shape',
	'text',
	'picture',
	'image',
	'group',
	'connector',
	'table',
]);

type AnyEl = Record<string, unknown> & {
	type?: string;
	children?: AnyEl[];
	table?: { rows?: unknown[] };
	text?: string;
};

function slimInherited(t: unknown): unknown {
	if (!t || typeof t !== 'object') return undefined;
	const o = t as Record<string, unknown>;
	const out: Record<string, unknown> = {};
	for (const k of ['x', 'y', 'width', 'height', 'rotation', 'flipHorizontal', 'flipVertical']) {
		if (o[k] !== undefined) out[k] = o[k];
	}
	return Object.keys(out).length ? out : undefined;
}

function slimEl(el: AnyEl): Record<string, unknown> {
	const out: Record<string, unknown> = {
		type: el.type,
		id: el.id,
	};
	if (el.shapeId !== undefined) out.shapeId = el.shapeId;
	if (el.name !== undefined) out.name = el.name;
	if (el.placeholderType !== undefined) out.placeholderType = el.placeholderType;
	if (el.placeholderSz !== undefined) out.placeholderSz = el.placeholderSz;
	out.x = el.x;
	out.y = el.y;
	out.width = el.width;
	out.height = el.height;
	if (el.xEmu !== undefined) out.xEmu = el.xEmu;
	if (el.yEmu !== undefined) out.yEmu = el.yEmu;
	if (el.widthEmu !== undefined) out.widthEmu = el.widthEmu;
	if (el.heightEmu !== undefined) out.heightEmu = el.heightEmu;
	if (el.rotation !== undefined) out.rotation = el.rotation;
	if (el.flipHorizontal) out.flipHorizontal = el.flipHorizontal;
	if (el.flipVertical) out.flipVertical = el.flipVertical;
	const inh = slimInherited(el.inheritedTransform);
	if (inh) out.inheritedTransform = inh;
	if (typeof el.text === 'string') out.text = el.text;
	if (el.shapeType !== undefined) out.shapeType = el.shapeType;
	if (el.type === 'table' && el.table && Array.isArray(el.table.rows)) {
		out.tableRowCount = el.table.rows.length;
	}
	if (Array.isArray(el.children)) {
		out.children = el.children.filter((c) => PORTED.has(String(c.type))).map(slimEl);
	}
	return out;
}

function masterPathForSlide(
	slidePath: string,
	layoutPath: string | undefined,
	masters: Array<{ path?: string; layouts?: Array<{ path?: string }> }>,
): string | undefined {
	if (!layoutPath) return undefined;
	for (const m of masters) {
		if ((m.layouts ?? []).some((l) => l.path === layoutPath)) return m.path;
	}
	// Fallback: walk is done by Rust via rels; TS dump leaves undefined when not on master.layouts
	void slidePath;
	return undefined;
}

const file = process.argv[2]!;
try {
	const bytes = new Uint8Array(readFileSync(file));
	const handler = new PptxHandler();
	// PptxHandler writes compatibility notices to console.log; silence during dump.
	const quiet = (..._args: unknown[]) => {};
	const orig = {
		log: console.log,
		info: console.info,
		warn: console.warn,
		debug: console.debug,
	};
	console.log = quiet;
	console.info = quiet;
	console.warn = quiet;
	console.debug = quiet;
	let data: Awaited<ReturnType<PptxHandler['load']>>;
	try {
		data = await handler.load(bytes.buffer as ArrayBuffer, { eagerDecodeImages: false });
	} finally {
		console.log = orig.log;
		console.info = orig.info;
		console.warn = orig.warn;
		console.debug = orig.debug;
	}
	const masters = (data.slideMasters ?? []).map((m) => ({
		path: m.path,
		name: m.name,
		layouts: (m.layouts ?? []).map((l) => ({
			path: l.path,
			name: l.name,
			...(l.type !== undefined ? { type: l.type } : {}),
		})),
		placeholders: (m.placeholders ?? []).map((p) => ({
			type: p.type,
			...(p.idx !== undefined ? { idx: p.idx } : {}),
			...(p.x !== undefined ? { x: p.x } : {}),
			...(p.y !== undefined ? { y: p.y } : {}),
			...(p.width !== undefined ? { width: p.width } : {}),
			...(p.height !== undefined ? { height: p.height } : {}),
		})),
	}));

	const theme = data.theme
		? {
				...(data.theme.colorScheme
					? { colorScheme: data.theme.colorScheme }
					: {}),
				...(data.theme.fontScheme
					? {
							fontScheme: {
								...(data.theme.fontScheme.majorFont
									? {
											majorFont: {
												...(data.theme.fontScheme.majorFont.latin !== undefined
													? { latin: data.theme.fontScheme.majorFont.latin }
													: {}),
												...(data.theme.fontScheme.majorFont.eastAsia !== undefined
													? { eastAsia: data.theme.fontScheme.majorFont.eastAsia }
													: {}),
												...(data.theme.fontScheme.majorFont.complexScript !== undefined
													? {
															complexScript:
																data.theme.fontScheme.majorFont.complexScript,
														}
													: {}),
											},
										}
									: {}),
								...(data.theme.fontScheme.minorFont
									? {
											minorFont: {
												...(data.theme.fontScheme.minorFont.latin !== undefined
													? { latin: data.theme.fontScheme.minorFont.latin }
													: {}),
												...(data.theme.fontScheme.minorFont.eastAsia !== undefined
													? { eastAsia: data.theme.fontScheme.minorFont.eastAsia }
													: {}),
												...(data.theme.fontScheme.minorFont.complexScript !== undefined
													? {
															complexScript:
																data.theme.fontScheme.minorFont.complexScript,
														}
													: {}),
											},
										}
									: {}),
							},
						}
					: {}),
			}
		: undefined;

	const out = {
		width: data.width,
		height: data.height,
		widthEmu: data.widthEmu ?? 0,
		heightEmu: data.heightEmu ?? 0,
		notesWidthEmu: data.notesWidthEmu ?? 0,
		notesHeightEmu: data.notesHeightEmu ?? 0,
		...(theme && Object.keys(theme).length ? { theme } : {}),
		slideMasters: masters,
		slides: data.slides.map((s) => {
			const layoutPath = s.layoutPath;
			const masterPath = masterPathForSlide(s.id, layoutPath, masters);
			const elements = (s.elements ?? [])
				.filter((e) => PORTED.has(String(e.type)))
				.map((e) => slimEl(e as AnyEl));
			const otherKinds = (s.elements ?? [])
				.map((e) => e.type)
				.filter((t) => !PORTED.has(String(t)));
			return {
				id: s.id,
				...(layoutPath !== undefined ? { layoutPath } : {}),
				...(masterPath !== undefined ? { masterPath } : {}),
				hasNotes: Boolean(
					(typeof s.notes === 'string' && s.notes.length > 0) ||
						(s.notesShapes && s.notesShapes.length > 0),
				),
				...(s.showMasterShapes !== undefined ? { showMasterShapes: s.showMasterShapes } : {}),
				...(otherKinds.length ? { otherKinds } : {}),
				elements,
			};
		}),
	};
	console.log(JSON.stringify(out));
} catch (e) {
	const msg = e instanceof Error ? e.message : String(e);
	console.log(JSON.stringify({ error: 'load_failed', detail: msg.slice(0, 200) }));
	process.exit(1);
}
