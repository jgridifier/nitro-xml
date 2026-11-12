// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/shared-strings.ts; changes listed in PROVENANCE.md.
//! Shared string table built while worksheets are written.

use std::collections::HashMap;

use nitro_xml_opc::xml::ns;

use crate::model::{Font, RichTextRun};

use super::style_xml::font_xml;
use super::xml_out::{t_element, XML_HEADER};

/// The `<si>` body of a plain or rich string.
pub fn rich_string_xml(text: &str, runs: Option<&[RichTextRun]>) -> String {
    let Some(runs) = runs.filter(|r| !r.is_empty()) else {
        return t_element(text);
    };
    let mut out = String::new();
    for run in runs {
        let font = run
            .font
            .as_ref()
            .filter(|font| font_has_props(font))
            .map(|font| font_xml(font, "rPr", false))
            .unwrap_or_default();
        out.push_str(&format!("<r>{font}{}</r>", t_element(&run.text)));
    }
    out
}

fn font_has_props(font: &Font) -> bool {
    font.name.is_some()
        || font.size.is_some()
        || font.bold.is_some()
        || font.italic.is_some()
        || font.underline.is_some()
        || font.strike.is_some()
        || font.color.is_some()
        || font.vert_align.is_some()
        || font.family.is_some()
        || font.scheme.is_some()
}

/// Collects the shared string table while worksheets are written.
#[derive(Default)]
pub struct SharedStringTable {
    items: Vec<String>,
    index: HashMap<String, usize>,
    references: usize,
    source: Option<String>,
    source_len: usize,
    pub inline: bool,
}

impl SharedStringTable {
    /// Retain source indices so clean worksheets never need rewriting when a string is added.
    pub fn from_source(source: Option<&str>, palette: Option<&[String]>) -> Self {
        let mut table = Self {
            inline: source.is_none(),
            ..Self::default()
        };
        if let Some(xml) = source {
            for item in crate::shared_strings::parse_shared_strings(Some(xml), palette) {
                let body = rich_string_xml(&item.text, item.runs.as_deref());
                table.index.entry(body.clone()).or_insert(table.items.len());
                table.items.push(body);
            }
            table.source_len = table.items.len();
            table.source = Some(xml.to_owned());
        }
        table
    }

    pub fn grew(&self) -> bool {
        self.items.len() > self.source_len
    }

    pub fn id(&mut self, text: &str, runs: Option<&[RichTextRun]>) -> usize {
        self.references += 1;
        let body = rich_string_xml(text, runs);
        if let Some(&id) = self.index.get(&body) {
            return id;
        }
        let id = self.items.len();
        self.index.insert(body.clone(), id);
        self.items.push(body);
        id
    }

    pub fn xml(&self) -> String {
        if let Some(source) = &self.source {
            if !self.grew() {
                return source.clone();
            }
            // Counts are optional. Omit them when appending rather than inventing a
            // reference count for clean sheets (which can also contain inline strings).
            let counts = regex::Regex::new(r#"\s+(?:count|uniqueCount)\s*=\s*(?:"[^"]*"|'[^']*')"#)
                .expect("constant regex");
            let start = source
                .find("<sst")
                .or_else(|| source.find(":sst").and_then(|i| source[..i].rfind('<')));
            if let Some(start) = start {
                if let Some(end) = source[start..].find('>').map(|i| start + i + 1) {
                    let opening = counts.replace_all(&source[start..end], "");
                    let added: String = self.items[self.source_len..]
                        .iter()
                        .map(|body| format!("<si xmlns=\"{}\">{body}</si>", ns::X))
                        .collect();
                    if opening.ends_with("/>") {
                        let name = opening[1..]
                            .split(|c: char| c.is_whitespace() || c == '/')
                            .next()
                            .unwrap_or("sst");
                        return format!(
                            "{}{}>{added}</{name}>{}",
                            &source[..start],
                            opening.trim_end_matches("/>"),
                            &source[end..]
                        );
                    }
                    if let Some(close) = source.rfind("</") {
                        let close = super::fragments::direct_child_xml(source)
                            .iter()
                            .find(|(name, _)| name == "extLst")
                            .and_then(|(_, raw)| source.find(raw))
                            .unwrap_or(close);
                        return format!(
                            "{}{}{}{added}{}",
                            &source[..start],
                            opening,
                            &source[end..close],
                            &source[close..]
                        );
                    }
                }
            }
        }

        let items: String = self
            .items
            .iter()
            .map(|body| format!("<si>{body}</si>"))
            .collect();
        format!(
            "{XML_HEADER}<sst xmlns=\"{}\" count=\"{}\" uniqueCount=\"{}\">{items}</sst>",
            ns::X,
            self.references,
            self.items.len()
        )
    }
}
