// SPDX-License-Identifier: MIT
// In-page side of the first-paint race (#41). Times load+init+first paint.

const stage = () => document.getElementById('stage');

function waitFrames(n = 2) {
	return new Promise((resolve) => {
		let left = n;
		const step = () => {
			left -= 1;
			if (left <= 0) resolve();
			else requestAnimationFrame(step);
		};
		requestAnimationFrame(step);
	});
}

function applyLightTheme(host) {
	if (!host?.style) return;
	const t = {
		'--xve-header-bg': '#f5f5f5',
		'--xve-header-fg': '#444444',
		'--xve-header-active-bg': '#d3e9dc',
		'--xve-selection': 'rgb(33 115 70 / 0.12)',
		'--xve-selection-border': '#217346',
		'--xve-sheet-bg': '#ffffff',
		'--xve-grid-line': '#e1e1e1',
	};
	for (const [k, v] of Object.entries(t)) host.style.setProperty(k, v);
}

function hideShellChrome(root) {
	if (!root) return;
	for (const sel of [
		'.chrome',
		'[part="sheet-tabs"]',
		'[part="status-bar"]',
		'.xve-tabs-row',
		'.xve-status',
		'.xve-title-bar',
		'.xve-ribbon',
		'office-ui-title-bar',
		'office-ui-status-bar',
	]) {
		root.querySelectorAll(sel).forEach((node) => {
			node.style.display = 'none';
		});
	}
	const v = root.querySelector('.v');
	if (v) {
		v.style.flex = '1 1 auto';
		v.style.minHeight = '0';
		v.style.height = '100%';
	}
}

function injectStyle(root, cssText, id) {
	if (!root) return;
	let el = root.querySelector(`style[data-fp="${id}"]`);
	if (!el) {
		el = document.createElement('style');
		el.dataset.fp = id;
		root.appendChild(el);
	}
	el.textContent = cssText;
}

async function paintNitroXlsx(fixtureUrl, bytes) {
	const { defineNitroXlsx, initCore } = await import('/packages/nitro-xml/loader/index.js');
	// Same font mirror as parity/visual xlsx host (pixel-comparable paint).
	const fontMirror = new URL('/fonts/css2', location.origin).href;
	await initCore({ fontSource: fontMirror });
	defineNitroXlsx();
	const el = document.createElement('nitro-xlsx');
	el.setAttribute('readonly', '');
	el.style.display = 'flex';
	el.style.flexDirection = 'column';
	el.style.width = '100%';
	el.style.height = '100%';
	applyLightTheme(el);
	stage().replaceChildren(el);
	const name = fixtureUrl.split('/').pop() || 'book.xlsx';
	const file = new File([bytes], name, {
		type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
	});
	await el.loadFile(file);
	const shadow = el.shadowRoot;
	hideShellChrome(shadow);
	const cv = shadow?.querySelector('canvas');
	if (cv) cv.style.visibility = 'hidden';
	// Visual-only grid CSS (not in shipped loader — bundle budget).
	try {
		const { GRID_CSS } = await import('/parity/visual/xlsx-grid-css.js');
		injectStyle(shadow, GRID_CSS, 'grid-css');
	} catch {
		/* optional */
	}
	try {
		const fontCss = await (await fetch('/fonts/visual-fonts.css')).text();
		injectStyle(
			shadow,
			fontCss +
				`\n:host,.xg-root,.xg-t,.xg-hd,.xg-tx{font-family:"Liberation Sans",Arial,sans-serif!important}`,
			'font-pin',
		);
	} catch {
		/* optional */
	}
	if (el.document) {
		const dpr = window.devicePixelRatio || 1;
		const w = el.clientWidth || 800;
		const h = el.clientHeight || 480;
		el.document.pushEvent({
			kind: 6,
			time_ms: performance.now(),
			width: w,
			height: h,
			dpr,
		});
	}
	const paint = async () => {
		el._tick?.();
		while (el._raf) await waitFrames(1);
	};
	for (let i = 0; i < 60; i++) {
		await waitFrames(1);
		if (el.document) await paint();
		const dom = shadow?.querySelector('.dom');
		if (dom && dom.childElementCount > 0) {
			hideShellChrome(shadow);
			return;
		}
	}
	throw new Error('nitro-xlsx: no grid paint');
}

