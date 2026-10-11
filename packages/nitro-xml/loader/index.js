// SPDX-License-Identifier: MIT
/**
 * Nitro XML loader — the only shipped JavaScript.
 * Instantiates wasm, forwards browser events, applies each frame's command buffer,
 * and returns batched DOM read-backs (text measurement and post-apply layout)
 * once per frame.
 */

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

export { Op, applyCommandBuffer, cssColor, encodeFillRect, encodeStrokeRect } from "./apply.js";
export { EventKind, EVENT_VERSION, encodeEventBatch, createEventCoalescer } from "./events.js";
export { flushMeasureBatch, readMeasureReqsFromMemory } from "./measure.js";
export {
  LayoutReadKind,
  LAYOUT_RESULT_LANES,
  flushLayoutReadBatch,
  readLayoutReqsFromMemory,
} from "./layout-readback.js";
export { openDocument, DocumentSession } from "./session.js";
export { WorkerDocument } from "./worker-host.js";
export { defineNitroXml, NitroXmlElement } from "./element.js";
export {
  applyFontSourceToHandle,
  setDefaultFontSource,
  getDefaultFontSource,
} from "./font-source.js";
