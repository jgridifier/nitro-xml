// SPDX-License-Identifier: MIT
/** Nitro XML loader — only shipped JS: wasm init, events, command apply, batched read-backs. */

export {
  ACCEPT,
  SUPPORTED_EXTENSIONS,
  UNSUPPORTED_BROWSER_MESSAGE,
  UNSUPPORTED_FORMAT_MESSAGE,
  extensionOf,
  isSupportedExtension,
} from "./formats.js";

export {
  initCore,
  compileWasmModule,
  loadFormatModule,
  loadEditorModule,
  prewarmWorker,
  takePrewarmedWorker,
  wasmSupported,
  defaultGlueUrl,
  defaultWasmUrl,
  unsupportedBrowserError,
  unsupportedFormatError,
} from "./bootstrap.js";

export { Op, applyCommandBuffer, cssColor } from "./apply.js";
export { EventKind, EVENT_VERSION, encodeEventBatch, createEventCoalescer } from "./events.js";
export { flushMeasureBatch, readMeasureReqsFromMemory } from "./measure.js";
export {
  LayoutReadKind,
  LAYOUT_RESULT_LANES,
  flushLayoutReadBatch,
  readLayoutReqsFromMemory,
  packLayoutReqs,
} from "./layout-readback.js";
export { openDocument, DocumentSession } from "./session.js";
export { WorkerDocument } from "./worker-host.js";
export { defineNitroXml, NitroXmlElement } from "./element.js";
export { defineNitroXlsx, NitroXlsxElement } from "./xlsx-element.js";
export { defineNitroPptx, NitroPptxElement } from "./pptx-element.js";
export {
  mountShellChrome,
  bindShellUi,
  followHyperlink,
} from "./shell-chrome.js";
export {
  applyFontSourceToHandle,
  setDefaultFontSource,
  getDefaultFontSource,
} from "./font-source.js";
