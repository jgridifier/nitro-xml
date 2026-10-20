// SPDX-License-Identifier: MIT
import {
UNSUPPORTED_BROWSER_MESSAGE,
isSupportedExtension,
UNSUPPORTED_FORMAT_MESSAGE,
} from "./formats.js";
import { setDefaultFontSource } from "./font-source.js";
const CACHE_NAME = "nitro-xml-wasm-v1";
let cachedModule = null;
let initPromise = null;
let wasmMemory = null;
let prewarmedWorker = null;
export function defaultGlueUrl() {
return new URL("../pkg/nitro_xml_wasm.js", import.meta.url);
}
export function defaultWasmUrl() {
return new URL("../pkg/nitro_xml_wasm_bg.wasm", import.meta.url);
}
export function wasmSupported() {
return typeof WebAssembly === "object" && typeof WebAssembly.instantiate === "function";
}
export async function compileWasmModule(source) {
if (source instanceof WebAssembly.Module) {
cachedModule = source;
return source;
}
if (cachedModule) return cachedModule;
let mod;
if (typeof Response !== "undefined" && source instanceof Response) {
if (typeof WebAssembly.compileStreaming === "function") {
mod = await WebAssembly.compileStreaming(source.clone());
} else {
mod = await WebAssembly.compile(await source.arrayBuffer());
}
await putModuleInCache(source.url || CACHE_NAME, source).catch(() => {});
} else if (typeof source === "string" || source instanceof URL) {
const url = String(source);
const fromCache = await getModuleFromCache(url).catch(() => null);
if (fromCache) {
cachedModule = fromCache;
return fromCache;
}
const res = await fetch(url);
if (!res.ok) throw new Error(`wasm fetch failed: ${res.status}`);
if (typeof WebAssembly.compileStreaming === "function") {
mod = await WebAssembly.compileStreaming(res.clone());
} else {
mod = await WebAssembly.compile(await res.arrayBuffer());
}
await putModuleInCache(url, res).catch(() => {});
} else {
mod = await WebAssembly.compile(source);
}
cachedModule = mod;
return mod;
}
async function getModuleFromCache(url) {
if (typeof caches === "undefined") return null;
const cache = await caches.open(CACHE_NAME);
const hit = await cache.match(url);
if (!hit) return null;
return WebAssembly.compile(await hit.arrayBuffer());
}
async function putModuleInCache(url, res) {
if (typeof caches === "undefined" || !url || !res || !res.clone) return;
await (await caches.open(CACHE_NAME)).put(url, res.clone());
}
export async function initCore(opts = {}) {
if (!wasmSupported()) {
throw unsupportedBrowserError();
}
if (opts.fontSource !== undefined) {
setDefaultFontSource(opts.fontSource);
}
if (initPromise) return initPromise;
initPromise = (async () => {
try {
const glueUrl = opts.glueUrl || defaultGlueUrl();
const wasmUrl = opts.wasmUrl || defaultWasmUrl();
const glue = await import( String(glueUrl));
let input = cachedModule;
if (!input) {
try {
const res = await fetch(String(wasmUrl));
if (!res.ok) throw new Error(`wasm fetch failed: ${res.status}`);
await compileWasmModule(res.clone()).catch(() => {});
input = cachedModule || res;
} catch {
input = String(wasmUrl);
}
}
const exports = await glue.default(input);
wasmMemory = exports.memory || null;
return { glue, exports, memory: wasmMemory };
} catch (e) {
initPromise = null;
throw unsupportedBrowserError(e);
}
})();
return initPromise;
}
export function getWasmMemory() {
return wasmMemory;
}
export async function loadFormatModule(formatExt) {
if (!isSupportedExtension(formatExt)) {
const err = new Error(UNSUPPORTED_FORMAT_MESSAGE);
err.code = "unsupported_format";
throw err;
}
await initCore();
return "core";
}
export async function loadEditorModule() {
const core = await initCore();
if (typeof core.exports.loadEditorChromeModule === "function") {
core.exports.loadEditorChromeModule();
}
return "editor-chrome";
}
export function prewarmWorker(workerUrl) {
if (typeof Worker === "undefined") return null;
if (prewarmedWorker) return prewarmedWorker;
try {
const url = workerUrl || new URL("./worker.js", import.meta.url);
prewarmedWorker = new Worker(url, { type: "module", name: "nitro-xml-prewarm" });
prewarmedWorker.postMessage({ type: "prewarm" });
return prewarmedWorker;
} catch {
return null;
}
}
export function takePrewarmedWorker() {
const w = prewarmedWorker;
prewarmedWorker = null;
return w;
}
export function unsupportedBrowserError(cause) {
const err = new Error(UNSUPPORTED_BROWSER_MESSAGE);
err.code = "unsupported_browser";
if (cause) err.cause = cause;
return err;
}
export function unsupportedFormatError() {
const err = new Error(UNSUPPORTED_FORMAT_MESSAGE);
err.code = "unsupported_format";
return err;
}
