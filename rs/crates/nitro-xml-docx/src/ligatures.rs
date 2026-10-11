// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/ligatures.ts; changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::{Element, Node};

const WORD_2010_NS: &str = "http://schemas.microsoft.com/office/word/2010/wordml";

const LIGATURE_VALUES: &[&str] = &[
    "none",
    "standard",
    "contextual",
    "historical",
    "discretional",
    "standardContextual",
    "standardHistorical",
    "contextualHistorical",
    "standardDiscretional",
    "contextualDiscretional",
    "historicalDiscretional",
    "standardContextualHistorical",
    "standardContextualDiscretional",
    "standardHistoricalDiscretional",
    "contextualHistoricalDiscretional",
    "all",
];

pub fn parse_ligatures(props: &Element) -> Option<String> {
    for child in props.children.iter() {
        if let Node::Element(el) = child {
            if el.local != "ligatures" || el.ns.as_deref() != Some(WORD_2010_NS) {
                continue;
            }
            let val = el
                .attrs
                .iter()
                .find(|a| a.local == "val")
                .map(|a| a.value.as_str());
            if let Some(v) = val {
                if LIGATURE_VALUES.contains(&v) {
                    return Some(v.to_owned());
                }
            }
        }
    }
    None
}
