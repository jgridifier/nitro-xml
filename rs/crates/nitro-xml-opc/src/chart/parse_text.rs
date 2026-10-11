// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/parse-text.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use crate::xml::Element;

use super::data_cache::chart_cache_values;
use super::dom::attr;
use super::dom::{c_child, c_children};
use super::model::{
    ChartCachePoint, ChartDataCache, ChartDataLabel, ChartDataLabelOptions, ChartDataLabels,
    ChartDataSource, ChartText, ChartTitle,
};
use super::parse_util::{
    c_bool, c_val, chart_lines, chart_text_body, extension_list_of, manual_layout, number_format,
    report_unmodelled, shape_properties, text_properties, ChartParseContext,
};

fn parse_cache(node: &Element) -> ChartDataCache {
    let cache_type = if node.local == "numCache" || node.local == "numLit" {
        "number"
    } else {
        "string"
    };
    let points = c_children(node, "pt")
        .into_iter()
        .map(|pt| {
            let mut point = ChartCachePoint {
                index: None,
                value: c_child(Some(pt), "v")
                    .map(|e| e.text_content())
                    .unwrap_or_default(),
                format_code: None,
            };
            if let Some(raw) = attr(pt, "idx") {
                if let Ok(index) = raw.parse::<i64>() {
                    if index >= 0 {
                        point.index = Some(index);
                    }
                }
            }
            if let Some(fc) = attr(pt, "formatCode") {
                point.format_code = Some(fc.to_string());
            }
            point
        })
        .collect();
    let mut cache = ChartDataCache {
        cache_type: cache_type.to_string(),
        point_count: None,
        format_code: None,
        points,
    };
    if let Some(count) = c_val(Some(node), "ptCount") {
        if let Ok(c) = count.parse::<i64>() {
            if c >= 0 {
                cache.point_count = Some(c);
            }
        }
    }
    if let Some(fc) = c_child(Some(node), "formatCode") {
        let t = fc.text_content();
        if !t.is_empty() {
            cache.format_code = Some(t);
        }
    }
    cache
}

pub fn parse_data_source(element: Option<&Element>) -> Option<ChartDataSource> {
    let element = element?;
    let mut source = ChartDataSource {
        kind: None,
        formula: None,
        cache: None,
        levels: None,
        level_point_count: None,
    };
    let ref_el = c_child(Some(element), "numRef")
        .or_else(|| c_child(Some(element), "strRef"))
        .or_else(|| c_child(Some(element), "multiLvlStrRef"));
    if let Some(ref_el) = ref_el {
        source.kind = Some(ref_el.local.clone());
        if let Some(f) = c_child(Some(ref_el), "f") {
            let t = f.text_content();
            if !t.is_empty() {
                source.formula = Some(t);
            }
        }
        let cache = c_child(Some(ref_el), "numCache").or_else(|| c_child(Some(ref_el), "strCache"));
        if let Some(cache) = cache {
            source.cache = Some(parse_cache(cache));
        }
        if let Some(levels) = c_child(Some(ref_el), "multiLvlStrCache") {
            source.levels = Some(
                c_children(levels, "lvl")
                    .into_iter()
                    .map(parse_cache)
                    .collect(),
            );
            if let Some(count) = c_val(Some(levels), "ptCount") {
                if let Ok(c) = count.parse::<i64>() {
                    if c >= 0 {
                        source.level_point_count = Some(c);
                    }
                }
            }
        }
        return Some(source);
    }
    let literal = c_child(Some(element), "numLit").or_else(|| c_child(Some(element), "strLit"));
    if let Some(literal) = literal {
        source.kind = Some(literal.local.clone());
        source.cache = Some(parse_cache(literal));
        return Some(source);
    }
    Some(source)
}