async function paintUpstreamXlsx(fixtureUrl, bytes) {
	const { defineXlsxEditor } = await import('/out/bundles/xlsx-editor.js');
	defineXlsxEditor();
	const el = document.createElement('xlsx-editor');
	el.setAttribute('read-only', '');
	el.setAttribute('show-toolbar', 'false');
	el.setAttribute('show-formula-bar', 'false');
	el.setAttribute('file-name', fixtureUrl.split('/').pop() || 'book.xlsx');
	el.style.display = 'block';
	el.style.width = '100%';
	el.style.height = '100%';
	applyLightTheme(el);
	stage().replaceChildren(el);
	await waitFrames(1);
	await el.load(bytes, el.fileName);
	const root = el.shadowRoot || el;
	hideShellChrome(root);
	for (let i = 0; i < 60; i++) {
		await waitFrames(1);
		const grid = root.querySelector('.xg-root, .xve-grid, [part="grid"]');
		const cells = root.querySelector('.xg-cells, .dom');
		if ((grid || cells) && (cells?.childElementCount > 0 || grid?.childElementCount > 0)) {
			hideShellChrome(root);
			return;
		}
		// Upstream may paint into light-DOM descendants of the editor.
		if (root.querySelectorAll('.xg-c').length > 0) {
			hideShellChrome(root);
			return;
		}
	}
	throw new Error('upstream xlsx-editor: no grid paint');
}

function fitScale(canvasW, canvasH, viewW, viewH) {
	if (viewW <= 0 || viewH <= 0) return 1;
	return Math.min(viewW / Math.max(1, canvasW), viewH / Math.max(1, canvasH), 1);
}

async function waitFonts() {
	if (document.fonts?.ready) await document.fonts.ready;
	try {
		await Promise.all([
			...[...document.fonts].map((face) => face.load().catch(() => {})),
			document.fonts.load('18px "Liberation Sans"'),
			document.fonts.load('bold 18px "Liberation Sans"'),
			document.fonts.load('18px Calibri'),
			document.fonts.load('18px Carlito'),
			document.fonts.load('18px Arial'),
		]);
	} catch {
		/* optional */
	}
}

/** Same post–first-content settle for both pptx entrants (PR #390 equivalence). */
async function settleAfterFirstPptxContent() {
	await waitFonts();
	await waitFrames(3);
}

// PPTX first-paint adapters: timed window is paintOnce() only (fixture fetch in setup() is outside).
// End state: first visible slide painted in #stage. Parse/open scope follows each product API — see PR
// "Equivalence / product path (required)".

async function paintNitroPptx(fixtureUrl, bytes) {
	const { defineNitroPptx, initCore } = await import('/packages/nitro-xml/loader/index.js');
	const fontMirror = new URL('/fonts/css2', location.origin).href;
	await initCore({ fontSource: fontMirror });
	defineNitroPptx();
	const el = document.createElement('nitro-pptx');
	el.setAttribute('readonly', '');
	el.style.display = 'flex';
	el.style.flexDirection = 'column';
	el.style.width = '100%';
	el.style.height = '100%';
	stage().replaceChildren(el);
	const name = fixtureUrl.split('/').pop() || 'deck.pptx';
	const file = new File([bytes], name, {
		type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
	});
	await el.loadFile(file);
	const shadow = el.shadowRoot;
	hideShellChrome(shadow);
	try {
		const fontCss = await (await fetch('/fonts/visual-fonts.css')).text();
		injectStyle(
			shadow,
			fontCss +
				`\n:host,.dom{font-family:"Liberation Sans",Arial,sans-serif;-webkit-font-smoothing:antialiased;-moz-osx-font-smoothing:grayscale}` +
				`\n.dom [data-pptx-element]{font-family:"Liberation Sans",Arial,sans-serif}`,
			'font-pin',
		);
	} catch {
		/* optional */
	}
	if (el.document) {
		const dpr = window.devicePixelRatio || 1;
		const w = el.clientWidth || 800;
		const h = el.clientHeight || 480;
		el.document.pushEvent({
			kind: 6,
			time_ms: performance.now(),
			width: w,
			height: h,
			dpr,
		});
	}
	let painted = false;
	for (let i = 0; i < 90; i++) {
		await waitFrames(1);
		const msg = shadow?.querySelector('.msg');
		if (msg && !msg.hidden && msg.textContent) {
			throw new Error(`nitro-pptx: ${msg.textContent}`);
		}
		if (el.document) await el.document.tick();
		const dom = shadow?.querySelector('.dom');
		if (dom && dom.childElementCount > 0) {
			painted = true;
			break;
		}
	}
	if (!painted) throw new Error('nitro-pptx: no slide DOM paint');
	hideShellChrome(shadow);
	await settleAfterFirstPptxContent();
}

