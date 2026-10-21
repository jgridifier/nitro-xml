// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/grid/selection-ops.ts,
// src/core/xlsx/ui/selection.ts (rangeRef/selectionRef),
// src/ui/src/xlsx/grid/grid-commands.ts (currentRegionOrAll),
// and src/core/xlsx/edit/filter.ts (currentRegion);
// changes listed in PROVENANCE.md.

use std::collections::HashSet;

use crate::address::{column_label, format_range, MAX_COL, MAX_ROW};
use crate::display_text::{display_text, Cell};
use crate::model::{
    ActiveWorksheet, CellAddress, CellRange, DumpCell, SheetSelection, WorkbookSlice,
};
use crate::sheet_access::dump_value_to_cell_value;

pub type HeaderState = &'static str;

fn is_whole_rows(range: &CellRange) -> bool {
    range.start.col == 0 && range.end.col == MAX_COL
}

fn is_whole_cols(range: &CellRange) -> bool {
    range.start.row == 0 && range.end.row == MAX_ROW
}

fn normalize_range(range: CellRange) -> CellRange {
    CellRange {
        start: CellAddress {
            row: range.start.row.min(range.end.row),
            col: range.start.col.min(range.end.col),
        },
        end: CellAddress {
            row: range.start.row.max(range.end.row),
            col: range.start.col.max(range.end.col),
        },
    }
}

pub fn cell_range(at: CellAddress) -> CellRange {
    CellRange { start: at, end: at }
}

pub fn span_range(a: CellAddress, b: CellAddress) -> CellRange {
    normalize_range(CellRange { start: a, end: b })
}

/// Whole sheet, written by [`range_ref`] as `1:1048576`.
pub fn all_range() -> CellRange {
    CellRange {
        start: CellAddress { row: 0, col: 0 },
        end: CellAddress {
            row: MAX_ROW,
            col: MAX_COL,
        },
    }
}

/// One range as Excel writes it: `B2:C5`, whole rows `2:4`, whole columns `A:C`,
/// the full sheet `1:1048576`.
pub fn range_ref(range: &CellRange) -> String {
    let range = normalize_range(range.clone());
    if is_whole_rows(&range) && is_whole_cols(&range) {
        return format!("1:{}", MAX_ROW + 1);
    }
    if is_whole_rows(&range) {
        return format!("{}:{}", range.start.row + 1, range.end.row + 1);
    }
    if is_whole_cols(&range) {
        return format!(
            "{}:{}",
            column_label(range.start.col),
            column_label(range.end.col)
        );
    }
    format_range(&range)
}

/// `B2:C5,E1` in the order the ranges were made.
pub fn selection_ref(selection: &SheetSelection) -> String {
    if selection.ranges.is_empty() {
        return crate::address::format_address(&selection.active);
    }
    selection
        .ranges
        .iter()
        .map(range_ref)
        .collect::<Vec<_>>()
        .join(",")
}

/// Grows a range until no merge sticks out of it.
pub fn expand_for_merges(sheet: &ActiveWorksheet, range: CellRange) -> CellRange {
    let mut out = normalize_range(range);
    if sheet.merges.is_empty() || is_whole_rows(&out) || is_whole_cols(&out) {
        return out;
    }
    for _ in 0..8 {
        let mut grown = false;
        for merge in &sheet.merges {
            let hit = merge.start.row <= out.end.row
                && merge.end.row >= out.start.row
                && merge.start.col <= out.end.col
                && merge.end.col >= out.start.col;
            if !hit {
                continue;
            }
            let next = CellRange {
                start: CellAddress {
                    row: out.start.row.min(merge.start.row),
                    col: out.start.col.min(merge.start.col),
                },
                end: CellAddress {
                    row: out.end.row.max(merge.end.row),
                    col: out.end.col.max(merge.end.col),
                },
            };
            if next.start != out.start || next.end != out.end {
                out = next;
                grown = true;
            }
        }
        if !grown {
            break;
        }
    }
    out
}

