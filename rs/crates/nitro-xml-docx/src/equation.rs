// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/equation.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::Element;

use crate::xml_util::serialize_element;

const MATH_NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/math",
    "http://purl.oclc.org/ooxml/officeDocument/math",
];

pub fn is_equation_element(element: &Element) -> bool {
    let ns = element.ns.as_deref().unwrap_or("");
    MATH_NS.contains(&ns) && (element.local == "oMath" || element.local == "oMathPara")
}

pub fn parse_equation(element: &Element) -> Option<Value> {
    if !is_equation_element(element) {
        return None;
    }
    let display = element.local == "oMathPara";
    Some(json!({
        "text": "",
        "equation": {
            "omml": serialize_element(element),
            "display": display,
        }
    }))
}
