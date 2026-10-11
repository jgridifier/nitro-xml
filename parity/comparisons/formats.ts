// SPDX-License-Identifier: MIT
/**
 * Re-exports format comparisons registered elsewhere.
 * xlsx worksheet + view-IR is ready — see comparisons/xlsx.ts (#16 / #18).
 * docx styles + body model is ready — see comparisons/docx.ts (#20 / #21).
 * pptx presentation model is ready — see comparisons/pptx.ts (#24).
 * Flip `ready` for later docx LayoutResult when that dump lands.
 */

export { pptxComparison } from './pptx.ts';
