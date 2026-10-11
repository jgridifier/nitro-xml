// SPDX-License-Identifier: MIT
/**
 * Re-exports format comparisons registered elsewhere.
 * xlsx worksheet + view-IR is ready — see comparisons/xlsx.ts (#16 / #18).
 * docx styles + body model is ready — see comparisons/docx.ts (#20 / #21).
 * docx Print Layout IR is ready — see comparisons/docx-layout.ts (#22).
 * pptx presentation model is ready — see comparisons/pptx.ts (#24).
 * Flip `ready` here when later format dumps land.
 */

export { pptxComparison } from './pptx.ts';
