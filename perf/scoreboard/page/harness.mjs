// SPDX-License-Identifier: MIT
// In-page side of the parse race. One fresh page per (file, entrant): load the entrant's scripts
// and bundle, fetch the fixture bytes into memory, then time parse() only.
import { canon } from '../lib/normalise.mjs';

let adapter = null;
let bytes = null;

function loadClassicScript(src) {
	return new Promise((resolve, reject) => {
		const s = document.createElement('script');
		s.src = src;
		s.onload = resolve;
		s.onerror = () => reject(new Error(`failed to load ${src}`));
		document.head.appendChild(s);
	});
}

export async function setup({ bundle, scripts = [], fixtureUrl }) {
	for (const src of scripts) await loadClassicScript(src);
	adapter = await import(bundle);
	if (adapter.init) await adapter.init();
	const res = await fetch(fixtureUrl);
	bytes = new Uint8Array(await res.arrayBuffer());
	return { bytes: bytes.byteLength, crossOriginIsolated: globalThis.crossOriginIsolated === true };
}

const heap = () => (performance.memory ? performance.memory.usedJSHeapSize : null);
const gc = () => globalThis.gc?.();

export async function run({ warmup, runs, format }) {
	const times = [];
	let model = null;
	let heapBefore = null;
	let heapAfter = null;
	let wasmBefore = null;
	let wasmAfter = null;
	for (let i = 0; i < warmup + runs; i++) {
		const input = bytes.slice();
		adapter.prepare?.(input);
		gc();
		const last = i === warmup + runs - 1;
		if (last) {
			heapBefore = heap();
			wasmBefore = adapter.wasmMemoryBytes?.() ?? null;
		}
		const t0 = performance.now();
		model = await adapter.parse(input);
		const t1 = performance.now();
		if (i >= warmup) times.push(t1 - t0);
		if (last) {
			heapAfter = heap();
			wasmAfter = adapter.wasmMemoryBytes?.() ?? null;
		} else {
			adapter.dispose?.(model);
			adapter.cleanup?.();
			model = null;
		}
	}
	const raw = adapter.extract(model);
	const output = canon[format](raw);
	adapter.dispose?.(model);
	adapter.cleanup?.();
	return { times, heapBefore, heapAfter, wasmBefore, wasmAfter, output };
}
