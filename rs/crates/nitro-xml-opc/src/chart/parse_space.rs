// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/parse-space.ts; changes listed in PROVENANCE.md.

use std::collections::{HashMap, HashSet};

use super::dom::{attr, first};
use crate::xml::{ns, parse_xml_str_with_options, Element, ParseOptions};

use super::dom::{c_child, c_children, first_ns, mc_children, rel_attr};
use super::model::{
    ChartLegend, ChartLegendEntry, ChartSpace, ChartSpaceParseResult, ChartView3D,
    NamespaceDeclaration,
};
use super::parse_chrome::parse_surface;
use super::parse_plot::parse_plot_area;
use super::parse_text::parse_title;
use super::parse_util::{
    c_bool, c_number, c_val, extension_list_of, manual_layout, report_unmodelled, shape_properties,
    text_properties, ChartParseContext,
};
use super::xml_serialize::build_xml;

fn parse_legend(context: &mut ChartParseContext, element: &Element) -> ChartLegend {
    let handled: HashSet<&str> = [
        "legendPos",
        "legendEntry",
        "layout",
        "overlay",
        "spPr",
        "txPr",
        "extLst",
    ]
    .into_iter()
    .collect();
    report_unmodelled(context, element, &handled);
    let entries = c_children(element, "legendEntry")
        .into_iter()
        .map(|entry| ChartLegendEntry {
            index: super::parse_util::c_number_i64(context, Some(entry), "idx"),
            deleted: c_bool(context, Some(entry), "delete"),
            tx_pr: text_properties(Some(entry)),
        })
        .collect();
    ChartLegend {
        position: c_val(Some(element), "legendPos"),
        overlay: c_bool(context, Some(element), "overlay"),
        entries,
        layout: manual_layout(Some(element)),
        sp_pr: shape_properties(context, Some(element)),
        tx_pr: text_properties(Some(element)),
        ext_lst: extension_list_of(context, Some(element)),
    }
}

fn parse_view3d(context: &mut ChartParseContext, element: &Element) -> ChartView3D {
    let handled: HashSet<&str> = [
        "rotX",
        "rotY",
        "depthPercent",
        "hPercent",
        "rAngAx",
        "perspective",
    ]
    .into_iter()
    .collect();
    report_unmodelled(context, element, &handled);
    ChartView3D {
        rot_x: c_number(context, Some(element), "rotX"),
        rot_y: c_number(context, Some(element), "rotY"),
        depth_percent: c_number(context, Some(element), "depthPercent"),
        height_percent: c_number(context, Some(element), "hPercent"),
        right_angle_axes: c_bool(context, Some(element), "rAngAx"),
        perspective: c_number(context, Some(element), "perspective"),
    }
}

fn chart_style(context: &mut ChartParseContext, space: &Element) -> Option<f64> {
    if let Some(direct) = c_number(context, Some(space), "style") {
        return Some(direct);
    }
    for alternate in mc_children(space, "AlternateContent") {
        if let Some(fallback) = first_ns(Some(alternate), "Fallback", ns::MC) {
            if let Some(style) = c_number(context, Some(fallback), "style") {
                return Some(style);
            }
        }
    }
    None
}

fn c14_style(space: &Element) -> Option<f64> {
    for alternate in mc_children(space, "AlternateContent") {
        if let Some(choice) = first_ns(Some(alternate), "Choice", ns::MC) {
            if let Some(style) = first(choice, "style", ns::C14) {
                let raw = attr(style, "val");
                if let Some(raw) = raw {
                    let value = raw.trim().parse::<f64>();
                    if let Ok(v) = value {
                        if v.is_finite() {
                            return Some(v);
                        }
                    }
                }
            }
        }
    }
    None
}

fn uri_prefix_map(source: &str) -> HashMap<String, String> {
    let mut map = HashMap::from([
        (ns::C.to_string(), "c".to_string()),
        (ns::A.to_string(), "a".to_string()),
        (ns::R.to_string(), "r".to_string()),
        (ns::MC.to_string(), "mc".to_string()),
        (ns::C14.to_string(), "c14".to_string()),
        (ns::C16.to_string(), "c16".to_string()),
        (ns::C16R2.to_string(), "c16r2".to_string()),
    ]);
    if let Some(decls) = namespace_declarations_from_source(source) {
        for decl in decls {
            map.insert(decl.uri, decl.prefix);
        }
    }
    map
}

