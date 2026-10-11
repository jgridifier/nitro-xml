// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/sparkline-data.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

use crate::address::parse_range;
use crate::model::{ActiveWorksheet, CellRange, DumpCellValue, WorkbookSlice};
use crate::sheet_access::{get_cell, CellIndex};
use crate::model::SparklineGroup;
use crate::sparklines::sheet_sparkline_groups;
use crate::view::sparkline_view::{sparkline_scale, sparkline_view, SparklineData, SparklineView};

const MAX_POINTS: usize = 10_000;

struct HostEntry {
    group: SparklineGroup,
    index: usize,
}

fn cell_key(row: u32, col: u32) -> u64 {
    (row as u64) << 32 | col as u64
}

fn hosts(sheet: &ActiveWorksheet) -> HashMap<u64, HostEntry> {
    let mut index = HashMap::new();
    for group in sheet_sparkline_groups(sheet) {
        for (i, line) in group.sparklines.iter().enumerate() {
            let key = cell_key(line.host.start.row, line.host.start.col);
            index.entry(key).or_insert(HostEntry {
                group: group.clone(),
                index: i,
            });
        }
    }
    index
}

fn sheet_by_index(sheets: &[Option<ActiveWorksheet>], index: usize) -> Option<&ActiveWorksheet> {
    sheets.get(index).and_then(|o| o.as_ref())
}

fn sheet_by_name<'a>(
    workbook: &WorkbookSlice,
    sheets: &'a [Option<ActiveWorksheet>],
    name: &str,
) -> Option<&'a ActiveWorksheet> {
    let idx = workbook.sheets.iter().position(|s| s.name == name)?;
    sheet_by_index(sheets, idx)
}

fn normalize_range(range: CellRange) -> CellRange {
    CellRange {
        start: crate::model::CellAddress {
            row: range.start.row.min(range.end.row),
            col: range.start.col.min(range.end.col),
        },
        end: crate::model::CellAddress {
            row: range.start.row.max(range.end.row),
            col: range.start.col.max(range.end.col),
        },
    }
}

/// Resolves `Sheet1!$A$1:$E$1`, `'My sheet'!A1:E1` or an unqualified range on `sheet`.
pub fn resolve_sparkline_range<'a>(
    workbook: &WorkbookSlice,
    sheets: &'a [Option<ActiveWorksheet>],
    sheet: &'a ActiveWorksheet,
    formula: &str,
) -> Option<(&'a ActiveWorksheet, CellRange)> {
    let ref_ = formula.trim().trim_start_matches('=');
    let bang = ref_.rfind('!');
    let (target_sheet, range_part) = if let Some(bang) = bang {
        let raw = ref_[..bang].trim();
        let name = if raw.starts_with('\'') && raw.ends_with('\'') {
            raw[1..raw.len() - 1].replace("''", "'")
        } else {
            raw.to_string()
        };
        let target = sheet_by_name(workbook, sheets, &name)?;
        (target, ref_[bang + 1..].replace('$', ""))
    } else {
        (sheet, ref_.replace('$', ""))
    };
    let range = parse_range(&range_part)?;
    Some((target_sheet, normalize_range(range)))
}

fn row_hidden(sheet: &ActiveWorksheet, row: u32) -> bool {
    sheet
        .row_info
        .get(&row)
        .and_then(|r| r.hidden)
        .unwrap_or(false)
}

fn col_hidden(sheet: &ActiveWorksheet, col: u32) -> bool {
    sheet
        .columns
        .iter()
        .any(|c| c.hidden == Some(true) && col >= c.min && col <= c.max)
}

fn last_stored_row(sheet: &ActiveWorksheet) -> u32 {
    sheet.cells.iter().map(|c| c.row).max().unwrap_or(0)
}