pub fn parse_chart_text(
    context: &mut ChartParseContext,
    tx: Option<&Element>,
    allow_value: bool,
) -> Option<ChartText> {
    let tx = tx?;
    let mut text = ChartText {
        rich: None,
        reference: None,
        value: None,
        text: None,
    };
    text.rich = chart_text_body(c_child(Some(tx), "rich"));
    if c_child(Some(tx), "strRef").is_some() {
        text.reference = parse_data_source(Some(tx));
    }
    if allow_value {
        if let Some(v) = c_child(Some(tx), "v") {
            text.value = Some(v.text_content());
        }
    }
    let cached = chart_cache_values(text.reference.as_ref().and_then(|r| r.cache.as_ref()));
    let flat = text
        .value
        .clone()
        .or_else(|| text.rich.as_ref().map(|r| r.body.text.clone()))
        .or_else(|| {
            cached.first().map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => String::new(),
            })
        });
    text.text = flat;
    let handled: HashSet<&str> = if allow_value {
        ["strRef", "v"].into_iter().collect()
    } else {
        ["rich", "strRef"].into_iter().collect()
    };
    report_unmodelled(context, tx, &handled);
    Some(text)
}

pub fn parse_title(context: &mut ChartParseContext, element: &Element) -> ChartTitle {
    let tx = parse_chart_text(context, c_child(Some(element), "tx"), false);
    let handled: HashSet<&str> = ["tx", "layout", "overlay", "spPr", "txPr", "extLst"]
        .into_iter()
        .collect();
    report_unmodelled(context, element, &handled);
    ChartTitle {
        tx: tx.clone(),
        text: tx.as_ref().and_then(|t| t.text.clone()),
        overlay: c_bool(context, Some(element), "overlay"),
        layout: manual_layout(Some(element)),
        sp_pr: shape_properties(context, Some(element)),
        tx_pr: text_properties(Some(element)),
        ext_lst: extension_list_of(context, Some(element)),
    }
}

fn label_options(context: &mut ChartParseContext, element: &Element) -> ChartDataLabelOptions {
    ChartDataLabelOptions {
        deleted: c_bool(context, Some(element), "delete"),
        show_legend_key: c_bool(context, Some(element), "showLegendKey"),
        show_value: c_bool(context, Some(element), "showVal"),
        show_category_name: c_bool(context, Some(element), "showCatName"),
        show_series_name: c_bool(context, Some(element), "showSerName"),
        show_percent: c_bool(context, Some(element), "showPercent"),
        show_bubble_size: c_bool(context, Some(element), "showBubbleSize"),
        separator: c_child(Some(element), "separator").map(|e| e.text_content()),
        position: c_val(Some(element), "dLblPos"),
        number_format: number_format(Some(element)),
        sp_pr: shape_properties(context, Some(element)),
        tx_pr: text_properties(Some(element)),
        ext_lst: extension_list_of(context, Some(element)),
    }
}

pub fn parse_data_labels(
    context: &mut ChartParseContext,
    element: Option<&Element>,
) -> Option<ChartDataLabels> {
    let element = element?;
    let label_handled: HashSet<&str> = [
        "delete",
        "showLegendKey",
        "showVal",
        "showCatName",
        "showSerName",
        "showPercent",
        "showBubbleSize",
        "separator",
        "dLblPos",
        "numFmt",
        "spPr",
        "txPr",
        "extLst",
        "idx",
        "layout",
        "tx",
    ]
    .into_iter()
    .collect();
    let labels = c_children(element, "dLbl")
        .into_iter()
        .map(|node| {
            report_unmodelled(context, node, &label_handled);
            let opts = label_options(context, node);
            ChartDataLabel {
                options: opts,
                index: super::parse_util::c_number_i64(context, Some(node), "idx"),
                tx: parse_chart_text(context, c_child(Some(node), "tx"), false),
                layout: manual_layout(Some(node)),
            }
        })
        .collect();
    let labels_handled: HashSet<&str> = [
        "delete",
        "showLegendKey",
        "showVal",
        "showCatName",
        "showSerName",
        "showPercent",
        "showBubbleSize",
        "separator",
        "dLblPos",
        "numFmt",
        "spPr",
        "txPr",
        "extLst",
        "dLbl",
        "showLeaderLines",
        "leaderLines",
    ]
    .into_iter()
    .collect();
    report_unmodelled(context, element, &labels_handled);
    let opts = label_options(context, element);
    Some(ChartDataLabels {
        options: opts,
        labels,
        show_leader_lines: c_bool(context, Some(element), "showLeaderLines"),
        leader_lines: chart_lines(context, Some(element), "leaderLines"),
    })
}
