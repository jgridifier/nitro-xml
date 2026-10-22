// SPDX-License-Identifier: MIT
/** Ambient types for the hand-written loader (JSDoc-backed; structural). */

export const ACCEPT: string;
export const SUPPORTED_EXTENSIONS: readonly string[];
export const UNSUPPORTED_BROWSER_MESSAGE: string;
export const UNSUPPORTED_FORMAT_MESSAGE: string;
export const PACKAGE_TOO_LARGE_MESSAGE: string;
export const VIEWER_RECOVERY_FAILED_MESSAGE: string;
export const EDITOR_RECOVERY_FAILED_MESSAGE: string;
export const EDITOR_RECOVERY_LAST_SAVE_MESSAGE: string;

export function extensionOf(name: string): string;
export function isSupportedExtension(name: string): boolean;
export function viewerOpenErrorMessage(err: unknown): string;

export function initCore(opts?: {
  fontSource?: string | false | ((families: string[]) => string | null);
  glueUrl?: string | URL;
  wasmUrl?: string | URL;
}): Promise<void>;
export function compileWasmModule(source: unknown): Promise<WebAssembly.Module>;
export function loadFormatModule(ext: string): Promise<unknown>;
export function loadEditorModule(): Promise<unknown>;
export function prewarmWorker(): void;
export function takePrewarmedWorker(): Worker | null;
export function wasmSupported(): boolean;
export function defaultGlueUrl(): URL;
export function defaultWasmUrl(): URL;
export function unsupportedBrowserError(): Error;
export function unsupportedFormatError(): Error;

export const Op: Record<string, number>;
export function applyCommandBuffer(...args: unknown[]): unknown;
export function cssColor(...args: unknown[]): string;

export const EventKind: Record<string, number>;
export const EVENT_VERSION: number;
export function encodeEventBatch(...args: unknown[]): unknown;
export function createEventCoalescer(...args: unknown[]): unknown;

export function flushMeasureBatch(...args: unknown[]): unknown;
export const LayoutReadKind: Record<string, number>;
export const LAYOUT_RESULT_LANES: number;
export function flushLayoutReadBatch(...args: unknown[]): unknown;
export function readLayoutReqsFromMemory(...args: unknown[]): unknown;
export function packLayoutReqs(...args: unknown[]): unknown;

export function openDocument(
  bytes: ArrayBuffer | Uint8Array,
  opts?: { name?: string; fontSource?: unknown },
): Promise<DocumentSession>;
export class DocumentSession {
  dispose(): void;
  tick(): Promise<unknown>;
  save(): Promise<Uint8Array>;
  applyEdit(spec: string): Promise<string>;
  editorKey(ev: unknown, mac?: boolean): Promise<string>;
}

export function createJournal(...args: unknown[]): unknown;
export class WorkerDocument {
  constructor(opts: { canvas?: HTMLCanvasElement | null; root?: Element | null; name?: string });
  onChrome: ((html: string) => void) | null;
  open(
    bytes: ArrayBuffer | Uint8Array,
    name?: string,
    opts?: { readonly?: boolean },
  ): Promise<void>;
  dispose(): void;
  tick(): Promise<unknown>;
  save(): Promise<Uint8Array>;
  applyEdit(spec: string): Promise<string>;
  editorKey(ev: unknown, mac?: boolean): Promise<string>;
}

export function defineNitroXml(): void;
export function defineTag(tag: string, ctor: CustomElementConstructor): void;
export class ViewerElement extends HTMLElement {
  loadFile(file: Blob, name?: string): Promise<void>;
  loadUrl(url: string): Promise<void>;
  loadBuffer(bytes: ArrayBuffer | Uint8Array, name?: string): Promise<void>;
  save(): Promise<Uint8Array>;
  readonly document: WorkerDocument | null;
  mode: string;
  readonly: boolean;
}
export class ShellViewerElement extends ViewerElement {}
export class NitroXmlElement extends ShellViewerElement {}

export function defineNitroXlsx(): void;
export class NitroXlsxElement extends ShellViewerElement {}
export function defineNitroPptx(): void;
export class NitroPptxElement extends ShellViewerElement {}
export function defineNitroDocx(): void;
export class NitroDocxElement extends ShellViewerElement {}

export function applyFontSourceToHandle(...args: unknown[]): unknown;
export function setDefaultFontSource(...args: unknown[]): void;
export function getDefaultFontSource(): unknown;