fn read_sparkline_values(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    range: &CellRange,
    display_hidden: bool,
) -> Vec<Option<f64>> {
    let mut end_row = range.end.row;
    if end_row - range.start.row >= MAX_POINTS as u32 {
        end_row = end_row.min(last_stored_row(sheet));
    }
    let mut out = Vec::new();
    let mut row = range.start.row;
    while row <= end_row && out.len() < MAX_POINTS {
        if !display_hidden && row_hidden(sheet, row) {
            row += 1;
            continue;
        }
        let mut col = range.start.col;
        while col <= range.end.col && out.len() < MAX_POINTS {
            if !display_hidden && col_hidden(sheet, col) {
                col += 1;
                continue;
            }
            let value = get_cell(sheet, index, row, col).map(|c| &c.value);
            let n = match value {
                Some(DumpCellValue::Number(n)) if n.is_finite() => Some(*n),
                Some(DumpCellValue::NumberRaw(s)) => {
                    s.parse::<f64>().ok().filter(|n| n.is_finite())
                }
                _ => None,
            };
            out.push(n);
            col += 1;
        }
        row += 1;
    }
    out
}

fn group_data(
    workbook: &WorkbookSlice,
    sheets: &[Option<ActiveWorksheet>],
    sheet: &ActiveWorksheet,
    group: &SparklineGroup,
) -> Vec<SparklineData> {
    let date_range = group.date_axis && group.date_formula.is_some();
    let date_values = if date_range {
        let formula = group.date_formula.as_deref().unwrap();
        resolve_sparkline_range(workbook, sheets, sheet, formula).map(|(sh, range)| {
            let idx = CellIndex::build(sh);
            read_sparkline_values(sh, &idx, &range, group.display_hidden)
        })
    } else {
        None
    };
    group
        .sparklines
        .iter()
        .map(|line| {
            let values = if let Some(formula) = &line.formula {
                resolve_sparkline_range(workbook, sheets, sheet, formula)
                    .map(|(sh, range)| {
                        let idx = CellIndex::build(sh);
                        read_sparkline_values(sh, &idx, &range, group.display_hidden)
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            if let Some(dates) = &date_values {
                SparklineData {
                    values,
                    dates: Some(dates.clone()),
                }
            } else {
                SparklineData {
                    values,
                    dates: None,
                }
            }
        })
        .collect()
}

#[allow(clippy::iter_cloned_collect)]
fn indexed_palette(workbook: &WorkbookSlice) -> Vec<&str> {
    workbook
        .palette
        .as_ref()
        .map(|p| p.iter().map(|s| s.as_str()).collect())
        .unwrap_or_else(|| {
            crate::view::colors::INDEXED_COLORS
                .iter()
                .copied()
                .collect()
        })
}

/// The paint descriptor of the sparkline hosted in a cell, if any.
pub fn sparkline_view_at(
    workbook: &WorkbookSlice,
    sheets: &[Option<ActiveWorksheet>],
    sheet_index: usize,
    row: u32,
    col: u32,
) -> Option<SparklineView> {
    let sheet = sheet_by_index(sheets, sheet_index)?;
    let host_map = hosts(sheet);
    let entry = host_map.get(&cell_key(row, col))?;
    let group = &entry.group;
    let index = entry.index;
    let needs_group = group.min_axis_type == "group" || group.max_axis_type == "group";
    let data = if needs_group {
        group_data(workbook, sheets, sheet, group)
    } else {
        let line = group.sparklines.get(index).cloned();
        let single = SparklineGroup {
            sparklines: line.map(|l| vec![l]).unwrap_or_default(),
            ..group.clone()
        };
        group_data(workbook, sheets, sheet, &single)
    };
    let at = if needs_group { index } else { 0 };
    let data_at = data.get(at).cloned().unwrap_or(SparklineData {
        values: Vec::new(),
        dates: None,
    });
    let scale = sparkline_scale(group, &data, at);
    let indexed = indexed_palette(workbook);
    Some(sparkline_view(
        group,
        &data_at,
        scale,
        &workbook.theme,
        &indexed,
    ))
}