/// The corner of the last range that Shift+arrows move (opposite the anchor).
pub fn moving_end(selection: &SheetSelection) -> CellAddress {
    let range = selection
        .ranges
        .last()
        .cloned()
        .unwrap_or_else(|| cell_range(selection.active));
    CellAddress {
        row: if range.start.row == selection.anchor.row {
            range.end.row
        } else {
            range.start.row
        },
        col: if range.start.col == selection.anchor.col {
            range.end.col
        } else {
            range.start.col
        },
    }
}

/// Selection used for grid chrome paint when the file has no `sheetView/selection` node.
/// Matches upstream `initialSelection`: active A1 and a **single-cell** range — no merge
/// expansion (unlike click `select_cell`). Expanding A1 across `A1:J1` merges would paint
/// every column header as selected (poi-no_drawing_patriarch ~0.37%).
pub fn paint_selection(sheet: &ActiveWorksheet) -> SheetSelection {
    if let Some(s) = sheet.view.selection.as_ref() {
        return s.clone();
    }
    let at = CellAddress { row: 0, col: 0 };
    SheetSelection {
        active: at,
        anchor: at,
        ranges: vec![cell_range(at)],
    }
}

/// A fresh one-cell selection at `at` (merge-expanded). Active stays on the clicked cell.
pub fn select_cell(sheet: &ActiveWorksheet, at: CellAddress) -> SheetSelection {
    let range = expand_for_merges(sheet, cell_range(at));
    SheetSelection {
        active: at,
        anchor: at,
        ranges: vec![range],
    }
}

/// Extends the last range from the anchor to `to` (Shift+click, Shift+arrows, drag).
pub fn extend_selection(
    selection: &SheetSelection,
    sheet: &ActiveWorksheet,
    to: CellAddress,
) -> SheetSelection {
    let range = span_range(selection.anchor, to);
    let last = expand_for_merges(sheet, range);
    let mut ranges = selection.ranges.clone();
    if ranges.is_empty() {
        ranges.push(last);
    } else {
        let n = ranges.len() - 1;
        ranges[n] = last;
    }
    SheetSelection {
        active: selection.active,
        anchor: selection.anchor,
        ranges,
    }
}

fn displayed_nonempty(workbook: &WorkbookSlice, cell: &DumpCell) -> bool {
    let view = Cell {
        value: dump_value_to_cell_value(&cell.value),
        style_id: cell.style_id,
        rich_text: cell.rich_text.clone(),
    };
    !display_text(workbook, Some(&view)).is_empty()
}

/// Current region around `at`: the smallest rectangle of non-empty displayed text.
pub fn current_region(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    at: CellAddress,
) -> CellRange {
    let filled: HashSet<(u32, u32)> = sheet
        .cells
        .iter()
        .filter(|c| displayed_nonempty(workbook, c))
        .map(|c| (c.row, c.col))
        .collect();
    let row_has = |row: u32, start_col: u32, end_col: u32| -> bool {
        let c0 = start_col.saturating_sub(1);
        let c1 = end_col.saturating_add(1).min(MAX_COL);
        (c0..=c1).any(|c| filled.contains(&(row, c)))
    };
    let col_has = |col: u32, start_row: u32, end_row: u32| -> bool {
        let r0 = start_row.saturating_sub(1);
        let r1 = end_row.saturating_add(1).min(MAX_ROW);
        (r0..=r1).any(|r| filled.contains(&(r, col)))
    };
    let mut r = cell_range(at);
    loop {
        let mut grew = false;
        if r.start.row > 0 && row_has(r.start.row - 1, r.start.col, r.end.col) {
            r.start.row -= 1;
            grew = true;
        }
        if r.end.row < MAX_ROW && row_has(r.end.row + 1, r.start.col, r.end.col) {
            r.end.row += 1;
            grew = true;
        }
        if r.start.col > 0 && col_has(r.start.col - 1, r.start.row, r.end.row) {
            r.start.col -= 1;
            grew = true;
        }
        if r.end.col < MAX_COL && col_has(r.end.col + 1, r.start.row, r.end.row) {
            r.end.col += 1;
            grew = true;
        }
        if !grew {
            break;
        }
    }
    r
}

