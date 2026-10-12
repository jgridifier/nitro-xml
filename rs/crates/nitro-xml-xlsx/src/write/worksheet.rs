// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/worksheet.ts (shell + sheetData);
// unmodelled children are copied from the source part. Changes listed in PROVENANCE.md.
//! One worksheet part. Comments, tables, drawings and conditional-format rewrite stay carried.

use std::collections::HashMap;

use nitro_xml_opc::xml::ns;

use crate::edit::shift::{rewrite_carried_sqrefs, AxisShift};
use crate::model::ActiveWorksheet;

use super::fragments::direct_child_xml;
use super::shared_strings::SharedStringTable;
use super::sheet_data::sheet_data_xml;
use super::sheet_props::{
    cols_xml, dimension_xml, merge_cells_xml, sheet_format_xml, sheet_views_xml,
};
use super::xml_out::XML_HEADER;

const NS_XR2: &str = "http://schemas.microsoft.com/office/spreadsheetml/2015/revision2";
const NS_XR3: &str = "http://schemas.microsoft.com/office/spreadsheetml/2016/revision3";

/// Child order from `CT_Worksheet`. Generated names replace the source; the rest are copied.
const ORDER: &[&str] = &[
    "sheetPr",
    "dimension",
    "sheetViews",
    "sheetFormatPr",
    "cols",
    "sheetData",
    "sheetCalcPr",
    "sheetProtection",
    "protectedRanges",
    "scenarios",
    "autoFilter",
    "sortState",
    "dataConsolidate",
    "customSheetViews",
    "mergeCells",
    "phoneticPr",
    "conditionalFormatting",
    "dataValidations",
    "hyperlinks",
    "printOptions",
    "pageMargins",
    "pageSetup",
    "headerFooter",
    "rowBreaks",
    "colBreaks",
    "customProperties",
    "cellWatches",
    "ignoredErrors",
    "smartTags",
    "drawing",
    "legacyDrawing",
    "legacyDrawingHF",
    "drawingHF",
    "picture",
    "oleObjects",
    "controls",
    "webPublishItems",
    "tableParts",
    "extLst",
];

const OWNED: &[&str] = &[
    "dimension",
    "sheetViews",
    "sheetFormatPr",
    "cols",
    "sheetData",
    "mergeCells",
];

/// Shift log and dynamic-array flag threaded through one worksheet write.
pub struct SheetWriteExtra<'a> {
    pub shifts: &'a [AxisShift],
    pub dynamic_cm: u32,
    pub dynamic_used: &'a mut bool,
}

pub fn worksheet_xml(
    sheet: &ActiveWorksheet,
    tab_selected: bool,
    style_count: usize,
    strings: &mut SharedStringTable,
    source_xml: Option<&str>,
    extra: SheetWriteExtra<'_>,
) -> String {
    let mut generated: HashMap<&str, String> = HashMap::new();
    generated.insert("dimension", dimension_xml(sheet));
    generated.insert("sheetViews", sheet_views_xml(sheet, tab_selected));
    generated.insert("sheetFormatPr", sheet_format_xml(sheet));
    generated.insert("cols", cols_xml(sheet, style_count));
    generated.insert(
        "sheetData",
        sheet_data_xml(
            strings,
            style_count,
            extra.dynamic_cm,
            extra.dynamic_used,
            sheet,
        ),
    );
    generated.insert("mergeCells", merge_cells_xml(sheet));
    let source = source_xml.map(direct_child_xml).unwrap_or_default();
    let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
    for (name, xml) in source {
        grouped.entry(name).or_default().push(xml);
    }
    let mut body = String::new();
    for name in ORDER {
        if OWNED.contains(name) {
            if let Some(xml) = generated.get(name) {
                if !xml.is_empty() {
                    body.push_str(xml);
                }
            }
        } else if let Some(chunks) = grouped.get(*name) {
            for chunk in chunks {
                if extra.shifts.is_empty() || !chunk.contains("sqref") {
                    body.push_str(chunk);
                } else if let Some(next) = rewrite_carried_sqrefs(chunk, extra.shifts) {
                    body.push_str(&next);
                }
            }
        }
    }
    format!(
        "{XML_HEADER}<worksheet xmlns=\"{}\" xmlns:r=\"{}\" xmlns:mc=\"{}\" mc:Ignorable=\"x14ac xr xr2 xr3\" xmlns:x14ac=\"{}\" xmlns:xr=\"{}\" xmlns:xr2=\"{NS_XR2}\" xmlns:xr3=\"{NS_XR3}\">{body}</worksheet>",
        ns::X,
        ns::R,
        ns::MC,
        ns::X14AC,
        ns::XR,
    )
}
