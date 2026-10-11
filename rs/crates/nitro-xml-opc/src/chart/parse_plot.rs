// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/parse-plot.ts; changes listed in PROVENANCE.md.

use std::collections::{HashMap, HashSet};

use super::dom::attr;
use crate::xml::Element;

use super::dom::{c_child, c_children, chart_elements};
use super::model::{ChartAxis, ChartAxisScaling, ChartGroupKind, ChartPlotArea, ChartPlotGroup};
use super::parse_chrome::{parse_data_table, parse_display_units};
use super::parse_series::parse_series;
use super::parse_text::{parse_data_labels, parse_title};
use super::parse_util::{
    c_bool, c_number, c_val, chart_lines, extension_list_of, manual_layout, number_format,
    report_unmodelled, shape_properties, text_properties, ChartParseContext,
};

fn chart_group_elements() -> HashMap<&'static str, (ChartGroupKind, bool)> {
    HashMap::from([
        ("barChart", (ChartGroupKind::Bar, false)),
        ("bar3DChart", (ChartGroupKind::Bar, true)),
        ("lineChart", (ChartGroupKind::Line, false)),
        ("line3DChart", (ChartGroupKind::Line, true)),
        ("areaChart", (ChartGroupKind::Area, false)),
        ("area3DChart", (ChartGroupKind::Area, true)),
        ("pieChart", (ChartGroupKind::Pie, false)),
        ("pie3DChart", (ChartGroupKind::Pie, true)),
        ("ofPieChart", (ChartGroupKind::OfPie, false)),
        ("doughnutChart", (ChartGroupKind::Doughnut, false)),
        ("scatterChart", (ChartGroupKind::Scatter, false)),
        ("radarChart", (ChartGroupKind::Radar, false)),
        ("bubbleChart", (ChartGroupKind::Bubble, false)),
        ("stockChart", (ChartGroupKind::Stock, false)),
        ("surfaceChart", (ChartGroupKind::Surface, false)),
        ("surface3DChart", (ChartGroupKind::Surface, true)),
    ])
}

fn parse_group(context: &mut ChartParseContext, element: &Element) -> ChartPlotGroup {
    let groups = chart_group_elements();
    let (kind, is3_d) = groups
        .get(element.local.as_str())
        .copied()
        .unwrap_or((ChartGroupKind::Bar, false));
    let handled: HashSet<&str> = [
        "ser",
        "axId",
        "varyColors",
        "dLbls",
        "barDir",
        "grouping",
        "gapWidth",
        "overlap",
        "gapDepth",
        "shape",
        "marker",
        "smooth",
        "dropLines",
        "hiLowLines",
        "upDownBars",
        "serLines",
        "firstSliceAng",
        "holeSize",
        "ofPieType",
        "scatterStyle",
        "radarStyle",
        "bubble3D",
        "bubbleScale",
        "showNegBubbles",
        "sizeRepresents",
        "wireframe",
        "extLst",
    ]
    .into_iter()
    .collect();
    report_unmodelled(context, element, &handled);
    let bars = c_child(Some(element), "upDownBars");
    let bar_dir = c_val(Some(element), "barDir");
    let bar_direction = if bar_dir.as_deref() == Some("bar") || bar_dir.as_deref() == Some("col") {
        bar_dir
    } else {
        None
    };
    let present = |local: &str| {
        if c_child(Some(element), local).is_some() {
            Some(true)
        } else {
            None
        }
    };
    ChartPlotGroup {
        kind,
        element: element.local.clone(),
        is3_d,
        series: c_children(element, "ser")
            .into_iter()
            .map(|ser| parse_series(context, ser))
            .collect(),
        axis_ids: c_children(element, "axId")
            .into_iter()
            .filter_map(|axis| {
                let raw = attr(axis, "val")?.trim();
                raw.parse::<f64>()
                    .ok()
                    .filter(|id| id.is_finite())
                    .map(|id| id as i64)
            })
            .collect(),
        vary_colors: c_bool(context, Some(element), "varyColors"),
        data_labels: parse_data_labels(context, c_child(Some(element), "dLbls")),
        bar_direction,
        grouping: c_val(Some(element), "grouping"),
        gap_width: c_number(context, Some(element), "gapWidth"),
        overlap: c_number(context, Some(element), "overlap"),
        gap_depth: c_number(context, Some(element), "gapDepth"),
        shape: c_val(Some(element), "shape"),
        marker: c_bool(context, Some(element), "marker"),
        smooth: c_bool(context, Some(element), "smooth"),
        drop_lines: present("dropLines"),
        hi_low_lines: present("hiLowLines"),
        up_down_bars: present("upDownBars"),
        series_lines: present("serLines"),
        drop_lines_sp_pr: chart_lines(context, Some(element), "dropLines").and_then(|l| l.sp_pr),
        hi_low_lines_sp_pr: chart_lines(context, Some(element), "hiLowLines").and_then(|l| l.sp_pr),
        series_lines_sp_pr: chart_lines(context, Some(element), "serLines").and_then(|l| l.sp_pr),
        up_down_bars_gap_width: c_number(context, bars, "gapWidth"),
        up_bars: chart_lines(context, bars, "upBars"),
        down_bars: chart_lines(context, bars, "downBars"),
        first_slice_angle: c_number(context, Some(element), "firstSliceAng"),
        hole_size: c_number(context, Some(element), "holeSize"),
        of_pie_type: c_val(Some(element), "ofPieType"),
        scatter_style: c_val(Some(element), "scatterStyle"),
        radar_style: c_val(Some(element), "radarStyle"),
        bubble3_d: c_bool(context, Some(element), "bubble3D"),
        bubble_scale: c_number(context, Some(element), "bubbleScale"),
        show_negative_bubbles: c_bool(context, Some(element), "showNegBubbles"),
        size_represents: c_val(Some(element), "sizeRepresents"),
        wireframe: c_bool(context, Some(element), "wireframe"),
        ext_lst: extension_list_of(context, Some(element)),
    }
}

