// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/restore-run-format.ts and
// restore-paragraph-format.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::parse_xml_str;

use crate::run_properties::parse_direct_run_properties;
use crate::styles::parse_direct_paragraph_formatting;
use crate::xml_util::{named_w, serialize_element};

const DIRECT_RUN_PROPERTY_KEYS: &[&str] = &[
    "bold",
    "italic",
    "boldComplexScript",
    "italicComplexScript",
    "caps",
    "smallCaps",
    "strike",
    "doubleStrike",
    "vanish",
    "underline",
    "underlineStyle",
    "underlineColor",
    "highlight",
    "verticalAlign",
    "fontSize",
    "fontFamily",
    "fontFamilyEastAsia",
    "fontFamilyComplexScript",
    "fontSizeComplexScript",
    "fontTheme",
    "color",
    "colorTheme",
    "characterSpacingTwips",
    "textScalePercent",
    "ligatures",
    "kerningHalfPoints",
    "positionHalfPoints",
    "shadingFill",
    "shadingThemeFill",
    "language",
    "eastAsiaLanguage",
    "bidiLanguage",
    "rtl",
    "style",
];

const PARAGRAPH_FORMAT_KEYS: &[&str] = &[
    "align",
    "justification",
    "direction",
    "style",
    "spacingBeforeTwips",
    "spacingAfterTwips",
    "lineSpacingTwips",
    "lineSpacingRule",
    "indentLeftTwips",
    "indentRightTwips",
    "indentStartTwips",
    "indentEndTwips",
    "firstLineTwips",
    "hangingTwips",
    "numbering",
    "pageBreakBefore",
    "tabStops",
    "keepNext",
    "keepLines",
    "widowControl",
    "contextualSpacing",
    "suppressLineNumbers",
    "outlineLevel",
    "dropCap",
    "borders",
    "shadingFill",
];

fn parse_run_snapshot(xml: &str) -> Result<nitro_xml_opc::xml::Element, String> {
    let doc = parse_xml_str(xml).map_err(|e| e.to_string())?;
    if !named_w(&doc.root, "rPr") {
        return Err("A formatting revision snapshot must contain Word rPr properties.".into());
    }
    Ok(doc.root)
}

fn parse_paragraph_snapshot(xml: &str) -> Result<nitro_xml_opc::xml::Element, String> {
    let doc = parse_xml_str(xml).map_err(|e| e.to_string())?;
    if !named_w(&doc.root, "pPr") {
        return Err("A formatting revision snapshot must contain Word pPr properties.".into());
    }
    Ok(doc.root)
}

/// Restores modeled direct run properties without changing text or comment anchors.
pub fn restore_run_formatting(run: &mut Value) -> Result<(), String> {
    let had_format_revision = run.get("formatRevision").is_some();
    let revision = run
        .get("formatRevision")
        .cloned()
        .or_else(|| run.get("revision").cloned());
    let xml = revision
        .as_ref()
        .and_then(|r| r.get("previousRunPropertiesXml"))
        .and_then(|v| v.as_str());
    let xml = xml.ok_or_else(|| {
        "Cannot reject a formatting revision without its prior run-properties snapshot.".to_string()
    })?;
    let snapshot = parse_run_snapshot(xml)?;
    let previous = parse_direct_run_properties(Some(&snapshot));
    if let Some(obj) = run.as_object_mut() {
        for key in DIRECT_RUN_PROPERTY_KEYS {
            obj.remove(*key);
        }
        if let Some(prev) = previous.as_object() {
            for (k, v) in prev {
                obj.insert(k.clone(), v.clone());
            }
        }
        obj.insert("restoredRunPropertiesXml".into(), json!(xml));
        obj.remove("sourceRunPropertiesXml");
        if had_format_revision {
            obj.remove("formatRevision");
        } else {
            obj.remove("revision");
        }
    }
    Ok(())
}

/// Restore paragraph formatting from a format-revision snapshot.
pub fn restore_paragraph_formatting(paragraph: &mut Value) -> Result<(), String> {
    let xml = paragraph
        .get("formatRevision")
        .and_then(|r| r.get("previousParagraphPropertiesXml"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            "Cannot reject a paragraph formatting revision without its prior properties snapshot."
                .to_string()
        })?;
    let properties = parse_paragraph_snapshot(xml)?;
    let previous = parse_direct_paragraph_formatting(Some(&properties));
    if let Some(obj) = paragraph.as_object_mut() {
        for key in PARAGRAPH_FORMAT_KEYS {
            obj.remove(*key);
        }
        if let Some(prev) = previous.as_object() {
            for (k, v) in prev {
                obj.insert(k.clone(), v.clone());
            }
        }
        obj.insert(
            "restoredParagraphPropertiesXml".into(),
            json!(serialize_element(&properties)),
        );
        if obj.contains_key("sourceParagraphPropertiesXml") {
            obj.insert(
                "sourceParagraphPropertiesXml".into(),
                json!(serialize_element(&properties)),
            );
        }
        obj.remove("formatRevision");
    }
    Ok(())
}
