// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/worksheet.ts; changes listed in PROVENANCE.md.
//! Parse one worksheet part (Stage 16).

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::opc::Package;
use nitro_xml_opc::xml::parse_xml_str;
use nitro_xml_opc::xml::Element;

use crate::address::parse_range;
use crate::cells::CellStore;
use crate::cf::read_conditional_formats;
use crate::comments::load_comment_addresses;
use crate::drawing::load_sheet_drawings;
use crate::hyperlinks::read_hyperlinks;
use crate::metadata::parse_dynamic_array_metadata;
use crate::model::{ActiveWorksheet, DifferentialStyle, SharedString};
use crate::sheet_data::{
    read_sheet_data_element, read_sheet_data_xml, store_to_dump_cells,
    store_to_dump_cells_with_sst, CellContext,
};
use crate::sheet_data_source::split_sheet_data;
use crate::sheet_props::{read_columns, read_data_validations, read_sheet_format, read_sheet_view};
use crate::sparklines::{extract_ext_lst, read_sparkline_groups_from_ext};
use crate::xml_util::{att, elements, x_children};

pub struct WorksheetContext {
    pub shared_strings: Vec<SharedString>,
    pub xf_map: Vec<u32>,
    pub date1904: bool,
    pub palette: Option<Vec<String>>,
    pub dxfs: Vec<DifferentialStyle>,
    pub dynamic_cells: HashSet<u32>,
    pub warnings: Vec<String>,
}

pub fn parse_worksheet(
    ctx: &mut WorksheetContext,
    pkg: &mut Package,
    part_name: &str,
    name: &str,
    sheet_id: u32,
    state: crate::model::SheetState,
) -> ActiveWorksheet {
    let xml = pkg.text(part_name).ok().flatten();
    let mut store = CellStore::new();
    let mut row_info: HashMap<u32, crate::model::RowInfo> = HashMap::new();

    if xml.is_none() {
        push_warn(
            &mut ctx.warnings,
            format!("Sheet \"{name}\" has no part {part_name}; it was loaded empty."),
        );
        return empty_worksheet(
            name,
            sheet_id,
            state,
            Some(part_name.to_owned()),
            store,
            row_info,
        );
    }
    let xml = xml.unwrap();
    let split = split_sheet_data(xml);
    let shell_owned = split
        .as_ref()
        .map(|s| s.shell_xml.clone())
        .unwrap_or_else(|| xml.to_string());
    let shell = shell_owned.as_str();
    let doc = match parse_xml_str(shell) {
        Ok(d) => d,
        Err(_) => {
            return empty_worksheet(
                name,
                sheet_id,
                state,
                Some(part_name.to_owned()),
                store,
                row_info,
            );
        }
    };
    let root = &doc.root;
    let mut by_name: HashMap<String, &Element> = HashMap::new();
    let mut cf_blocks: Vec<&Element> = Vec::new();
    for node in elements(root) {
        if node.local == "conditionalFormatting" {
            cf_blocks.push(node);
        } else if !by_name.contains_key(&node.local) {
            by_name.insert(node.local.clone(), node);
        }
    }
    let view = read_sheet_view(by_name.get("sheetViews").copied());
    let (default_row_height, default_col_width, format) =
        read_sheet_format(by_name.get("sheetFormatPr").copied());
    let columns = read_columns(by_name.get("cols").copied(), &ctx.xf_map);
    let merges_parent = by_name.get("mergeCells").copied().unwrap_or(root);
    let merges = x_children(merges_parent, "mergeCell")
        .into_iter()
        .filter_map(|node| att(Some(node), "ref").and_then(parse_range))
        .collect();

    let mut cell_ctx = CellContext {
        shared_strings: &ctx.shared_strings,
        xf_map: &ctx.xf_map,
        date1904: ctx.date1904,
        palette: ctx.palette.as_deref(),
        dynamic_cells: &ctx.dynamic_cells,
        warnings: &mut ctx.warnings,
    };
    if let Some(split) = split {
        read_sheet_data_xml(
            &mut cell_ctx,
            &split.sheet_data_inner,
            &mut store,
            &mut row_info,
        );
    } else if let Some(data) = by_name.get("sheetData") {
        read_sheet_data_element(&mut cell_ctx, data, &mut store, &mut row_info);
    }

    let cells = store_to_dump_cells_with_sst(&store, &ctx.shared_strings);
    let ext_lst = extract_ext_lst(&shell_owned);
    let sparkline_groups = by_name
        .get("extLst")
        .map(|e| read_sparkline_groups_from_ext(e))
        .unwrap_or_default();
    let data_validations = read_data_validations(by_name.get("dataValidations").copied());
    let comment_addresses = load_comment_addresses(pkg, part_name);
    let hyperlinks = read_hyperlinks(pkg, part_name, by_name.get("hyperlinks").copied());
    let mut warn = |m: String| push_warn(&mut ctx.warnings, m);
    let drawings = load_sheet_drawings(pkg, part_name, Some(root), &mut warn);
    let mut conditional_formats =
        read_conditional_formats(&cf_blocks, &ctx.dxfs, ctx.palette.as_deref(), &mut warn);
    crate::cf::link_data_bar_extensions(&mut conditional_formats, &ext_lst);
    ActiveWorksheet {
        name: name.to_owned(),
        sheet_id,
        state,
        part_name: Some(part_name.to_owned()),
        default_row_height,
        default_col_width,
        format,
        view,
        columns,
        merges,
        row_info: row_info.into_iter().collect(),
        cells,
        data_validations,
        comment_addresses,
        hyperlinks,
        ext_lst,
        sparkline_groups,
        drawings,
        conditional_formats,
        dimension: None,
    }
}

fn empty_worksheet(
    name: &str,
    sheet_id: u32,
    state: crate::model::SheetState,
    part_name: Option<String>,
    store: CellStore,
    row_info: HashMap<u32, crate::model::RowInfo>,
) -> ActiveWorksheet {
    ActiveWorksheet {
        name: name.to_owned(),
        sheet_id,
        state,
        part_name,
        default_row_height: 15.0,
        default_col_width: None,
        format: None,
        view: crate::sheet_props::default_sheet_view(),
        columns: Vec::new(),
        merges: Vec::new(),
        row_info: row_info.into_iter().collect(),
        cells: store_to_dump_cells(&store),
        data_validations: Vec::new(),
        comment_addresses: Vec::new(),
        hyperlinks: Vec::new(),
        ext_lst: Vec::new(),
        sparkline_groups: Vec::new(),
        drawings: Vec::new(),
        conditional_formats: Vec::new(),
        dimension: None,
    }
}

fn push_warn(warnings: &mut Vec<String>, message: String) {
    if !warnings.contains(&message) {
        warnings.push(message);
    }
}

pub fn load_dynamic_cells(pkg: &mut Package, workbook_part: &str) -> HashSet<u32> {
    let metadata = pkg
        .target_of_type(
            workbook_part,
            nitro_xml_opc::opc::relationship_types::SHEET_METADATA,
        )
        .ok()
        .flatten();
    let xml = metadata.and_then(|p| pkg.text(&p).ok().flatten());
    parse_dynamic_array_metadata(xml)
}

#[cfg(test)]
mod tests {
    include!("worksheet_tests.rs");
}
