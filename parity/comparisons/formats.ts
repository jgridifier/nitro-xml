// SPDX-License-Identifier: MIT
/**
 * Format comparisons still waiting on their Rust dump.
 * xlsx worksheet slice is ready — see comparisons/xlsx.ts (#16).
 * docx styles + body model is ready — see comparisons/docx.ts (#20 / #21).
 * pptx presentation model is ready — see comparisons/pptx.ts (#24).
 * Flip `ready` here when later xlsx CellItem / docx LayoutResult land.
 */

export { pptxComparison } from './pptx.ts';
