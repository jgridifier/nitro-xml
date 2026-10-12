// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/parse-series.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use crate::xml::Element;

use super::dom::{c_child, c_children};
use super::model::{ChartDataPoint, ChartMarker, ChartPictureOptions, ChartSpaceSeries};
use super::parse_text::{parse_chart_text, parse_data_labels, parse_data_source};
use super::parse_util::{
    c_bool, c_number, c_val, extension_list_of, report_unmodelled, shape_properties,
    ChartParseContext,
};

fn parse_marker(context: &mut ChartParseContext, element: Option<&Element>) -> Option<ChartMarker> {
    let element = element?;
    let handled: HashSet<&str> = ["symbol", "size", "spPr", "extLst"].into_iter().collect();
    report_unmodelled(context, element, &handled);
    Some(ChartMarker {
        symbol: c_val(Some(element), "symbol"),
        size: c_number(context, Some(element), "size"),
        sp_pr: shape_properties(context, Some(element)),
        ext_lst: extension_list_of(context, Some(element)),
    })
}

fn parse_picture_options(
    context: &mut ChartParseContext,
    element: &Element,
) -> Option<ChartPictureOptions> {
    let po = c_child(Some(element), "pictureOptions")?;
    let handled: HashSet<&str> = [
        "applyToFront",
        "applyToSides",
        "applyToEnd",
        "pictureFormat",
        "pictureStackUnit",
        "extLst",
    ]
    .into_iter()
    .collect();
    report_unmodelled(context, po, &handled);
    let format = c_val(Some(po), "pictureFormat");
    let picture_format =
        format.filter(|f| matches!(f.as_str(), "stretch" | "stack" | "stackScale" | "sideways"));
    Some(ChartPictureOptions {
        apply_to_front: c_bool(context, Some(po), "applyToFront"),
        apply_to_sides: c_bool(context, Some(po), "applyToSides"),
        apply_to_end: c_bool(context, Some(po), "applyToEnd"),
        picture_format,
        picture_stack_unit: c_number(context, Some(po), "pictureStackUnit"),
    })
}

fn parse_data_point(context: &mut ChartParseContext, element: &Element) -> ChartDataPoint {
    let handled: HashSet<&str> = [
        "idx",
        "invertIfNegative",
        "marker",
        "bubble3D",
        "explosion",
        "spPr",
        "extLst",
    ]
    .into_iter()
    .collect();
    report_unmodelled(context, element, &handled);
    let index = c_number(context, Some(element), "idx");
    ChartDataPoint {
        index: index
            .filter(|i| i.fract() == 0.0 && *i >= 0.0)
            .map(|i| i as i64),
        invert_if_negative: c_bool(context, Some(element), "invertIfNegative"),
        bubble3_d: c_bool(context, Some(element), "bubble3D"),
        explosion: c_number(context, Some(element), "explosion"),
        marker: parse_marker(context, c_child(Some(element), "marker")),
        sp_pr: shape_properties(context, Some(element)),
        picture_options: parse_picture_options(context, element),
        ext_lst: extension_list_of(context, Some(element)),
    }
}

pub fn parse_series(context: &mut ChartParseContext, element: &Element) -> ChartSpaceSeries {
    let handled: HashSet<&str> = [
        "idx",
        "order",
        "tx",
        "spPr",
        "invertIfNegative",
        "dPt",
        "dLbls",
        "cat",
        "val",
        "xVal",
        "yVal",
        "smooth",
        "marker",
        "explosion",
        "bubbleSize",
        "bubble3D",
        "shape",
        "extLst",
    ]
    .into_iter()
    .collect();
    report_unmodelled(context, element, &handled);
    ChartSpaceSeries {
        index: super::parse_util::c_number_i64(context, Some(element), "idx"),
        order: super::parse_util::c_number_i64(context, Some(element), "order"),
        tx: parse_chart_text(context, c_child(Some(element), "tx"), true),
        sp_pr: shape_properties(context, Some(element)),
        marker: parse_marker(context, c_child(Some(element), "marker")),
        data_labels: parse_data_labels(context, c_child(Some(element), "dLbls")),
        data_points: c_children(element, "dPt")
            .into_iter()
            .map(|p| parse_data_point(context, p))
            .collect(),
        categories: parse_data_source(c_child(Some(element), "cat")),
        values: parse_data_source(c_child(Some(element), "val")),
        x_values: parse_data_source(c_child(Some(element), "xVal")),
        y_values: parse_data_source(c_child(Some(element), "yVal")),
        bubble_sizes: parse_data_source(c_child(Some(element), "bubbleSize")),
        smooth: c_bool(context, Some(element), "smooth"),
        invert_if_negative: c_bool(context, Some(element), "invertIfNegative"),
        explosion: c_number(context, Some(element), "explosion"),
        bubble3_d: c_bool(context, Some(element), "bubble3D"),
        shape: c_val(Some(element), "shape"),
        picture_options: parse_picture_options(context, element),
        ext_lst: extension_list_of(context, Some(element)),
    }
}
