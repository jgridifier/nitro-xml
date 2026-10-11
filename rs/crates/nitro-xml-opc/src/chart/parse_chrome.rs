// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/parse-chrome.ts; changes listed in PROVENANCE.md.

use std::collections::HashSet;

use crate::xml::Element;

use super::dom::c_child;
use super::model::{ChartDataTable, ChartDisplayUnits, ChartDisplayUnitsLabel, ChartSurface};
use super::parse_text::parse_chart_text;
use super::parse_util::{
    c_bool, c_number, c_val, extension_list_of, report_unmodelled, shape_properties,
    text_properties, ChartParseContext,
};
use super::xml_serialize::build_xml;

pub fn parse_surface(
    context: &mut ChartParseContext,
    element: Option<&Element>,
) -> Option<ChartSurface> {
    let element = element?;
    let handled: HashSet<&str> = ["thickness", "spPr", "pictureOptions", "extLst"]
        .into_iter()
        .collect();
    report_unmodelled(context, element, &handled);
    let picture = c_child(Some(element), "pictureOptions");
    Some(ChartSurface {
        thickness: c_number(context, Some(element), "thickness"),
        sp_pr: shape_properties(context, Some(element)),
        picture_options_xml: picture.map(build_xml),
        ext_lst: extension_list_of(context, Some(element)),
    })
}

pub fn parse_data_table(
    context: &mut ChartParseContext,
    element: Option<&Element>,
) -> Option<ChartDataTable> {
    let element = element?;
    let handled: HashSet<&str> = [
        "showHorzBorder",
        "showVertBorder",
        "showOutline",
        "showKeys",
        "spPr",
        "txPr",
        "extLst",
    ]
    .into_iter()
    .collect();
    report_unmodelled(context, element, &handled);
    Some(ChartDataTable {
        show_horizontal_border: c_bool(context, Some(element), "showHorzBorder"),
        show_vertical_border: c_bool(context, Some(element), "showVertBorder"),
        show_outline: c_bool(context, Some(element), "showOutline"),
        show_keys: c_bool(context, Some(element), "showKeys"),
        sp_pr: shape_properties(context, Some(element)),
        tx_pr: text_properties(Some(element)),
        ext_lst: extension_list_of(context, Some(element)),
    })
}

pub fn parse_display_units(
    context: &mut ChartParseContext,
    element: Option<&Element>,
) -> Option<ChartDisplayUnits> {
    let element = element?;
    let handled: HashSet<&str> = ["custUnit", "builtInUnit", "dispUnitsLbl", "extLst"]
        .into_iter()
        .collect();
    report_unmodelled(context, element, &handled);
    let label_element = c_child(Some(element), "dispUnitsLbl");
    let label = label_element.map(|label_element| {
        let label_handled: HashSet<&str> = ["layout", "tx", "spPr", "txPr"].into_iter().collect();
        report_unmodelled(context, label_element, &label_handled);
        ChartDisplayUnitsLabel {
            layout: super::parse_util::manual_layout(Some(label_element)),
            tx: parse_chart_text(context, c_child(Some(label_element), "tx"), false),
            sp_pr: shape_properties(context, Some(label_element)),
            tx_pr: text_properties(Some(label_element)),
        }
    });
    Some(ChartDisplayUnits {
        built_in_unit: c_val(Some(element), "builtInUnit"),
        custom_unit: c_number(context, Some(element), "custUnit"),
        label,
        ext_lst: extension_list_of(context, Some(element)),
    })
}
