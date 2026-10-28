// SPDX-License-Identifier: MIT
/**
 * Bundle entry (build-ts.mjs): upstream vanilla pptx viewer. `createPptxViewer` is the public entry of
 * pptx-vanilla-viewer; it wires createRenderController (viewer/render-controller.ts) internally. Test-only.
 */
export { createPptxViewer } from '../../upstream/viewers/pptx/packages/vanilla/dist/index.js';
