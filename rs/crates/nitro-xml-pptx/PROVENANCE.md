# nitro-xml-pptx provenance

Rows are also recorded in the repo-root `PROVENANCE.md` (licence check).

| File | Upstream source @ e66aaec | Changes |
|------|---------------------------|---------|
| `src/presentation.rs` | `PptxHandlerRuntimeLoadSession.ts`, `LoadPipeline.ts`, `SlideMasters.ts` | Presentation open over `Package`, lazy slides, master/layout/theme chain, issue-24 dump. |
| `src/slide.rs` | `SpTreeParsing.ts`, `ShapeParsing.ts`, `PictureParsing.ts`, `GroupParsing.ts` | Ported element kinds; DrawingML transform/geometry; document-order walk (no raw-XML UTF-8 scan). |
| `src/inheritance.rs` | `merge-xml-objects.ts`, `PlaceholderLookup.ts`, `placeholder-style-family.ts` | Placeholder match + layout-over-master merge. |
| `src/theme.rs` | `ThemeLoading.ts` | Colour/font scheme via `nitro_xml_opc::drawingml` (#25). |