fn namespace_declarations_from_source(source: &str) -> Option<Vec<NamespaceDeclaration>> {
    if source.is_empty() {
        return None;
    }
    let start = source
        .find("<c:chartSpace")
        .or_else(|| source.find("<chartSpace"))?;
    let tag_end = source[start..].find('>')? + start;
    let tag = &source[start..tag_end];
    let bindings: HashSet<(&str, &str)> = HashSet::from([("c", ns::C), ("a", ns::A), ("r", ns::R)]);
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(idx) = tag[pos..].find("xmlns:") {
        let i = pos + idx + 6;
        let rest = &tag[i..];
        let colon = rest.find('=')?;
        let prefix = rest[..colon].trim();
        let after = rest[colon + 1..].trim_start();
        let (uri, consumed) = {
            let stripped = after.strip_prefix('"')?;
            let end = stripped.find('"')? + 1;
            (stripped[..end - 1].to_string(), end + 1)
        };
        pos = i + colon + 1 + consumed;
        if bindings.contains(&(prefix, uri.as_str())) {
            continue;
        }
        out.push(NamespaceDeclaration {
            prefix: prefix.to_string(),
            uri,
        });
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Parses a chart part (`c:chartSpace`) from XML text or a parsed root element.
pub fn parse_chart_space_xml(xml: &str) -> ChartSpaceParseResult {
    let doc = parse_xml_str_with_options(
        xml,
        ParseOptions {
            sort_attributes: false,
        },
    )
    .expect("chart xml parse");
    parse_chart_space_element_with_source(&doc.root, xml)
}

pub fn parse_chart_space_element(root: &Element) -> ChartSpaceParseResult {
    parse_chart_space_element_with_source(root, "")
}

fn parse_chart_space_element_with_source(
    root: &Element,
    source_xml: &str,
) -> ChartSpaceParseResult {
    let mut context = ChartParseContext {
        issues: Vec::new(),
        source_xml: source_xml.to_string(),
        uri_prefixes: uri_prefix_map(source_xml),
    };
    if root.local != "chartSpace" || (root.ns.is_some() && root.ns.as_deref() != Some(ns::C)) {
        context.issues.push(super::model::ChartParseIssue {
            code: "CHART_ROOT_UNEXPECTED".into(),
            message: format!(
                "The part root is {{{}}}{}, not c:chartSpace (chartex parts are not modelled).",
                root.ns.as_deref().unwrap_or(""),
                root.local
            ),
        });
        return ChartSpaceParseResult {
            chart_space: ChartSpace {
                plot_area: super::model::ChartPlotArea {
                    groups: Vec::new(),
                    axes: Vec::new(),
                    layout: None,
                    data_table: None,
                    sp_pr: None,
                    ext_lst: None,
                },
                date1904: None,
                language: None,
                rounded_corners: None,
                style: None,
                c14_style: None,
                title: None,
                auto_title_deleted: None,
                view3_d: None,
                floor: None,
                side_wall: None,
                back_wall: None,
                legend: None,
                plot_visible_only: None,
                display_blanks_as: None,
                show_data_labels_over_max: None,
                sp_pr: None,
                tx_pr: None,
                external_data_rel_id: None,
                external_data_auto_update: None,
                print_settings: None,
                user_shapes_rel_id: None,
                chart_ext_lst: None,
                ext_lst: None,
                namespace_declarations: None,
            },
            issues: context.issues,
        };
    }
    let space_handled: HashSet<&str> = [
        "date1904",
        "lang",
        "roundedCorners",
        "AlternateContent",
        "style",
        "chart",
        "spPr",
        "txPr",
        "externalData",
        "printSettings",
        "userShapes",
        "extLst",
    ]
    .into_iter()
    .collect();
    report_unmodelled(&mut context, root, &space_handled);
    let chart = c_child(Some(root), "chart");
    if chart.is_none() {
        context.issues.push(super::model::ChartParseIssue {
            code: "CHART_CHART_MISSING".into(),
            message: "The part has no c:chart.".into(),
        });
    }
    if let Some(chart_el) = chart {
        let chart_handled: HashSet<&str> = [
            "title",
            "autoTitleDeleted",
            "view3D",
            "floor",
            "sideWall",
            "backWall",
            "plotArea",
            "legend",
            "plotVisOnly",
            "dispBlanksAs",
            "showDLblsOverMax",
            "extLst",
        ]
        .into_iter()
        .collect();
        report_unmodelled(&mut context, chart_el, &chart_handled);
    }
    let title_el = chart.and_then(|c| c_child(Some(c), "title"));
    let legend_el = chart.and_then(|c| c_child(Some(c), "legend"));
    let view3d_el = chart.and_then(|c| c_child(Some(c), "view3D"));
    let print_settings = c_child(Some(root), "printSettings");
    let chart_space = ChartSpace {
        plot_area: parse_plot_area(
            &mut context,
            chart.and_then(|c| c_child(Some(c), "plotArea")),
        ),
        date1904: c_bool(&mut context, Some(root), "date1904"),
        language: c_val(Some(root), "lang"),
        rounded_corners: c_bool(&mut context, Some(root), "roundedCorners"),
        style: chart_style(&mut context, root),
        c14_style: c14_style(root),
        title: title_el.map(|t| parse_title(&mut context, t)),
        auto_title_deleted: c_bool(&mut context, chart, "autoTitleDeleted"),
        view3_d: view3d_el.map(|v| parse_view3d(&mut context, v)),
        floor: parse_surface(&mut context, chart.and_then(|c| c_child(Some(c), "floor"))),
        side_wall: parse_surface(
            &mut context,
            chart.and_then(|c| c_child(Some(c), "sideWall")),
        ),
        back_wall: parse_surface(
            &mut context,
            chart.and_then(|c| c_child(Some(c), "backWall")),
        ),
        legend: legend_el.map(|l| parse_legend(&mut context, l)),
        plot_visible_only: c_bool(&mut context, chart, "plotVisOnly"),
        display_blanks_as: chart.and_then(|c| c_val(Some(c), "dispBlanksAs")),
        show_data_labels_over_max: c_bool(&mut context, chart, "showDLblsOverMax"),
        sp_pr: shape_properties(&context, Some(root)),
        tx_pr: text_properties(Some(root)),
        external_data_rel_id: rel_attr(c_child(Some(root), "externalData"), "id"),
        external_data_auto_update: c_bool(
            &mut context,
            c_child(Some(root), "externalData"),
            "autoUpdate",
        ),
        print_settings: print_settings.map(build_xml),
        user_shapes_rel_id: rel_attr(c_child(Some(root), "userShapes"), "id"),
        chart_ext_lst: chart.and_then(|c| extension_list_of(&context, Some(c))),
        ext_lst: extension_list_of(&context, Some(root)),
        namespace_declarations: namespace_declarations_from_source(&context.source_xml),
    };
    ChartSpaceParseResult {
        chart_space,
        issues: context.issues,
    }
}
