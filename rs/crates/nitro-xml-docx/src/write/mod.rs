// SPDX-License-Identifier: MIT
//! DOCX package writer (#80): identity shortcut for clean hosts; dirty saves rewrite `word/document.xml`.

mod apply;
mod document_xml;
mod element;
mod inline;
mod paragraph;
mod run;
mod table;
mod xml_escape;

use nitro_xml_opc::write::PackageWriter;

use crate::edit::host::DocxHost;
use nitro_xml_opc::xml::parse_xml_str;

use self::apply::apply_model;
use self::document_xml::build_document_xml;
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
    let xml = format!("{}{}", XML_HEADER, build_document_xml(&doc.root));
    let mut writer = PackageWriter::open(original).map_err(|e| e.to_string())?;
    writer.add_text("word/document.xml", xml, Some(CT_DOCUMENT));
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
}
