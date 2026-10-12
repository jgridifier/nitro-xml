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
}
