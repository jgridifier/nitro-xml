// SPDX-License-Identifier: MIT
//! Minimal `[Content_Types].xml` serializer for dirty package writes (#65).

use crate::opc::ContentTypes;

const NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

/// Serialize content types for a rewritten package (deterministic key order).
pub fn build_content_types_xml(types: &ContentTypes) -> String {
    let mut out = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#);
    out.push_str(&format!(r#"<Types xmlns="{NS}">"#));
    for (ext, ct) in &types.defaults {
        out.push_str(&format!(
            r#"<Default Extension="{}" ContentType="{}"/>"#,
            xml_escape(ext),
            xml_escape(ct)
        ));
    }
    for (part, ct) in &types.overrides {
        out.push_str(&format!(
            r#"<Override PartName="{}" ContentType="{}"/>"#,
            xml_escape(part),
            xml_escape(ct)
        ));
    }
    out.push_str("</Types>");
    out
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    // Catches: content-types rewrite scrambling Default/Override order across saves.
    #[test]
    fn content_types_xml_is_sorted_and_stable() {
        let types = ContentTypes {
            defaults: BTreeMap::from([
                (
                    "rels".into(),
                    "application/vnd.openxmlformats-package.relationships+xml".into(),
                ),
                ("xml".into(), "application/xml".into()),
            ]),
            overrides: BTreeMap::from([(
                "/xl/workbook.xml".into(),
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml".into(),
            )]),
        };
        let a = build_content_types_xml(&types);
        let b = build_content_types_xml(&types);
        assert_eq!(a, b);
        assert!(a.find("Extension=\"rels\"").unwrap() < a.find("Extension=\"xml\"").unwrap());
    }
}
