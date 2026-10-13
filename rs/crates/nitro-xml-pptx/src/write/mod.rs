// SPDX-License-Identifier: MIT
//! PPTX package writer (#87): identity shortcut for clean hosts; dirty saves
//! rewrite only the slide parts marked by shape commands.

mod slide_xml;

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
/// [`PackageWriter`]'s identity shortcut. Otherwise only `host.dirty_parts`
/// slide XML is replaced. Notes parts are TODO(#86); rich runs, bullets,
/// autofit, tables and slide add/delete are TODO(#85) / TODO(#86).
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