/// Ctrl+A: the current region, or `None` when that region is one cell or already selected
/// (the caller then selects the whole sheet).
pub fn current_region_or_all(
    workbook: &WorkbookSlice,
    sheet: &ActiveWorksheet,
    selection: &SheetSelection,
) -> Option<CellRange> {
    let region = current_region(workbook, sheet, selection.active);
    let single = region.start.row == region.end.row && region.start.col == region.end.col;
    let same = selection.ranges.last().is_some_and(|last| {
        last.start.row == region.start.row
            && last.start.col == region.start.col
            && last.end.row == region.end.row
            && last.end.col == region.end.col
    });
    if single || same {
        None
    } else {
        Some(region)
    }
}

pub fn row_selected(selection: &SheetSelection, row: u32) -> HeaderState {
    let mut state = "none";
    for r in &selection.ranges {
        if row >= r.start.row && row <= r.end.row {
            if is_whole_rows(r) {
                return "full";
            }
            state = "part";
        }
    }
    state
}

pub fn col_selected(selection: &SheetSelection, col: u32) -> HeaderState {
    let mut state = "none";
    for r in &selection.ranges {
        if col >= r.start.col && col <= r.end.col {
            if is_whole_cols(r) {
                return "full";
            }
            state = "part";
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SheetState;
    use crate::sheet_props::default_sheet_view;
    use std::collections::BTreeMap;

    fn sheet_with_merge() -> ActiveWorksheet {
        ActiveWorksheet {
            name: "t".into(),
            sheet_id: 1,
            state: SheetState::Visible,
            part_name: None,
            default_row_height: 15.0,
            default_col_width: None,
            format: None,
            view: default_sheet_view(),
            columns: vec![],
            merges: vec![CellRange {
                start: CellAddress { row: 0, col: 0 },
                end: CellAddress { row: 1, col: 1 },
            }],
            row_info: BTreeMap::new(),
            cell_store: None,
            cells: vec![],
            cell_pos: Default::default(),
            data_validations: vec![],
            comment_addresses: vec![],
            hyperlinks: vec![],
            ext_lst: vec![],
            sparkline_groups: vec![],
            drawings: Vec::new(),
            conditional_formats: Vec::new(),
            dimension: None,
        }
    }

    #[test]
    fn catches_paint_selection_defaults_to_a1_when_view_has_none() {
        // Catches: default paint selection must stay A1:A1 (not merge-expanded), else headers
        // mark every column in A1:J1 and the outline/active chrome drifts vs TS (poi-no_drawing).
        let sheet = sheet_with_merge();
        assert!(sheet.view.selection.is_none());
        let sel = paint_selection(&sheet);
        assert_eq!(sel.active, CellAddress { row: 0, col: 0 });
        assert_eq!(
            sel.ranges,
            vec![CellRange {
                start: CellAddress { row: 0, col: 0 },
                end: CellAddress { row: 0, col: 0 },
            }]
        );
        assert_eq!(col_selected(&sel, 0), "part");
        assert_eq!(col_selected(&sel, 1), "none");
        assert_eq!(row_selected(&sel, 0), "part");
        // Catches: A1:B2 merge must not expand open selection (would mark col B / row 2 headers).
        assert_eq!(col_selected(&sel, 1), "none");
        assert_eq!(row_selected(&sel, 1), "none");
    }

    #[test]
    fn merge_click_expands_and_extend_reaches_c2() {
        // Catches: click on covered corner of A1:B2 merge yields range A1:B2 with active at clicked cell; extend to C1 yields A1:C2.
        let sheet = sheet_with_merge();
        let clicked = CellAddress { row: 1, col: 1 };
        let sel = select_cell(&sheet, clicked);
        assert_eq!(sel.active, clicked);
        assert_eq!(sel.anchor, clicked);
        assert_eq!(
            sel.ranges[0],
            CellRange {
                start: CellAddress { row: 0, col: 0 },
                end: CellAddress { row: 1, col: 1 },
            }
        );
        assert_eq!(selection_ref(&sel), "A1:B2");
        let ext = extend_selection(&sel, &sheet, CellAddress { row: 0, col: 2 });
        assert_eq!(ext.active, clicked);
        assert_eq!(
            ext.ranges[0],
            CellRange {
                start: CellAddress { row: 0, col: 0 },
                end: CellAddress { row: 1, col: 2 },
            }
        );
        assert_eq!(selection_ref(&ext), "A1:C2");
    }
}
