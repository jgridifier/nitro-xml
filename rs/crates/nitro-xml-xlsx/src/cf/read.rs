// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/conditional.ts;
// changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::Element;

use crate::model::{CfvoThreshold, Color, ConditionalFormat, ConditionalRule, DifferentialStyle};
use crate::sheet_props::parse_sqref;
use crate::style_parts::parse_color;
use crate::xml_util::{att, bool_attr, num_attr, x_children, x_first};

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
            Some(ConditionalRule::DataBar {
                min,
                max,
                color,
                priority,
                stop_if_true,
                show_value: bool_attr(bar, "showValue") != Some(false),
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
