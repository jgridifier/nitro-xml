# PROVENANCE.md

Ported code from upstream ChristopherVR/ooxml@e66aaec:

## PowerPoint rendering

### Outline warping (Issue #27: Text Warp)

- `rs/crates/nitro-xml-pptx/src/render/wordart_envelope_outline_warp.rs` (Apache-2.0):
  - Ported `flattenOutline`, `buildWarpedGlyphOutlinePathD` from `src/ui/src/pptx/render/text-warp-glyph-outline.ts`
  - Changes: `max_segment` parameter instead of constant, no Bezier curve support (precomputed from JS), coordinate format aligned with Rust layout space
  
- `packages/nitro-xml/loader/glyph-outline.js` (Apache-2.0):
  - Ported `contourRaster`, `simplify` from `src/ui/src/pptx/render/text-warp-glyph-trace.ts` and `src/ui/src/pptx/render/text-warp-glyph-trace-contour.ts`
  - Changes: No DOM/webfont integration (runs in host only), cache structure, simplified caching protocol

