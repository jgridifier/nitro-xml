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

/**
 * All built-in command ids registered in CHROME_COMMANDS (commands.rs) and the office editors.
 * Guard-tested against the Rust registry in chrome/commands.rs#guard_js_command_ids.
 * Pass any `string` to `execCommand` / `ribbon.hide` for forward-compat with ids added in later releases.
 */
export type NitroCommandId =
  | "home.paste" | "home.paste-values" | "home.paste-formulas" | "home.paste-formatting"
  | "home.paste-transpose" | "home.paste-special" | "home.cut" | "home.copy"
  | "home.format-painter" | "home.font-name" | "home.font-size" | "home.grow-font"
  | "home.shrink-font" | "home.bold" | "home.italic" | "home.underline"
  | "home.underline-double" | "home.strikethrough" | "home.borders" | "home.borders-more"
  | "home.fill-color" | "home.font-color" | "home.font-settings"
  | "home.align-top" | "home.align-middle" | "home.align-bottom" | "home.orientation"
  | "home.alignment-settings" | "home.wrap-text" | "home.align-left" | "home.align-center"
  | "home.align-right" | "home.indent-decrease" | "home.indent-increase"
  | "home.merge-center" | "home.merge-across" | "home.merge-cells" | "home.unmerge"
  | "home.number-format" | "home.number-settings" | "home.accounting" | "home.accounting-more"
  | "home.percent" | "home.comma" | "home.decimal-increase" | "home.decimal-decrease"
  | "home.cf-quick" | "home.cf-data-bar" | "home.cf-color-scale" | "home.cf-icon-set"
  | "home.cf-new-rule" | "home.cf-clear-selection" | "home.cf-clear-sheet" | "home.cf-manage"
  | "home.format-as-table" | "home.cell-styles"
  | "cells.insert-split" | "cells.delete-split" | "cells.insert" | "cells.delete"
  | "cells.insert-rows" | "cells.delete-rows" | "cells.insert-columns" | "cells.delete-columns"
  | "sheet.insert" | "sheet.delete"
  | "format.row-height" | "format.autofit-rows" | "format.column-width"
  | "format.autofit-columns" | "format.default-width" | "format.hide-rows"
  | "format.hide-columns" | "sheet.hide" | "format.unhide-rows" | "format.unhide-columns"
  | "sheet.unhide" | "sheet.rename" | "sheet.move-copy" | "sheet.tab-color"
  | "review.protect-sheet" | "format.lock-cell" | "format.cells"
  | "home.autosum" | "home.autosum-average" | "home.autosum-count" | "home.autosum-max"
  | "home.autosum-min" | "home.autosum-more" | "home.fill-down" | "home.fill-right"
  | "home.fill-up" | "home.fill-left" | "home.fill-series" | "home.clear-all"
  | "home.clear-formats" | "home.clear-contents" | "home.clear-comments" | "home.clear-hyperlinks"
  | "data.sort-asc" | "data.sort-desc" | "data.sort-custom" | "data.filter"
  | "data.filter-clear" | "data.filter-reapply"
  | "home.find" | "home.replace" | "home.go-to" | "home.go-to-special"
  | "home.select-formulas" | "home.select-comments" | "home.select-conditional"
  | "home.select-constants" | "home.select-validation"
  | "file.save" | "file.open" | "file.new" | "file.print" | "edit.undo" | "edit.redo"
  | "home.format-general" | "home.format-currency" | "home.format-percent"
  | "home.format-scientific" | "home.format-date" | "home.format-time" | "home.format-number"
  | "edit.insert-date" | "edit.insert-time" | "formulas.show-formulas"
  | "edit.select-column" | "edit.select-row" | "sheet.previous" | "sheet.next"
  | "insert.table" | "insert.link" | "review.new-comment" | "formulas.calculate-now"
  | "home.shape-fill" | "home.shape-outline" | "home.shape-dash"
  | "home.duplicate" | "home.delete" | "home.group" | "home.ungroup"
  | "home.bring-forward" | "home.send-backward" | "home.bring-to-front" | "home.send-to-back"
  | "home.subscript" | "home.superscript" | "home.highlight"
  | "home.double-underline" | "home.double-strike" | "home.caps" | "home.small-caps"
  | "home.hidden" | "home.expand-spacing" | "home.condense-spacing"
  | "home.scale-up" | "home.scale-down"
  | "home.bullets" | "home.numbering" | "home.outdent" | "home.indent" | "home.align-justify"
  | "home.line-spacing" | "home.space-before" | "home.space-after"
  | "home.hanging-indent" | "home.first-line" | "home.indent-right" | "home.outdent-right"
  | "home.shading" | "home.border-bottom" | "home.border-top" | "home.border-left"
  | "home.border-right" | "home.border-double" | "home.border-dash" | "home.border-dot"
  | "home.style-normal" | "home.style-title" | "home.style-heading-1"
  | "home.style-heading-2" | "home.style-heading-3"
  | "insert.slide" | "insert.duplicate-slide" | "home.layout" | "home.delete-slide"
  | "insert.move-slide-up" | "insert.move-slide-down"
  | "table.insert-row" | "table.delete-row" | "table.insert-column" | "table.delete-column"
  | "table.cell-text" | "table.delete"
  | "insert.picture" | "insert.shape" | "insert.text-box" | "insert.page-break"
  | "home.landscape" | "home.narrow-margins" | "home.moderate-margins" | "home.wide-margins"
  | "home.page-a4" | "home.page-legal" | "home.page-break-before";
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
/** One entry in the `el.commands` snapshot. */
export interface NitroCommandEntry {
  id: NitroCommandId | string;
  label: string;
  shortcut: string | null;
  enabled: boolean;
}

/** Descriptor for a host-added ribbon command. */
export interface NitroRibbonAddEntry {
  /** Unique command id for this host button; fires as `nitro-command` detail.id. */
  id: string;
  /** Button label (text only; no HTML). */
  label: string;
  /** SVG path data for the button icon (the `d` attribute only; no HTML injection). */
  icon?: string;
  /** Tab name to place the button in (not yet routed; reserved for future use). */
  tab?: string;
  /** Group label (not yet routed; reserved for future use). */
  group?: string;
  /** When true the button is disabled in read-only mode. Default: true. */
  editing?: boolean;
}

/** Ribbon configuration set on `el.ribbon`. */
export interface NitroRibbonConfig {
  /** Command ids to hide from the ribbon, menus, and shortcut dispatch. */
  hide?: NitroCommandId[];
  /** Host-added buttons appended at the end of the ribbon / embedded toolbar. */
  add?: NitroRibbonAddEntry[];
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
  /**
   * Ribbon configuration (#590). Set before or after `loadBuffer`/`loadFile`/`loadUrl`.
   * `hide` removes commands from the ribbon, menus, and shortcut dispatch.
   * `add` injects host buttons at the end of the ribbon / embedded toolbar; they fire
   * `nitro-command` and do not reach the built-in command registry.
   */
  ribbon: NitroRibbonConfig | null;
  /**
   * Run a registered command by id (#590). Returns `false` and logs valid ids when `id`
   * is unknown. Fires a cancelable `nitro-command` event before dispatch; `preventDefault()`
   * stops execution. Available after the first `loadBuffer`/`loadFile`/`loadUrl`.
   */
  execCommand(id: NitroCommandId | string): boolean;
  /**
   * Snapshot of all known commands with their current enabled state (#590).
   * Includes built-in commands and any `ribbon.add` host commands.
   * Available after the first `loadBuffer`/`loadFile`/`loadUrl`.
   */
  readonly commands: NitroCommandEntry[];
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
