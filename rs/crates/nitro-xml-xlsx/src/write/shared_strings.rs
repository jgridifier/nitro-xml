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
}

impl SharedStringTable {
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

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn xml(&self) -> String {
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
