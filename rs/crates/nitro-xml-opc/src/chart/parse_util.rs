// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/parse-util.ts; changes listed in PROVENANCE.md.

use super::dom::{attr, c_child, chart_elements, first};
use super::manual_layout::read_chart_manual_layout_values;
use super::model::{
    ChartLayout, ChartLines, ChartManualLayout, ChartNumberFormat, ChartParseIssue,
    ChartShapeProperties, ChartTextBody,
};
use super::xml_serialize::{build_xml, extension_list, inner_xml};
use crate::drawingml::{parse_drawing_fill, parse_drawing_line, parse_drawing_text_body};
use crate::xml::{ns, Element};
use std::collections::HashMap;

pub struct ChartParseContext {
    pub issues: Vec<ChartParseIssue>,
    pub source_xml: String,
    /// Namespace URI → preferred prefix from the part root (for `extLst` / extension XML).
    pub uri_prefixes: HashMap<String, String>,
}

pub fn c_val(parent: Option<&Element>, local: &str) -> Option<String> {
    c_child(parent, local)
        .and_then(|e| attr(e, "val"))
        .map(|s| s.to_string())
}

fn path(parent: &Element, element: &Element) -> String {
    format!("c:{}/c:{}", parent.local, element.local)
}

fn malformed(context: &mut ChartParseContext, parent: &Element, element: &Element, value: &str) {
    context.issues.push(ChartParseIssue {
        code: "CHART_VALUE_INVALID".into(),
        message: format!(
            "{} has an unreadable value \"{}\".",
            path(parent, element),
            value
        ),
    });
}

pub fn c_bool(
    context: &mut ChartParseContext,
    parent: Option<&Element>,
    local: &str,
) -> Option<bool> {
    let element = c_child(parent, local)?;
    let raw = attr(element, "val");
    if raw.is_none() {
        return Some(true);
    }
    let value = raw.unwrap().trim().to_ascii_lowercase();
    if value == "1" || value == "true" {
        return Some(true);
    }
    if value == "0" || value == "false" {
        return Some(false);
    }
    malformed(context, parent.unwrap_or(element), element, raw.unwrap());
    None
}

pub fn c_number_i64(
    context: &mut ChartParseContext,
    parent: Option<&Element>,
    local: &str,
) -> Option<i64> {
    c_number(context, parent, local).and_then(|n| {
        if n.fract() == 0.0 && n >= 0.0 {
            Some(n as i64)
        } else {
            None
        }
    })
}

pub fn c_number(
    context: &mut ChartParseContext,
    parent: Option<&Element>,
    local: &str,
) -> Option<f64> {
    let element = c_child(parent, local)?;
    let raw = attr(element, "val")?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        malformed(context, parent.unwrap_or(element), element, raw);
        return None;
    }
    let value = trimmed.parse::<f64>();
    match value {
        Ok(v) if v.is_finite() => Some(v),
        _ => {
            malformed(context, parent.unwrap_or(element), element, raw);
            None
        }
    }
}

pub fn report_unmodelled(
    context: &mut ChartParseContext,
    element: &Element,
    handled: &std::collections::HashSet<&str>,
) {
    for child in chart_elements(element) {
        if handled.contains(child.local.as_str()) {
            continue;
        }
        context.issues.push(ChartParseIssue {
            code: "CHART_ELEMENT_NOT_MODELLED".into(),
            message: format!(
                "c:{}/c:{} is not modelled; it is kept only in the source part.",
                element.local, child.local
            ),
        });
    }
}

pub fn read_shape_fields(_context: &ChartParseContext, sp_pr: &Element) -> ChartShapeProperties {
    let mut out = ChartShapeProperties {
        fill: None,
        line: None,
        effects_xml: None,
        source_xml: None,
    };
    out.fill = parse_drawing_fill(Some(sp_pr));
    out.line = parse_drawing_line(first(sp_pr, "ln", ns::A));
    if let Some(effects) = first(sp_pr, "effectLst", ns::A) {
        out.effects_xml = Some(build_xml(effects));
    }
    out
}

pub fn shape_properties(
    context: &ChartParseContext,
    parent: Option<&Element>,
) -> Option<ChartShapeProperties> {
    let sp_pr = c_child(parent, "spPr")?;
    let mut out = read_shape_fields(context, sp_pr);
    let source = inner_xml(sp_pr);
    if !source.is_empty() {
        out.source_xml = Some(source);
    }
    Some(out)
}

pub fn chart_text_body(element: Option<&Element>) -> Option<ChartTextBody> {
    let element = element?;
    let body = parse_drawing_text_body(Some(element))?;
    let source = inner_xml(element);
    Some(ChartTextBody {
        body,
        source_xml: if source.is_empty() {
            None
        } else {
            Some(source)
        },
    })
}

pub fn text_properties(parent: Option<&Element>) -> Option<ChartTextBody> {
    chart_text_body(c_child(parent, "txPr"))
}

pub fn chart_lines(
    context: &ChartParseContext,
    parent: Option<&Element>,
    local: &str,
) -> Option<ChartLines> {
    let element = c_child(parent, local)?;
    let sp_pr = shape_properties(context, Some(element));
    Some(ChartLines { sp_pr })
}

pub fn number_format(parent: Option<&Element>) -> Option<ChartNumberFormat> {
    let element = c_child(parent, "numFmt")?;
    let format_code = attr(element, "formatCode")?;
    let linked = attr(element, "sourceLinked");
    let source_linked = match linked {
        Some("1") | Some("true") => Some(true),
        Some("0") | Some("false") => Some(false),
        _ => None,
    };
    Some(ChartNumberFormat {
        format_code: format_code.to_string(),
        source_linked,
    })
}

pub fn manual_layout(parent: Option<&Element>) -> Option<ChartLayout> {
    let layout = c_child(parent, "layout")?;
    let manual_el = c_child(Some(layout), "manualLayout");
    let manual = manual_el
        .and_then(|m| read_chart_manual_layout_values(|name| c_val(Some(m), name)))
        .unwrap_or(ChartManualLayout {
            layout_target: None,
            x_mode: None,
            y_mode: None,
            width_mode: None,
            height_mode: None,
            x: None,
            y: None,
            width: None,
            height: None,
        });
    let source = inner_xml(layout);
    Some(ChartLayout {
        manual,
        source_xml: if source.is_empty() {
            None
        } else {
            Some(source)
        },
    })
}

pub fn extension_list_of(context: &ChartParseContext, parent: Option<&Element>) -> Option<String> {
    extension_list(parent, &context.uri_prefixes)
}
