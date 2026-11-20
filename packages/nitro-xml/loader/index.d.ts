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

export function checkInputSize(actual: number): void;
export function extensionOf(name: string): string;
export function isSupportedExtension(name: string): boolean;
export function viewerOpenErrorMessage(err: unknown): string;

/** An explicit font entry: a font file/source URL with family metadata, or a stylesheet URL string. */
export type FontEntry =
  | { family: string; url: string; weight?: string; style?: string }
  | string;

export function initCore(opts?: {
  /** Where to load webfont stylesheets from.
   * - omitted / string: CSS2 mirror base URL (default: fonts.googleapis.com/css2)
   * - `false` / `"off"`: no remote fetch; substitutions recorded as diagnostics
   * - `"page"`: reuse families already in document.fonts; no Google request
   * - function: main-thread resolver `(families) => href | null` (openDocument only)
   */
  fontSource?: string | false | "page" | ((families: string[]) => string | null);
  /** Explicit font list loaded via FontFace on the main thread and in the worker.
   * No request to fonts.googleapis.com / fonts.gstatic.com is made when this is set. */
  fonts?: FontEntry[];
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
/** Size errors retain existing codes; limits are bytes, entry counts, or compression ratios.
 * NITRO_TIMEOUT uses milliseconds. */
export interface NitroError extends Error {
  code?: string;
  limit?: number;
  actual?: number;
  part?: string;
}
export class WorkerDocument {
  constructor(opts: {
    canvas?: HTMLCanvasElement | null;
    root?: Element | null;
    name?: string;
    /** Per-document font list; overrides the process default from initCore. */
    fonts?: FontEntry[];
    /** Per-document font source; overrides the process default from initCore. */
    fontSource?: string | false | "page";
  });
  onChrome: ((html: string) => void) | null;
  onError?: (error: NitroError) => void;
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
  /** Resolves after first editable paint + soft-share unlock (no-op when read-only). */
  whenEditReady(): Promise<void>;
}

/** Selection snapshot returned by `getSelection()` on each element type. */
/** v0.2.0: the caret only; `text` is "" and `start` equals `end` (text ranges: #561). */
export type NitroDocxSelection = {
  format: "docx";
  text: string;
  page: number;
  start: { paragraph: number; run: number; offset: number } | null;
  end: { paragraph: number; run: number; offset: number } | null;
  collapsed: boolean;
};
export type NitroXlsxSelection = {
  format: "xlsx";
  sheet: string;
  sheetIndex: number;
  range: string | null;
  anchor: string | null;
  values: (string | number | boolean | null)[][];
  truncated: boolean;
};
/** v0.2.0: the clicked shape only; `text` is its whole text, `textRange` is null (text ranges: #561). */
export type NitroPptxSelection = {
  format: "pptx";
  slide: number;
  shapeId: string | null;
  shapeName: string | null;
  text: string | null;
  textRange: { start: number; end: number } | null;
  collapsed: true;
};
export type NitroSelection = NitroDocxSelection | NitroXlsxSelection | NitroPptxSelection;

export function defineNitroXml(): void;
export function defineTag(tag: string, ctor: CustomElementConstructor): void;
/** Emits bubbling, composed `error` CustomEvents with NitroError in detail.
 * Failed load/save calls also reject with that error. */
export class ViewerElement extends HTMLElement {
  /** Open a file, optionally at position. Does not steal focus unless `{ focus: true }` or `autofocus`. */
  loadFile(file: Blob, name?: string, opts?: NitroOpenOptions): Promise<void>;
  /** Fetch and open a URL, optionally at position. Does not steal focus unless `{ focus: true }` or `autofocus`. */
  loadUrl(url: string, opts?: NitroOpenOptions): Promise<void>;
  /** Open bytes, optionally at position before the requested frame.
   *  Pass `{ focus: true }` to focus the editor after load, or set the `autofocus` attribute. */
  loadBuffer(bytes: ArrayBuffer | Uint8Array, name?: string, opts?: NitroOpenOptions): Promise<void>;
  save(): Promise<Uint8Array>;
  readonly document: WorkerDocument | null;
  mode: string;
  readonly: boolean;
}
/** Chrome theme: a built-in name, or `--nx-*` tokens (`"--nx-accent"` or short `accent`). See docs/theme.md. */
export type NitroTheme = "office" | "quiet" | "dark" | Record<string, string>;
/** Position indices are zero-based; zoom is percent and scroll offsets are CSS pixels. */
export interface NitroPosition {
  page?: number;
  sheet?: number;
  sheetName?: string;
  slide?: number;
  scrollTop: number;
  scrollLeft: number;
  zoom?: number;
}
export type NitroPositionInput = Omit<Partial<NitroPosition>, "sheet"> & { sheet?: number | string };
export interface NitroOpenOptions {
  /** Applied before the requested frame; does not focus or emit nitro-change. */
  position?: NitroPositionInput;
  /** Opt-in: focus the editor after load. Same as the `autofocus` attribute. */
  focus?: boolean;
}
export class ShellViewerElement extends ViewerElement {
  /** Sets `data-theme` and the `--nx-*` variables on the element. A string attribute may also hold JSON tokens. */
  theme: NitroTheme | null;
  /**
   * True when the loaded document has unsaved edits; false in read-only mode.
   * Flips emit a bubbling, composed `nitro-change` CustomEvent `{ detail: { dirty: boolean } }`.
   * Resets to false after `save()` succeeds or a new document is loaded.
   */
  readonly dirty: boolean;
  /**
   * Selection snapshot (#561). v0.2.0: DOCX returns the caret only and PPTX the clicked
   * shape only, both `collapsed: true` with no selected text range (text ranges: #561).
   * XLSX: stored cell values of the range, at most 10,000 cells (`truncated` past it).
   * Call after load completes. Never marks the document dirty; no nitro-selectionchange event.
   */
  getSelection(): Promise<NitroSelection | null>;
  /** Last painted position, synchronously. Before open: zero scroll offsets. */
  getPosition(): NitroPosition;
  /** Navigate without focus/change events. Bounds clamp; unknown sheet names reject. */
  setPosition(position: NitroPositionInput): Promise<void>;
  /** Replace bytes in the same worker/element/shadowRoot. Keeps position by default;
   * keeps mode, resets dirty silently and the edit journal, and emits no nitro-change or focus change. */
  reload(bytes: ArrayBuffer | Uint8Array, opts?: { keepPosition?: boolean; name?: string }): Promise<void>;
  /** Live mode switch without reparsing. `readonly="false"` enables editing;
   * absent/empty/other attribute values mean readonly. Emits nitro-mode after completion,
   * detail {readonly:boolean}; no nitro-change. save() rejects while readonly. */
  readonly: boolean;
  /** Explicit font list; overrides the process default set via initCore({ fonts }).
   * No request to fonts.googleapis.com / fonts.gstatic.com is made when set.
   * Set before calling loadBuffer / loadFile / loadUrl. */
  fonts: FontEntry[] | null;
}
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
