// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/shared-strings.ts; changes listed in
// PROVENANCE.md.
//! Shared string table (`xl/sharedStrings.xml`).

use nitro_xml_opc::xml::parse_xml_str;

use crate::model::{RichTextRun, SharedString};
use crate::style_parts::parse_font;
use crate::xml_util::{x_children, x_first, x_text};

/**
 * Reads a `CT_Rst` (`<si>` or `<is>`): plain `<t>` or rich `<r>` runs. Phonetic runs (`rPh`)
 * are ignored.
 */
pub fn parse_rich_string(
    element: &nitro_xml_opc::xml::Element,
    palette: Option<&[String]>,
) -> SharedString {
    let runs = x_children(element, "r");
    if runs.is_empty() {
        return SharedString {
            text: x_text(x_first(Some(element), "t")),
            runs: None,
        };
    }
    let parsed: Vec<RichTextRun> = runs
        .into_iter()
        .map(|run| {
            let mut out = RichTextRun {
                text: x_text(x_first(Some(run), "t")),
                font: None,
            };
            if let Some(r_pr) = x_first(Some(run), "rPr") {
                let font = parse_font(Some(r_pr), palette, false);
                if font
                    .to_json()
                    .as_object()
                    .map(|o| !o.is_empty())
                    .unwrap_or(false)
                {
                    out.font = Some(font);
                }
            }
            out
        })
        .collect();
    let text = parsed.iter().map(|r| r.text.as_str()).collect::<String>();
    SharedString {
        text,
        runs: Some(parsed),
    }
}

pub fn parse_shared_strings(xml: Option<&str>, palette: Option<&[String]>) -> Vec<SharedString> {
    let Some(xml) = xml else {
        return Vec::new();
    };
    let Ok(doc) = parse_xml_str(xml) else {
        return Vec::new();
    };
    x_children(&doc.root, "si")
        .into_iter()
        .map(|si| parse_rich_string(si, palette))
        .collect()
}
