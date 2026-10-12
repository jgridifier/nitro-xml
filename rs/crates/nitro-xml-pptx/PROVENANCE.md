# nitro-xml-pptx provenance

Rows are also recorded in the repo-root `PROVENANCE.md` (licence check).

| File | Upstream source @ e66aaec | Changes |
|------|---------------------------|---------|
| `src/presentation.rs` | `PptxHandlerRuntimeLoadSession.ts`, `LoadPipeline.ts`, `SlideMasters.ts` | Presentation open over `Package`, lazy slides, master/layout/theme chain, issue-24 dump. |
| `src/slide.rs` | `SpTreeParsing.ts`, `ShapeParsing.ts`, `PictureParsing.ts`, `GroupParsing.ts`, `PptxHandlerRuntimeBackgroundParsing.ts` | Ported element kinds; background chain; shape fill/stroke; chart/SmartArt frames. |
| `src/inheritance.rs` | `merge-xml-objects.ts`, `PlaceholderLookup.ts`, `placeholder-style-family.ts` | Placeholder match + layout-over-master merge. |
| `src/theme.rs` | `ThemeLoading.ts` | Colour/font scheme via `nitro_xml_opc::drawingml` (#25). |
| `src/text.rs` | `PptxHandlerRuntimeShapeParagraphContentParsing.ts`, `BulletParsing.ts`, `ruby-element-parsing.ts` | Flat `element.text` parity: bullets, fields, math, breaks, ruby. |
| `src/auto_number.rs` | `auto-number-format.ts`, `auto-number-scripts.ts` | `ST_TextAutonumberScheme` markers for list text. |
| `src/level_style.rs` | `PlaceholderStyles.ts`, `MasterElements.ts`, `PlaceholderDefaults.ts` | `lstStyle` / master `txStyles` bullet level styles. |
| `src/placeholder_defaults.rs` | `PlaceholderLookup.ts`, `PlaceholderDefaults.ts` | Effective placeholder level-style cascade for bullets. |
| `src/table.rs` | `PptxGraphicFrameParser.ts` | Table row count on graphic-frame dump elements. |
| `src/render/stage.rs` | `dom/slide-stage.ts`, `render/slide-background.ts` | P1 stage; solid/linear background (#27). |
| `src/render/shape.rs` | `dom/elements/text-shape.ts` | P2 fill, stroke, `box-shadow` from outerShdw (#27). |
| `src/outer_shadow.rs` | `render/visual-effects.ts`, `PptxShapeEffectStyleExtractor.ts`, `render/text-effects.ts` | Parse `a:outerShdw` → CSS `box-shadow` / `text-shadow`; `a:glow` → CSS `filter: drop-shadow` (#27). |
| `src/visual_3d_materials.rs` | `render/visual-3d-materials.ts`, `render/visual-3d.ts` | Metal+bevel legacy brightness compensation until SVG bevel lighting (#27). |
| `src/render/prst_geom.rs` | `render/shape-geometry-cascade.ts`, `core/geometry/shape-geometry.ts`, `geometry/preset-clip-paths-core.ts` | `a:prstGeom` border-radius and clip-path for ellipse, roundRect, arrows (#27). |
| `src/render/text.rs` | `dom/elements/text-block.ts`, `render/text-body-layout.ts`, `render/text-effects.ts`, `render/text-run-style.ts` | P3 text body; `bodyPr` anchor + `pPr` align via flex; run `outerShdw` → `text-shadow`; first-run font + text outline (#27). |
| `src/render/shape_autofit_resize.rs` | `render/shape-autofit-resize.ts` | `a:spAutoFit` shape height from post-apply `ScrollHeight` (#27). |
| `src/text_layout.rs` | `text-body-properties-parser.ts`, `text-body-layout.ts` | Parse `a:bodyPr/@anchor`, insets, paragraph `algn`, and autofit mode for paint. |
| `src/text_body_rect.rs` | `text-body-rect.ts` | COM-verified preset geometry `textRect` as extra body padding atop `a:bodyPr` insets. |
| `src/verified_text_rect_presets.rs` | `verified-text-rect-presets.ts` | Allowlist gating which presets honour geometry text rectangles. |
| `src/render/smartart.rs` | `dom/elements/smartart-svg.ts` | P11 S2 cached-drawing paint at the element box (#27). |
| `src/render/picture.rs` | `dom/elements/image.ts` | P5 picture frame; `a:glow` on `img` (#27). |
| `src/render/group.rs` | `dom/elements/group.ts` | P6 group transform + child recursion (#27). |
| `src/render/connector.rs` | `dom/elements/connector.ts` | P7 connector SVG stub (#27). |
| `src/render/table.rs` | `dom/elements/table.ts` | P8 table grid stub (#27). |

