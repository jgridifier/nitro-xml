// SPDX-License-Identifier: MIT
// Init script injected into the page before any upstream code runs (Playwright addInitScript).
// It wraps the DOM and canvas-2D entry points, attributes every call to a docs/painters.md row
// through the JS call stack, and buckets the counts by animation frame. `__ROWS__` and
// `__DEFER__` are replaced by measure.mjs before injection.
(() => {
	const ROWS = __ROWS__;
	// Generic helpers (element factories, text measurers) whose calls belong to their caller's row.
	const DEFER = new Set(__DEFER__);
	Error.stackTraceLimit = 200;
	const FILE_RE = /\/(src\/(?:ui\/src|core)\/[^?:)\s]+\.ts)/;

	let recording = false;
	let frame = 0;
	let busy = false; // guards against counting calls made while we record a call
	// key `${frame}\u0000${row}\u0000${category}` -> count
	const counts = new Map();
	// row "other" diagnostics: innermost upstream file -> count
	const otherFiles = new Map();

	function attribute() {
		const stack = new Error().stack || '';
		let deferred;
		let firstUpstream;
		for (const line of stack.split('\n')) {
			const m = FILE_RE.exec(line);
			if (!m) continue;
			const file = m[1];
			firstUpstream ??= file;
			const row = ROWS[file];
			if (!row) continue; // not a classified file (layout engine, node_modules): keep walking
			if (DEFER.has(file)) {
				deferred ??= row;
				continue;
			}
			return row;
		}
		if (deferred) return deferred;
		if (firstUpstream) otherFiles.set(firstUpstream, (otherFiles.get(firstUpstream) || 0) + 1);
		return firstUpstream ? 'other' : 'harness';
	}

	function record(category) {
		if (!recording || busy) return;
		busy = true;
		try {
			const key = `${frame}\u0000${attribute()}\u0000${category}`;
			counts.set(key, (counts.get(key) || 0) + 1);
		} finally {
			busy = false;
		}
	}

	function wrapMethod(proto, name, category) {
		const desc = proto && Object.getOwnPropertyDescriptor(proto, name);
		if (!desc || typeof desc.value !== 'function') return;
		const original = desc.value;
		Object.defineProperty(proto, name, {
			...desc,
			value: function (...args) {
				record(category);
				return original.apply(this, args);
			},
		});
	}

	function wrapSetter(proto, name, category) {
		const desc = proto && Object.getOwnPropertyDescriptor(proto, name);
		if (!desc || typeof desc.set !== 'function' || !desc.configurable) return;
		const set = desc.set;
		Object.defineProperty(proto, name, {
			...desc,
			set(value) {
				record(category);
				set.call(this, value);
			},
		});
	}

	// Node creation and parsing.
	for (const name of ['createElement', 'createElementNS', 'createTextNode', 'importNode'])
		wrapMethod(Document.prototype, name, 'create');
	wrapMethod(Node.prototype, 'cloneNode', 'create');
	wrapMethod(DOMParser.prototype, 'parseFromString', 'create');
	// Tree changes.
	for (const name of ['appendChild', 'insertBefore', 'removeChild', 'replaceChild'])
		wrapMethod(Node.prototype, name, 'tree');
	for (const name of [
		'append',
		'prepend',
		'replaceChildren',
		'remove',
		'before',
		'after',
		'replaceWith',
		'insertAdjacentElement',
		'insertAdjacentHTML',
	])
		wrapMethod(Element.prototype, name, 'tree');
	wrapMethod(CharacterData.prototype, 'remove', 'tree');
	wrapMethod(DocumentFragment.prototype, 'append', 'tree');
	// Attributes, classes and reflected state.
	for (const name of ['setAttribute', 'setAttributeNS', 'removeAttribute', 'toggleAttribute'])
		wrapMethod(Element.prototype, name, 'attr');
	wrapSetter(Element.prototype, 'className', 'attr');
	wrapSetter(HTMLElement.prototype, 'hidden', 'attr');
	for (const name of ['add', 'remove', 'toggle', 'replace'])
		wrapMethod(DOMTokenList.prototype, name, 'attr');
	// Text and markup.
	wrapSetter(Node.prototype, 'textContent', 'text');
	wrapSetter(Element.prototype, 'innerHTML', 'text');
	wrapSetter(HTMLElement.prototype, 'innerText', 'text');
	// Inline style: setProperty/removeProperty, cssText, and every property setter.
	for (let proto = Object.getPrototypeOf(document.createElement('div').style); proto && proto !== Object.prototype; proto = Object.getPrototypeOf(proto)) {
		for (const name of Object.getOwnPropertyNames(proto)) {
			if (name === 'setProperty' || name === 'removeProperty') wrapMethod(proto, name, 'style');
			else wrapSetter(proto, name, 'style');
		}
	}
	// Canvas 2D: every method and every state setter (font, fillStyle, ...).
	for (const ctor of [globalThis.CanvasRenderingContext2D, globalThis.OffscreenCanvasRenderingContext2D]) {
		const proto = ctor && ctor.prototype;
		if (!proto) continue;
		for (const name of Object.getOwnPropertyNames(proto)) {
			if (name === 'constructor') continue;
			const desc = Object.getOwnPropertyDescriptor(proto, name);
			if (typeof desc.value === 'function') wrapMethod(proto, name, 'canvas');
			else if (desc.set) wrapSetter(proto, name, 'canvas');
		}
	}

	// One frame = everything between two of our animation-frame ticks (our callback is queued
	// first each tick, so the app's own rAF paint lands in the frame it hands to the compositor).
	const raf = window.requestAnimationFrame.bind(window);
	const tick = () => {
		frame++;
		raf(tick);
	};
	raf(tick);

	window.__painterCalls = {
		frame: () => frame,
		start() {
			counts.clear();
			otherFiles.clear();
			recording = true;
			return frame;
		},
		stop() {
			recording = false;
			const rows = [];
			for (const [key, n] of counts) {
				const [f, row, category] = key.split('\u0000');
				rows.push({ frame: Number(f), row, category, n });
			}
			const other = [...otherFiles].sort((a, b) => b[1] - a[1]).slice(0, 25);
			return { end: frame, rows, other };
		},
		/** Frames since `since` with no recorded call at all. */
		quietSince(since) {
			let last = since;
			for (const key of counts.keys()) last = Math.max(last, Number(key.split('\u0000')[0]));
			return frame - last;
		},
	};
})();
