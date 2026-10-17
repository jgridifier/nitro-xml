// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/conditional.ts;
// changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::{parse_xml_str, Element, Node};

use crate::model::{CfvoThreshold, Color, ConditionalFormat, ConditionalRule, DifferentialStyle};
use crate::sheet_props::parse_sqref;
use crate::style_parts::parse_color;
use crate::xml_util::{att, bool_attr, elements, num_attr, x_children, x_first};

fn formulas_of(rule: &Element) -> Vec<String> {
    x_children(rule, "formula")
        .into_iter()
        .map(|f| f.text_content().trim().to_string())
        .collect()
}

fn cfvo(node: &Element) -> CfvoThreshold {
    let kind = att(Some(node), "type").unwrap_or("num").to_string();
    let mut threshold = CfvoThreshold {
        kind,
        value: att(Some(node), "val").map(|s| s.to_string()),
        gte: None,
    };
    if bool_attr(Some(node), "gte") == Some(false) {
        threshold.gte = Some(false);
    }
    threshold
}

fn empty_dxf() -> DifferentialStyle {
    DifferentialStyle {
        font: None,
        fill: None,
        border: None,
        num_fmt: None,
    }
}

fn style_from_dxf(rule: &Element, dxfs: &[DifferentialStyle]) -> DifferentialStyle {
    match num_attr(Some(rule), "dxfId") {
        Some(id) if (id as usize) < dxfs.len() => dxfs[id as usize].clone(),
        _ => empty_dxf(),
    }
}

fn colors_of(parent: Option<&Element>, palette: Option<&[String]>) -> Vec<Color> {
    parent
        .map(|p| {
            x_children(p, "color")
                .into_iter()
                .filter_map(|node| parse_color(Some(node), palette))
                .collect()
        })
        .unwrap_or_default()
}

