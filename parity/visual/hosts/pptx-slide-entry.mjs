// SPDX-License-Identifier: MIT
/**
 * Bundle entry for upstream pptx slide-stage painters (#27 visual gate).
 * Prepared by prepare.mjs into out/bundles/pptx-slide.js (test-only).
 *
 * Mirrors tools/painter-calls: loadPresentation + renderSlideStage through the
 * default element registry — the same path the vanilla viewer uses on open.
 */
export {
	loadPresentation,
	renderSlideStage,
	createDefaultRegistry,
	createTranslator,
} from '../../../upstream/src/ui/dist/pptx/dom/index.mjs';
