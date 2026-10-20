// SPDX-License-Identifier: MIT
//! PPTX package writer (#87): identity shortcut for clean hosts; dirty saves
//! rewrite only the slide parts marked by shape commands.

mod slide_xml;
mod structure;

use std::collections::HashMap;

use nitro_xml_opc::opc::Package;
use nitro_xml_opc::write::PackageWriter;

use crate::edit::host::PptxHost;

use self::slide_xml::{max_numeric_id, rewrite_slide};

/// Options for [`save_pptx_with_options`].
#[derive(Debug, Clone, Default)]
pub struct PptxSaveOptions {
    /// Negative control (#87): skip rewriting dirty `ppt/slides/slideN.xml` parts.
    pub skip_slide_rewrite: bool,
}

/// Save `host` over `original` package bytes.
pub fn save_pptx(host: &PptxHost, original: &[u8]) -> Result<Vec<u8>, String> {
    save_pptx_with_options(host, original, &PptxSaveOptions::default())
}

/// [`save_pptx`] with explicit options (see [`PptxSaveOptions::skip_slide_rewrite`]).
///
/// A clean host, or `skip_slide_rewrite`, returns the original bytes through
/// [`PackageWriter`]'s identity shortcut. A slide-list edit rebuilds
/// `p:sldIdLst`, relationships and slide parts. Otherwise only
/// `host.dirty_parts` slide XML is replaced. Notes parts are TODO(#86); rich
/// runs and autofit are TODO(#85) / TODO(#86). List markers (`a:buChar`,
/// `a:buAutoNum`, `a:buNone`) and authored tables (`a:tbl`) are written
/// inside the dirty slide.
pub fn save_pptx_with_options(
    host: &PptxHost,
    original: &[u8],
    options: &PptxSaveOptions,
) -> Result<Vec<u8>, String> {
    if !host.is_dirty() || options.skip_slide_rewrite {
        return PackageWriter::open(original)
            .map_err(|e| e.to_string())?
            .finish()
            .map_err(|e| e.to_string());
    }
    if host.structure_dirty {
        return structure::save_with_structure(host, original);
    }

    let mut pkg = Package::open(original).map_err(|e| e.to_string())?;
    let mut max_id = 0i64;
    let mut sources: HashMap<String, String> = HashMap::new();
    for slide in &host.slides {
        let xml = {
            let text = pkg.text(&slide.path).map_err(|e| e.to_string())?;
            text.map(str::to_owned)
        };
        let Some(xml) = xml else {
            return Err(format!("missing part {}", slide.path));
        };
        max_id = max_id.max(max_numeric_id(&xml)?);
        if host.dirty_parts.contains(&slide.path) {
            sources.insert(slide.path.clone(), xml);
        }
    }

    // Upstream `PptxElementXmlBuilder.nextId` is one past the max `@_id` across
    // every slide part, then incremented as new shapes are serialized.
    let mut next_id = max_id + 1;
    let mut writer = PackageWriter::open(original).map_err(|e| e.to_string())?;
    for slide in &host.slides {
        if !host.dirty_parts.contains(&slide.path) {
            continue;
        }
        let src = sources
            .get(&slide.path)
            .ok_or_else(|| format!("missing dirty part {}", slide.path))?;
        let rewritten = rewrite_slide(src, slide, &mut next_id)?;
        // `replace` keeps content types, rels and every other part byte-identical.
        writer.replace(&slide.path, rewritten.into_bytes());
    }
    writer.finish().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::host::{GeometryPatch, PptxHost};

    // Catches: clean-host save taking the PackageWriter identity shortcut without touching slide XML.
    #[test]
    fn clean_host_returns_original_bytes() {
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let host = PptxHost::open(bytes.as_slice()).expect("open");
        assert!(!host.is_dirty());
        let out = save_pptx(&host, bytes.as_slice()).expect("save");
        assert_eq!(out.as_slice(), bytes.as_slice());
    }

    // Catches: --skip-slide-rewrite still rewriting a dirty slide.
    #[test]
    fn skip_slide_rewrite_leaves_original_bytes() {
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_shape_text(0, None, Some(0), "Rewritten title")
            .expect("text");
        assert!(host.is_dirty());
        let skipped = save_pptx_with_options(
            &host,
            bytes.as_slice(),
            &PptxSaveOptions {
                skip_slide_rewrite: true,
            },
        )
        .expect("skip");
        assert_eq!(skipped.as_slice(), bytes.as_slice());
        let dirty = save_pptx(&host, bytes.as_slice()).expect("dirty");
        assert_ne!(dirty.as_slice(), bytes.as_slice());
        let xml = slide_xml(&dirty, "ppt/slides/slide1.xml");
        assert!(xml.contains("Rewritten title"), "{xml}");
        assert!(!slide_xml(&dirty, "ppt/slides/slide2.xml").contains("Rewritten title"));
    }

    #[test]
    fn catches_underline_and_align_dropped_on_slide_save() {
        // Catches: a slide save that drops a:rPr/@u and a:pPr/@algn, so reopen loses underline and center.
        use crate::edit::host::TextStylePatch;
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_text_style(
            0,
            None,
            Some(0),
            TextStylePatch {
                underline: Some(true),
                ..TextStylePatch::default()
            },
        )
        .expect("underline");
        host.set_paragraph_align(0, None, Some(0), 0, "center")
            .expect("align");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let again = PptxHost::open(&saved).expect("reopen");
        let el = &again.slides[0].elements[0];
        assert!(
            el.text_segments.iter().any(|s| s.underline),
            "reopened shape lost underline"
        );
        assert!(
            el.text_segments
                .iter()
                .any(|s| s.align.as_deref() == Some("center")),
            "reopened shape lost center align: {:?}",
            el.text_segments
        );
    }

    #[test]
    fn catches_justify_and_list_markers_dropped_on_slide_save() {
        // Catches: a slide save that omits a:pPr/@algn="just", a:buChar, or a:buAutoNum.
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_paragraph_align(0, None, Some(0), 0, "justify")
            .expect("justify");
        host.set_bullets(0, None, Some(0), "bullet", None, None)
            .expect("bullets");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save bullets");
        let again = PptxHost::open(&saved).expect("reopen bullets");
        let el = &again.slides[0].elements[0];
        let info = el
            .text_segments
            .iter()
            .find_map(|s| s.bullet_info.as_ref())
            .expect("bullet marker missing after reopen");
        assert_eq!(info.char.as_deref(), Some("•"));
        assert!(
            el.text_segments
                .iter()
                .any(|s| s.align.as_deref() == Some("justify")),
            "justify missing after reopen: {:?}",
            el.text_segments
        );

        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.set_bullets(0, None, Some(0), "numbered", None, None)
            .expect("numbering");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save numbering");
        let again = PptxHost::open(&saved).expect("reopen numbering");
        let info = again.slides[0].elements[0]
            .text_segments
            .iter()
            .find_map(|s| s.bullet_info.as_ref())
            .expect("numbering missing after reopen");
        assert_eq!(info.auto_num_type.as_deref(), Some("arabicPeriod"));
    }

    // Catches: new, duplicated and deleted slides never landing in the package, and a text box saved without txBox.
    #[test]
    fn catches_slide_structure_and_text_box_missing_after_reopen() {
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let opened = PptxHost::open(bytes.as_slice()).expect("open");
        let before = opened.slides.len();
        let original_text = slide_plain(&opened.slides[0]);
        assert!(
            !original_text.is_empty(),
            "fixture slide has no text to duplicate"
        );

        let mut host = opened.clone();
        host.add_slide(None).expect("add");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save add");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let again = PptxHost::open(&saved).expect("reopen add");
        assert_eq!(
            again.slides.len(),
            before + 1,
            "new slide missing after reopen"
        );

        let mut host = opened.clone();
        host.duplicate_slide(0).expect("dup");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save dup");
        let again = PptxHost::open(&saved).expect("reopen dup");
        assert_eq!(again.slides.len(), before + 1);
        assert_eq!(
            slide_plain(&again.slides[1]),
            original_text,
            "duplicated slide text diverged"
        );

        let mut host = opened.clone();
        let removed = host.slides[before - 1].path.clone();
        host.delete_slide((before - 1) as u32).expect("delete");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save delete");
        let again = PptxHost::open(&saved).expect("reopen delete");
        assert_eq!(
            again.slides.len(),
            before - 1,
            "deleted slide still in the package"
        );
        assert!(
            Package::open(saved.as_slice())
                .expect("pkg")
                .text(&removed)
                .expect("read")
                .is_none(),
            "deleted slide part {removed} was left in the package"
        );

        let mut host = opened;
        let count = host.slides[0].elements.len();
        host.add_text_box(0, "text-box-0", 80.0, 80.0, 240.0, 80.0)
            .expect("text box");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save text box");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(
            xml.contains("txBox=\"1\""),
            "text box was not written: {xml}"
        );
        let again = PptxHost::open(&saved).expect("reopen text box");
        assert!(
            again.slides[0].elements.len() > count,
            "text box missing after reopen"
        );
    }

    // Catches: saving a moved shape re-quantizes an untouched non-multiple EMU axis.
    #[test]
    fn untouched_emu_axis_survives_partial_move() {
        let bytes = emu_package();
        let mut host = PptxHost::open(&bytes).expect("open");
        let before = host.slide(0).expect("slide").elements[0].x_emu;
        assert_eq!(before, Some(10_000), "parser dropped the stored x EMU");
        host.patch_geometry(
            0,
            None,
            Some(0),
            GeometryPatch {
                y: Some(40.0),
                ..GeometryPatch::default()
            },
        )
        .expect("move");
        let saved = save_pptx(&host, &bytes).expect("save");
        let xml = slide_xml(&saved, "ppt/slides/slide1.xml");
        assert!(
            xml.contains("x=\"10000\""),
            "untouched axis was re-quantized: {xml}"
        );
        assert!(
            xml.contains("y=\"381000\""),
            "moved axis was not re-quantized: {xml}"
        );
    }

    #[test]
    fn catches_reordered_slides_restored_to_the_open_order() {
        // Catches: reorder writing the slide list in the original order, so reopen shows slide 1 first.
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        let first = host.slides[0].path.clone();
        let second = host.slides[1].path.clone();
        host.reorder_slide(0, 1).expect("reorder");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        let again = PptxHost::open(&saved).expect("reopen");
        assert_eq!(again.slides[0].path, second);
        assert_eq!(again.slides[1].path, first);
    }

    #[test]
    fn catches_inserted_slide_table_missing_rows_after_reopen() {
        // Catches: insert table / row / column edits not writing a:tbl, so reopen has no extra grid.
        let bytes = include_bytes!("../../../../../fixtures/generated/pptx/slides_10.pptx");
        let mut host = PptxHost::open(bytes.as_slice()).expect("open");
        host.add_table(0, "table-0", 80.0, 80.0, 360.0, 120.0)
            .expect("table");
        host.insert_slide_row(0, Some("table-0"), None, 1, "below")
            .expect("row");
        host.insert_slide_column(0, Some("table-0"), None, 1, "right")
            .expect("col");
        host.set_slide_cell_text(0, Some("table-0"), None, 2, 2, "tail")
            .expect("cell");
        let saved = save_pptx(&host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let again = PptxHost::open(&saved).expect("reopen");
        let table = again.slides[0]
            .elements
            .iter()
            .find(|el| el.type_ == "table")
            .expect("table missing after reopen");
        let model = table.table.as_ref().expect("table model");
        assert_eq!(model.rows.len(), 3, "inserted row missing");
        assert_eq!(model.column_widths.len(), 3, "inserted column missing");
        assert_eq!(model.rows[2].cells[2].text, "tail");
        host.delete_slide_row(0, Some("table-0"), None, 2)
            .expect("delete row");
        host.delete_slide_column(0, Some("table-0"), None, 2)
            .expect("delete col");
        let trimmed = save_pptx(&host, bytes.as_slice()).expect("save trim");
        let trimmed_host = PptxHost::open(&trimmed).expect("reopen trim");
        let trimmed_table = trimmed_host.slides[0]
            .elements
            .iter()
            .find(|el| el.type_ == "table")
            .expect("table missing after delete");
        let trimmed_model = trimmed_table.table.as_ref().expect("model");
        assert_eq!(trimmed_model.rows.len(), 2);
        assert_eq!(trimmed_model.column_widths.len(), 2);
    }

    fn slide_plain(slide: &crate::edit::host::EditableSlide) -> String {
        let mut out = String::new();
        for el in &slide.elements {
            if let Some(text) = &el.text {
                out.push_str(text);
            }
        }
        out
    }

    fn slide_xml(pkg: &[u8], name: &str) -> String {
        let mut package = Package::open(pkg).expect("reopen");
        package
            .text(name)
            .expect("read")
            .unwrap_or_default()
            .to_owned()
    }

    fn emu_package() -> Vec<u8> {
        use std::io::{Cursor, Write};
        let pres = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst>
  <p:sldSz cx="9144000" cy="5143500"/>
</p:presentation>"#;
        let pres_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
</Relationships>"#;
        let root_rels = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#;
        let slide = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld><p:spTree>
    <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
    <p:grpSpPr/>
    <p:sp>
      <p:nvSpPr><p:cNvPr id="2" name="Box"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
      <p:spPr><a:xfrm><a:off x="10000" y="20000"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
      <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Box</a:t></a:r></a:p></p:txBody>
    </p:sp>
  </p:spTree></p:cSld>
</p:sld>"#;
        let types = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
</Types>"#;
        let parts: &[(&str, &[u8])] = &[
            ("[Content_Types].xml", types),
            ("_rels/.rels", root_rels),
            ("ppt/presentation.xml", pres),
            ("ppt/_rels/presentation.xml.rels", pres_rels),
            ("ppt/slides/slide1.xml", slide),
        ];
        let mut cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in parts {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        cursor.into_inner()
    }
}