fn read_rule(
    rule: &Element,
    dxfs: &[DifferentialStyle],
    palette: Option<&[String]>,
) -> Option<ConditionalRule> {
    let kind = att(Some(rule), "type").unwrap_or("");
    let priority = num_attr(Some(rule), "priority").unwrap_or(1.0) as i32;
    let stop_if_true = bool_attr(Some(rule), "stopIfTrue") == Some(true);
    let style = style_from_dxf(rule, dxfs);
    match kind {
        "cellIs" => Some(ConditionalRule::CellIs {
            operator: att(Some(rule), "operator").unwrap_or("equal").to_string(),
            formulas: formulas_of(rule),
            style,
            priority,
            stop_if_true,
        }),
        "expression" => Some(ConditionalRule::Expression {
            formula: formulas_of(rule).into_iter().next().unwrap_or_default(),
            style,
            priority,
            stop_if_true,
        }),
        "colorScale" => {
            let scale = x_first(Some(rule), "colorScale");
            Some(ConditionalRule::ColorScale {
                thresholds: scale
                    .map(|s| x_children(s, "cfvo").into_iter().map(cfvo).collect())
                    .unwrap_or_default(),
                colors: colors_of(scale, palette),
                priority,
                stop_if_true,
            })
        }
        "dataBar" => {
            let bar = x_first(Some(rule), "dataBar");
            let mut thresholds = bar
                .map(|b| {
                    x_children(b, "cfvo")
                        .into_iter()
                        .map(cfvo)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let min = thresholds.first().cloned().unwrap_or(CfvoThreshold {
                kind: "min".into(),
                value: None,
                gte: None,
            });
            let max = thresholds.get(1).cloned().unwrap_or(CfvoThreshold {
                kind: "max".into(),
                value: None,
                gte: None,
            });
            let _ = &mut thresholds;
            let color = colors_of(bar, palette).into_iter().next().unwrap_or(Color {
                rgb: Some("FF638EC6".into()),
                theme: None,
                tint: None,
                indexed: None,
                auto: None,
            });
            // Classic `minLength`/`maxLength` on the 2006 dataBar; else Excel defaults 10/90.
            let min_length = num_attr(bar, "minLength").unwrap_or(10.0);
            let max_length = num_attr(bar, "maxLength").unwrap_or(90.0);
            let extension_id = x_first(Some(rule), "extLst").and_then(|lst| {
                // `<x14:id>{GUID}</x14:id>` under the B025F937 ext uri.
                fn find_id(el: &Element) -> Option<String> {
                    if el.local == "id" {
                        let t = el.text_content().trim().to_string();
                        if !t.is_empty() {
                            return Some(t);
                        }
                    }
                    for child in &el.children {
                        if let Node::Element(e) = child {
                            if let Some(id) = find_id(e) {
                                return Some(id);
                            }
                        }
                    }
                    None
                }
                find_id(lst)
            });
            Some(ConditionalRule::DataBar {
                min,
                max,
                color,
                priority,
                stop_if_true,
                show_value: bool_attr(bar, "showValue") != Some(false),
                min_length,
                max_length,
                border: false,
                auto_min: false,
                auto_max: false,
                extension_id,
            })
        }
        "iconSet" => {
            let set = x_first(Some(rule), "iconSet");
            Some(ConditionalRule::IconSet {
                icon_set: att(set, "iconSet").unwrap_or("3TrafficLights1").to_string(),
                thresholds: set
                    .map(|s| x_children(s, "cfvo").into_iter().map(cfvo).collect())
                    .unwrap_or_default(),
                priority,
                stop_if_true,
                reverse: bool_attr(set, "reverse") == Some(true),
                show_value: bool_attr(set, "showValue") != Some(false),
            })
        }
        "containsText" | "notContainsText" | "beginsWith" | "endsWith" => {
            Some(ConditionalRule::Text {
                kind: kind.to_string(),
                text: att(Some(rule), "text").unwrap_or("").to_string(),
                style,
                priority,
                stop_if_true,
            })
        }
        "top10" => Some(ConditionalRule::Top10 {
            rank: num_attr(Some(rule), "rank").unwrap_or(10.0),
            percent: bool_attr(Some(rule), "percent") == Some(true),
            bottom: bool_attr(Some(rule), "bottom") == Some(true),
            style,
            priority,
            stop_if_true,
        }),
        "aboveAverage" => Some(ConditionalRule::AboveAverage {
            below: bool_attr(Some(rule), "aboveAverage") == Some(false),
            equal_average: bool_attr(Some(rule), "equalAverage") == Some(true),
            style,
            priority,
            stop_if_true,
        }),
        "duplicateValues" => Some(ConditionalRule::UniqueDup {
            duplicate: true,
            style,
            priority,
            stop_if_true,
        }),
        "uniqueValues" => Some(ConditionalRule::UniqueDup {
            duplicate: false,
            style,
            priority,
            stop_if_true,
        }),
        other if !other.is_empty() => Some(ConditionalRule::Other {
            kind: other.to_string(),
            priority,
            stop_if_true,
            style,
        }),
        _ => None,
    }
}

/// Reads every `conditionalFormatting` block.
pub fn read_conditional_formats(
    blocks: &[&Element],
    dxfs: &[DifferentialStyle],
    palette: Option<&[String]>,
    warn: &mut dyn FnMut(String),
) -> Vec<ConditionalFormat> {
    let mut out = Vec::new();
    for block in blocks {
        let ranges = parse_sqref(att(Some(block), "sqref").unwrap_or(""));
        let mut rules = Vec::new();
        for node in x_children(block, "cfRule") {
            if let Some(rule) = read_rule(node, dxfs, palette) {
                rules.push(rule);
            } else {
                warn(format!(
                    "Conditional formatting rule type \"{}\" is not supported and was dropped.",
                    att(Some(node), "type").unwrap_or("")
                ));
            }
        }
        if !ranges.is_empty() && !rules.is_empty() {
            out.push(ConditionalFormat { ranges, rules });
        }
    }
    out
}

/// Apply linked x14 dataBar settings (`minLength`/`maxLength`/`border`/`autoMin`/`autoMax`).
///
/// Port of upstream `readConditionalExtensions` for the fields the paint path needs.
pub fn link_data_bar_extensions(formats: &mut [ConditionalFormat], ext_lst: &[String]) {
    if ext_lst.is_empty() {
        return;
    }
    let mut by_id: std::collections::HashMap<String, (f64, f64, bool, bool, bool)> =
        std::collections::HashMap::new();
    for xml in ext_lst {
        // extLst fragments are not always a single rooted document; wrap for the parser.
        let wrapped = if xml.trim_start().starts_with("<?xml") {
            xml.clone()
        } else {
            format!(
                r#"<root xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main">{xml}</root>"#
            )
        };
        let Ok(doc) = parse_xml_str(&wrapped) else {
            continue;
        };
        fn walk(
            el: &Element,
            out: &mut std::collections::HashMap<String, (f64, f64, bool, bool, bool)>,
        ) {
            if el.local == "cfRule" && att(Some(el), "type") == Some("dataBar") {
                if let Some(id) = att(Some(el), "id") {
                    let bar = elements(el).find(|c| c.local == "dataBar");
                    let min_length = num_attr(bar, "minLength").unwrap_or(10.0);
                    let max_length = num_attr(bar, "maxLength").unwrap_or(90.0);
                    let border = bool_attr(bar, "border") == Some(true);
                    let mut auto_min = false;
                    let mut auto_max = false;
                    if let Some(bar) = bar {
                        let cfvos: Vec<_> = elements(bar).filter(|c| c.local == "cfvo").collect();
                        if cfvos.first().and_then(|c| att(Some(c), "type")) == Some("autoMin") {
                            auto_min = true;
                        }
                        if cfvos.get(1).and_then(|c| att(Some(c), "type")) == Some("autoMax") {
                            auto_max = true;
                        }
                    }
                    out.insert(
                        id.to_string(),
                        (min_length, max_length, border, auto_min, auto_max),
                    );
                }
            }
            for child in elements(el) {
                walk(child, out);
            }
        }
        walk(&doc.root, &mut by_id);
    }
    if by_id.is_empty() {
        return;
    }
    for format in formats.iter_mut() {
        for rule in format.rules.iter_mut() {
            if let ConditionalRule::DataBar {
                min_length,
                max_length,
                border,
                auto_min,
                auto_max,
                extension_id,
                ..
            } = rule
            {
                let Some(id) = extension_id.as_deref() else {
                    continue;
                };
                if let Some((mn, mx, b, amin, amax)) = by_id.get(id) {
                    *min_length = *mn;
                    *max_length = *mx;
                    *border = *b;
                    *auto_min = *amin;
                    *auto_max = *amax;
                }
            }
        }
    }
}
