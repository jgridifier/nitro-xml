// SPDX-License-Identifier: MIT
//! DOCX package writer (#80): identity shortcut for clean hosts; dirty saves rewrite `word/document.xml`.

mod apply;
mod document_xml;
mod element;
mod inline;
mod numbering_part;
mod paragraph;
mod picture;
mod run;
mod table;
mod xml_escape;

use nitro_xml_opc::write::PackageWriter;

use crate::edit::host::DocxHost;
use nitro_xml_opc::xml::parse_xml_str;

use self::apply::apply_model;
use self::document_xml::build_document_xml;
use self::numbering_part::write_numbering;
use self::picture::write_inline_pictures;
use self::xml_escape::XML_HEADER;

const CT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

/// Options for [`save_docx_with_options`].
#[derive(Debug, Clone, Default)]
pub struct DocxSaveOptions {
    /// Negative control (#80): skip rewriting `word/document.xml` even when the host is dirty.
    pub skip_document_rewrite: bool,
}

/// Save `host` over `original` package bytes.
pub fn save_docx(host: &DocxHost, original: &[u8]) -> Result<Vec<u8>, String> {
    save_docx_with_options(host, original, &DocxSaveOptions::default())
}

/// [`save_docx`] with explicit options (see [`DocxSaveOptions::skip_document_rewrite`]).
pub fn save_docx_with_options(
    host: &DocxHost,
    original: &[u8],
    options: &DocxSaveOptions,
) -> Result<Vec<u8>, String> {
    if !host.is_dirty() {
        return PackageWriter::open(original)
            .map_err(|e| e.to_string())?
            .finish()
            .map_err(|e| e.to_string());
    }
    if options.skip_document_rewrite {
        return PackageWriter::open(original)
            .map_err(|e| e.to_string())?
            .finish()
            .map_err(|e| e.to_string());
    }

    let mut doc = parse_xml_str(&host.source_document_xml).map_err(|e| e.to_string())?;
    apply_model(&mut doc, host)?;
    let mut xml = format!("{}{}", XML_HEADER, build_document_xml(&doc.root));
    let mut writer = PackageWriter::open(original).map_err(|e| e.to_string())?;
    write_inline_pictures(&mut writer, original, host, &mut xml)?;
    writer.add_text("word/document.xml", xml, Some(CT_DOCUMENT));
    write_numbering(&mut writer, original, host)?;
    writer.finish().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::host::DocxHost;

    // Catches: clean-host save taking the PackageWriter identity shortcut without touching document.xml.
    #[test]
    fn clean_host_returns_original_bytes() {
        let bytes =
            include_bytes!("../../../../../fixtures/generated/docx/painter_line_numbers.docx");
        let host = DocxHost::open(bytes.as_slice()).expect("open");
        let out = save_docx(&host, bytes.as_slice()).expect("save");
        assert_eq!(out.as_slice(), bytes.as_slice());
    }

    // Catches: inserting a table leaving word/document.xml without a new w:tbl, so reopen has no extra table.
    #[test]
    fn catches_inserted_table_missing_after_reopen() {
        let bytes = include_bytes!("../../../../../fixtures/generated/docx/pages_10.docx");
        let mut host = DocxHost::open(bytes.as_slice()).expect("open");
        let before = host
            .blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
            .count();
        crate::edit::table::insert_table(&mut host, 1, 2, 2).expect("insert");
        let saved = save_docx(&host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let again = DocxHost::open(&saved).expect("reopen");
        let after = again
            .blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
            .count();
        assert_eq!(after, before + 1, "reopen dropped the new table");
        let table = &again.blocks[1];
        assert_eq!(
            table["type"], "table",
            "inserted table was not the second block"
        );
        let rows = table["rows"].as_array().expect("rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].as_array().expect("row").len(), 2);
    }

    #[test]
    fn catches_deleted_table_still_present_after_reopen() {
        // Catches: deleting a table leaving its w:tbl in word/document.xml.
        let bytes = include_bytes!("../../../../../fixtures/generated/docx/pages_10.docx");
        let mut host = DocxHost::open(bytes.as_slice()).expect("open");
        let before_blocks = host.blocks.len();
        let before = host
            .blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
            .count();
        assert!(before >= 1, "fixture has no table to delete");
        let at = host
            .blocks
            .iter()
            .position(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
            .expect("table");
        crate::edit::table::delete_table(&mut host, at).expect("delete");
        let saved = save_docx(&host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let again = DocxHost::open(&saved).expect("reopen");
        let after = again
            .blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
            .count();
        assert_eq!(after, before - 1, "reopen still has the deleted table");
        assert_eq!(again.blocks.len(), before_blocks - 1);
    }

    #[test]
    fn catches_numbering_missing_from_document_and_numbering_xml() {
        // Catches: a numbered paragraph saved without w:numPr or word/numbering.xml.
        let bytes = include_bytes!("../../../../../fixtures/generated/docx/pages_10.docx");
        let mut host = DocxHost::open(bytes.as_slice()).expect("open");
        let block = host
            .blocks
            .iter()
            .position(|b| b.get("type").and_then(|t| t.as_str()) == Some("paragraph"))
            .expect("paragraph");
        let id = crate::edit::numbering::ensure_list_definition(&mut host, "decimal").expect("def");
        crate::edit::numbering::apply_numbering(&mut host, block, id, 0).expect("apply");
        let saved = save_docx(&host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let again = DocxHost::open(&saved).expect("reopen");
        let numbering = again.blocks[block]
            .get("numbering")
            .expect("numbering missing after reopen");
        assert_eq!(numbering["numId"], id);
        assert_eq!(numbering["level"], 0);
        let fmt = again
            .numbering_catalog
            .as_ref()
            .and_then(|c| c.get("nums"))
            .and_then(|n| n.get(id.to_string()))
            .and_then(|n| n.get("abstractNumId"))
            .and_then(|v| v.as_str())
            .and_then(|abs| {
                again
                    .numbering_catalog
                    .as_ref()
                    .unwrap()
                    .get("abstractNums")
                    .and_then(|a| a.get(abs))
                    .and_then(|a| a.get("levels"))
                    .and_then(|l| l.get("0"))
                    .and_then(|l| l.get("numFmt"))
                    .and_then(|v| v.as_str())
            });
        assert_eq!(fmt, Some("decimal"));
    }

    #[test]
    fn catches_inline_picture_missing_from_document_and_media() {
        // Catches: insert picture saving without w:drawing or word/media, so reopen has no image.
        use crate::edit::drawing::{append_inline_picture, INSERTED_PNG};
        use nitro_xml_opc::opc::Package;
        let bytes = include_bytes!("../../../../../fixtures/generated/docx/pages_10.docx");
        let mut host = DocxHost::open(bytes.as_slice()).expect("open");
        append_inline_picture(&mut host, 0).expect("picture");
        let saved = save_docx(&host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let again = DocxHost::open(&saved).expect("reopen");
        let image = again
            .blocks
            .iter()
            .filter_map(|b| b.get("runs")?.as_array())
            .flatten()
            .find_map(|run| run.get("image"))
            .expect("image missing after reopen");
        let part = image.get("partName").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            part.contains("word/media/"),
            "partName {part} is not a media part"
        );
        assert_eq!(
            image.get("contentType").and_then(|v| v.as_str()),
            Some("image/png")
        );
        let mut pkg = Package::open(&saved).expect("pkg");
        let png = pkg.read(part).expect("read").expect("png bytes");
        assert_eq!(png, INSERTED_PNG);
    }

    #[test]
    fn catches_strike_and_highlight_missing_from_saved_run() {
        // Catches: strikethrough or highlight saving without w:strike / w:highlight, so reopen drops them.
        use crate::edit::format::set_run_formatting;
        use serde_json::json;
        let bytes = include_bytes!("../../../../../fixtures/generated/docx/pages_10.docx");
        let mut host = DocxHost::open(bytes.as_slice()).expect("open");
        let block = host
            .blocks
            .iter()
            .position(|b| b.get("type").and_then(|t| t.as_str()) == Some("paragraph"))
            .expect("paragraph");
        let mut patch = serde_json::Map::new();
        patch.insert("strike".into(), json!(true));
        patch.insert("highlight".into(), json!("yellow"));
        set_run_formatting(&mut host, block, 0, &patch).expect("format");
        let saved = save_docx(&host, bytes.as_slice()).expect("save");
        let again = DocxHost::open(&saved).expect("reopen");
        let run = &again.blocks[block]["runs"][0];
        assert_eq!(run["strike"], true, "reopened run lost strike: {run}");
        assert_eq!(
            run["highlight"], "yellow",
            "reopened run lost highlight: {run}"
        );
    }

    #[test]
    fn catches_indent_spacing_script_font_and_bullets_missing_after_reopen() {
        // Catches: a paragraph save that drops w:ind, w:spacing, w:vertAlign, w:rFonts, or a bullet numPr.
        use crate::edit::format::{
            adjust_left_indent, cycle_line_spacing, set_run_formatting, INDENT_STEP_TWIPS,
        };
        use crate::edit::numbering::toggle_bullet_list;
        use serde_json::json;
        let bytes = include_bytes!("../../../../../fixtures/generated/docx/pages_10.docx");
        let mut host = DocxHost::open(bytes.as_slice()).expect("open");
        let block = host
            .blocks
            .iter()
            .position(|b| b.get("type").and_then(|t| t.as_str()) == Some("paragraph"))
            .expect("paragraph");
        assert!(adjust_left_indent(&mut host, block, INDENT_STEP_TWIPS).expect("indent"));
        cycle_line_spacing(&mut host, block).expect("spacing");
        let mut patch = serde_json::Map::new();
        patch.insert("verticalAlign".into(), json!("superscript"));
        patch.insert("fontFamily".into(), json!("Georgia"));
        set_run_formatting(&mut host, block, 0, &patch).expect("run");
        toggle_bullet_list(&mut host, block).expect("bullets");
        let saved = save_docx(&host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let again = DocxHost::open(&saved).expect("reopen");
        let para = &again.blocks[block];
        assert_eq!(para["indentLeftTwips"], 720, "indent: {para}");
        assert_eq!(para["lineSpacingTwips"], 360, "spacing: {para}");
        assert_eq!(para["lineSpacingRule"], "auto");
        assert_eq!(para["runs"][0]["verticalAlign"], "superscript");
        assert_eq!(para["runs"][0]["fontFamily"], "Georgia");
        let num_id = para["numbering"]["numId"].as_i64().expect("numId");
        let fmt = again
            .numbering_catalog
            .as_ref()
            .and_then(|c| c.get("nums"))
            .and_then(|n| n.get(num_id.to_string()))
            .and_then(|n| n.get("abstractNumId"))
            .and_then(|v| v.as_str())
            .and_then(|abs| {
                again
                    .numbering_catalog
                    .as_ref()
                    .unwrap()
                    .get("abstractNums")
                    .and_then(|a| a.get(abs))
                    .and_then(|a| a.get("levels"))
                    .and_then(|l| l.get("0"))
                    .and_then(|l| l.get("numFmt"))
                    .and_then(|v| v.as_str())
            });
        assert_eq!(fmt, Some("bullet"));

        let mut host = DocxHost::open(bytes.as_slice()).expect("open");
        let mut patch = serde_json::Map::new();
        patch.insert("verticalAlign".into(), json!("subscript"));
        set_run_formatting(&mut host, block, 0, &patch).expect("sub");
        let saved = save_docx(&host, bytes.as_slice()).expect("save sub");
        let again = DocxHost::open(&saved).expect("reopen sub");
        assert_eq!(again.blocks[block]["runs"][0]["verticalAlign"], "subscript");

        let mut host = DocxHost::open(bytes.as_slice()).expect("open");
        assert!(adjust_left_indent(&mut host, block, INDENT_STEP_TWIPS).expect("in"));
        assert!(adjust_left_indent(&mut host, block, -INDENT_STEP_TWIPS).expect("out"));
        assert!(!host.is_dirty(), "indent then outdent left the host dirty");
        assert_eq!(
            save_docx(&host, bytes.as_slice()).expect("clean"),
            bytes.as_slice()
        );
    }
}
