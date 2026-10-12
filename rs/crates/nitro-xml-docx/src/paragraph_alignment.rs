// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/paragraph-alignment.ts; changes listed in
// PROVENANCE.md.

use nitro_xml_opc::xml::Element;

use crate::simple_types::on_off_element;
use crate::xml_util::{first_w, get_w};

pub type ParagraphAlign = &'static str;

pub fn align_from_justification(jc: &str, rtl: bool) -> Option<ParagraphAlign> {
    match jc {
        "left" | "center" | "right" => Some(match jc {
            "left" => "left",
            "center" => "center",
            _ => "right",
        }),
        "start" => Some(if rtl { "right" } else { "left" }),
        "end" => Some(if rtl { "left" } else { "right" }),
        "both" | "distribute" | "thaiDistribute" | "mediumKashida" | "highKashida"
        | "lowKashida" => Some("justify"),
        _ => None,
    }
}

pub struct ParsedJustification {
    pub align: Option<ParagraphAlign>,
    pub justification: Option<String>,
}

pub fn parse_justification(p_pr: Option<&Element>) -> ParsedJustification {
    let jc = get_w(first_w(p_pr, "jc"), "val");
    let justification = jc.filter(|v| is_st_jc(v));
    if justification.is_none() {
        return ParsedJustification {
            align: None,
            justification: None,
        };
    }
    let justification = justification.unwrap();
    let rtl = on_off_element(first_w(p_pr, "bidi")) == Some(true);
    let align = align_from_justification(&justification, rtl);
    ParsedJustification {
        justification: Some(justification.to_owned()),
        align,
    }
}

/// `w:jc` value to write (`paragraphJustification` upstream).
pub fn paragraph_justification_for_write(block: &serde_json::Value) -> Option<String> {
    let align = block.get("align").and_then(|v| v.as_str());
    let justification = block.get("justification").and_then(|v| v.as_str());
    let rtl = block.get("direction").and_then(|v| v.as_str()) == Some("rtl");
    if let Some(jc) = justification {
        if align_from_justification(jc, rtl) == align {
            return Some(jc.to_owned());
        }
    }
    match align {
        None => None,
        Some("justify") => Some("both".to_owned()),
        Some(other) => Some(other.to_owned()),
    }
}

fn is_st_jc(value: &str) -> bool {
    matches!(
        value,
        "left"
            | "center"
            | "right"
            | "both"
            | "distribute"
            | "thaiDistribute"
            | "start"
            | "end"
            | "mediumKashida"
            | "highKashida"
            | "lowKashida"
            | "numTab"
    )
}