async function paintUpstreamPptx(fixtureUrl, bytes) {
	const {
		loadPresentation,
		renderSlideStage,
		createDefaultRegistry,
		createTranslator,
	} = await import('/out/bundles/pptx-slide.js');
	const viewW = 800;
	const viewH = 480;
	const loaded = await loadPresentation(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
	try {
		if (!loaded.slides?.length) throw new Error('upstream pptx: empty slides');
		const scale = fitScale(loaded.canvasSize.width, loaded.canvasSize.height, viewW, viewH);
		const slideEl = renderSlideStage({
			document,
			slide: loaded.slides[0],
			canvasSize: loaded.canvasSize,
			mediaDataUrls: loaded.mediaDataUrls,
			colorScheme: loaded.colorScheme,
			fontScheme: loaded.fontScheme,
			tableStyleMap: loaded.tableStyleMap,
			registry: createDefaultRegistry(),
			t: createTranslator('en'),
			scale,
			slides: loaded.slides,
			currentSlideIndex: 0,
			reading: true,
		});
		const wrap = document.createElement('div');
		wrap.id = 'ts-wrap';
		wrap.style.position = 'absolute';
		wrap.style.inset = '0';
		wrap.style.overflow = 'hidden';
		wrap.style.background = '#fff';
		wrap.appendChild(slideEl);
		stage().replaceChildren(wrap);
		try {
			const fontCss = await (await fetch('/fonts/visual-fonts.css')).text();
			injectStyle(
				document.head,
				fontCss +
					`\n#ts-wrap{font-family:"Liberation Sans",Arial,sans-serif;-webkit-font-smoothing:antialiased;-moz-osx-font-smoothing:grayscale}` +
					`\n#ts-wrap .pptxv-element{font-family:"Liberation Sans",Arial,sans-serif}`,
				'font-pin-ts',
			);
		} catch {
			/* optional */
		}
		let mounted = false;
		for (let i = 0; i < 90; i++) {
			await waitFrames(1);
			const slideRoot = wrap.firstElementChild;
			if (
				slideRoot &&
				(slideRoot.childElementCount > 0 ||
					wrap.querySelector('.pptxv-element, [class*="pptx"]'))
			) {
				mounted = true;
				break;
			}
		}
		if (!mounted) throw new Error('upstream pptx: no slide DOM paint');
		await settleAfterFirstPptxContent();
	} finally {
		loaded.handler?.dispose?.();
	}
}

const PAINTERS = {
	'nitro-xml': { xlsx: paintNitroXlsx, pptx: paintNitroPptx },
	'upstream-ts': { xlsx: paintUpstreamXlsx, pptx: paintUpstreamPptx },
};

let fixtureBytes = null;
let fixtureUrl = null;
let entrantId = null;
let format = null;

export async function setup({ entrant, format: fmt, fixturePath }) {
	entrantId = entrant;
	format = fmt;
	fixtureUrl = fixturePath;
	const res = await fetch(fixturePath);
	if (!res.ok) throw new Error(`fixture fetch ${res.status} ${fixturePath}`);
	fixtureBytes = new Uint8Array(await res.arrayBuffer());
	stage().replaceChildren();
	stage().dataset.ready = '0';
	return {
		bytes: fixtureBytes.byteLength,
		crossOriginIsolated: globalThis.crossOriginIsolated === true,
	};
}

/**
 * One cold open-to-first-paint on this page (includes module/wasm load).
 * The runner uses a fresh page per sample so worker-based viewers need not re-enter.
 */
export async function paintOnce() {
	const painter = PAINTERS[entrantId]?.[format];
	if (!painter) throw new Error(`no first-paint adapter for ${entrantId}/${format}`);
	const input = fixtureBytes.slice();
	globalThis.gc?.();
	const t0 = performance.now();
	await painter(fixtureUrl, input);
	const t1 = performance.now();
	stage().dataset.ready = '1';
	await waitFrames(2);
	return { ms: t1 - t0 };
}