fn parse_axis(context: &mut ChartParseContext, element: &Element, kind: &str) -> ChartAxis {
    let handled: HashSet<&str> = [
        "axId",
        "scaling",
        "delete",
        "axPos",
        "majorGridlines",
        "minorGridlines",
        "title",
        "numFmt",
        "majorTickMark",
        "minorTickMark",
        "tickLblPos",
        "spPr",
        "txPr",
        "crossAx",
        "crosses",
        "crossesAt",
        "crossBetween",
        "majorUnit",
        "minorUnit",
        "auto",
        "lblAlgn",
        "lblOffset",
        "tickLblSkip",
        "tickMarkSkip",
        "noMultiLvlLbl",
        "baseTimeUnit",
        "dispUnits",
        "extLst",
    ]
    .into_iter()
    .collect();
    report_unmodelled(context, element, &handled);
    let scaling_element = c_child(Some(element), "scaling");
    if let Some(scaling_element) = scaling_element {
        let scaling_handled: HashSet<&str> = ["orientation", "logBase", "min", "max", "extLst"]
            .into_iter()
            .collect();
        report_unmodelled(context, scaling_element, &scaling_handled);
    }
    let scaling = ChartAxisScaling {
        orientation: c_val(scaling_element, "orientation"),
        min: c_number(context, scaling_element, "min"),
        max: c_number(context, scaling_element, "max"),
        log_base: c_number(context, scaling_element, "logBase"),
    };
    let title = c_child(Some(element), "title");
    ChartAxis {
        kind: kind.to_string(),
        scaling,
        major_gridlines: c_child(Some(element), "majorGridlines").is_some(),
        minor_gridlines: c_child(Some(element), "minorGridlines").is_some(),
        major_gridlines_sp_pr: chart_lines(context, Some(element), "majorGridlines")
            .and_then(|l| l.sp_pr),
        minor_gridlines_sp_pr: chart_lines(context, Some(element), "minorGridlines")
            .and_then(|l| l.sp_pr),
        id: c_number(context, Some(element), "axId"),
        cross_axis_id: c_number(context, Some(element), "crossAx"),
        position: c_val(Some(element), "axPos"),
        deleted: c_bool(context, Some(element), "delete"),
        title: title.map(|t| parse_title(context, t)),
        number_format: number_format(Some(element)),
        major_tick_mark: c_val(Some(element), "majorTickMark"),
        minor_tick_mark: c_val(Some(element), "minorTickMark"),
        tick_label_position: c_val(Some(element), "tickLblPos"),
        crosses: c_val(Some(element), "crosses"),
        crosses_at: c_number(context, Some(element), "crossesAt"),
        cross_between: c_val(Some(element), "crossBetween"),
        major_unit: c_number(context, Some(element), "majorUnit"),
        minor_unit: c_number(context, Some(element), "minorUnit"),
        auto: c_bool(context, Some(element), "auto"),
        label_align: c_val(Some(element), "lblAlgn"),
        label_offset: c_number(context, Some(element), "lblOffset"),
        tick_label_skip: c_number(context, Some(element), "tickLblSkip"),
        tick_mark_skip: c_number(context, Some(element), "tickMarkSkip"),
        no_multi_level_labels: c_bool(context, Some(element), "noMultiLvlLbl"),
        base_time_unit: c_val(Some(element), "baseTimeUnit"),
        display_units: parse_display_units(context, c_child(Some(element), "dispUnits")),
        sp_pr: shape_properties(context, Some(element)),
        tx_pr: text_properties(Some(element)),
        ext_lst: extension_list_of(context, Some(element)),
    }
}

pub fn parse_plot_area(
    context: &mut ChartParseContext,
    element: Option<&Element>,
) -> ChartPlotArea {
    let mut plot_area = ChartPlotArea {
        layout: None,
        groups: Vec::new(),
        axes: Vec::new(),
        data_table: None,
        sp_pr: None,
        ext_lst: None,
    };
    let element = match element {
        Some(e) => e,
        None => {
            context.issues.push(super::model::ChartParseIssue {
                code: "CHART_PLOT_AREA_MISSING".into(),
                message: "The chart has no c:plotArea.".into(),
            });
            return plot_area;
        }
    };
    let groups = chart_group_elements();
    let axis_elements: HashMap<&str, &str> = HashMap::from([
        ("catAx", "cat"),
        ("valAx", "val"),
        ("dateAx", "date"),
        ("serAx", "ser"),
    ]);
    let mut handled: HashSet<&str> = ["layout", "dTable", "spPr", "extLst"].into_iter().collect();
    for child in chart_elements(element) {
        if groups.contains_key(child.local.as_str()) {
            plot_area.groups.push(parse_group(context, child));
            handled.insert(child.local.as_str());
        }
        if let Some(kind) = axis_elements.get(child.local.as_str()) {
            plot_area.axes.push(parse_axis(context, child, kind));
            handled.insert(child.local.as_str());
        }
    }
    report_unmodelled(context, element, &handled);
    plot_area.layout = manual_layout(Some(element));
    plot_area.data_table = parse_data_table(context, c_child(Some(element), "dTable"));
    plot_area.sp_pr = shape_properties(context, Some(element));
    plot_area.ext_lst = extension_list_of(context, Some(element));
    plot_area
}
