// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/cf-evaluator.ts,
// cf-helpers.ts, cf-values.ts (cellIs + iconSet path); changes listed in PROVENANCE.md.

use crate::model::{
    ActiveWorksheet, CfvoThreshold, ConditionalFormat, ConditionalRule, DumpCellValue,
};
use crate::sheet_access::{get_cell, range_contains, CellIndex};
use crate::view::types::IconView;

#[derive(Debug, Clone, PartialEq)]
pub struct CfIcon {
    pub set: String,
    pub index: u32,
    pub hide_value: bool,
}

fn literal_number(formula: &str) -> Option<f64> {
    let text = formula.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<f64>().ok()
}

fn cell_number(value: &DumpCellValue) -> Option<f64> {
    match value {
        DumpCellValue::Number(n) => Some(*n),
        DumpCellValue::NumberRaw(s) => s.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
        _ => None,
    }
}

fn compare_num(a: f64, b: f64) -> i32 {
    if a == b {
        0
    } else if a < b {
        -1
    } else {
        1
    }
}

fn test_operator(operator: &str, value: f64, a: f64, b: Option<f64>) -> bool {
    match operator {
        "equal" => compare_num(value, a) == 0,
        "notEqual" => compare_num(value, a) != 0,
        "greaterThan" => compare_num(value, a) > 0,
        "greaterThanOrEqual" => compare_num(value, a) >= 0,
        "lessThan" => compare_num(value, a) < 0,
        "lessThanOrEqual" => compare_num(value, a) <= 0,
        "between" | "notBetween" => {
            let Some(b) = b else {
                return false;
            };
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            let inside = value >= lo && value <= hi;
            if operator == "between" {
                inside
            } else {
                !inside
            }
        }
        _ => false,
    }
}

fn icon_index(value: f64, cuts: &[f64], list: &[CfvoThreshold]) -> usize {
    let mut index = 0usize;
    for i in 1..cuts.len() {
        let cut = cuts.get(i).copied().unwrap_or(f64::INFINITY);
        let gte = list.get(i).and_then(|t| t.gte) != Some(false);
        let reached = if gte { value >= cut } else { value > cut };
        if reached {
            index = i;
        }
    }
    index
}

fn range_numbers(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    format: &ConditionalFormat,
) -> Vec<f64> {
    let mut nums = Vec::new();
    for range in &format.ranges {
        for row in range.start.row..=range.end.row {
            for col in range.start.col..=range.end.col {
                if let Some(cell) = get_cell(sheet, index, row, col) {
                    if let Some(n) = cell_number(&cell.value) {
                        if n.is_finite() {
                            nums.push(n);
                        }
                    }
                }
            }
        }
    }
    nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    nums
}

fn threshold_value(t: &CfvoThreshold, nums: &[f64]) -> f64 {
    let min = nums.first().copied().unwrap_or(0.0);
    let max = nums.last().copied().unwrap_or(0.0);
    let n = t.value.as_deref().and_then(literal_number).unwrap_or(0.0);
    match t.kind.as_str() {
        "min" => min,
        "max" => max,
        "percent" => min + ((max - min) * n) / 100.0,
        "num" | "formula" => n,
        "percentile" => {
            if nums.is_empty() {
                return 0.0;
            }
            let rank = (n / 100.0).clamp(0.0, 1.0) * (nums.len() - 1) as f64;
            let lo = rank.floor() as usize;
            let hi = rank.ceil() as usize;
            let a = nums[lo];
            let b = nums[hi.min(nums.len() - 1)];
            a + (b - a) * (rank - lo as f64)
        }
        _ => n,
    }
}

/// Icon for `(row, col)` when an icon-set rule applies after higher-priority stopIfTrue rules.
pub fn cf_icon_at(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    row: u32,
    col: u32,
) -> Option<CfIcon> {
    if sheet.conditional_formats.is_empty() {
        return None;
    }
    let mut entries: Vec<(&ConditionalFormat, &ConditionalRule, usize)> = Vec::new();
    let mut order = 0usize;
    for format in &sheet.conditional_formats {
        for rule in &format.rules {
            entries.push((format, rule, order));
            order += 1;
        }
    }
    entries.sort_by(|a, b| {
        let pa = match a.1 {
            ConditionalRule::CellIs { priority, .. }
            | ConditionalRule::IconSet { priority, .. }
            | ConditionalRule::Other { priority, .. } => *priority,
        };
        let pb = match b.1 {
            ConditionalRule::CellIs { priority, .. }
            | ConditionalRule::IconSet { priority, .. }
            | ConditionalRule::Other { priority, .. } => *priority,
        };
        pa.cmp(&pb).then(a.2.cmp(&b.2))
    });

    let cell = get_cell(sheet, index, row, col);
    let value = cell.map(|c| &c.value);
    let number = value.and_then(cell_number);
    let mut icon: Option<CfIcon> = None;

    for (format, rule, _) in entries {
        if !format.ranges.iter().any(|r| range_contains(r, row, col)) {
            continue;
        }
        match rule {
            ConditionalRule::CellIs {
                operator,
                formulas,
                stop_if_true,
                ..
            } => {
                let Some(n) = number else {
                    continue;
                };
                let a = formulas
                    .first()
                    .and_then(|f| literal_number(f))
                    .unwrap_or(0.0);
                let b = formulas.get(1).and_then(|f| literal_number(f));
                if !test_operator(operator, n, a, b) {
                    continue;
                }
                // Matching non-graphic stopIfTrue stops lower-priority rules (including icon sets).
                if *stop_if_true {
                    return icon;
                }
            }
            ConditionalRule::IconSet {
                icon_set,
                thresholds,
                reverse,
                show_value,
                ..
            } => {
                if icon.is_some() {
                    continue;
                }
                let Some(n) = number else {
                    continue;
                };
                let nums = range_numbers(sheet, index, format);
                let cuts: Vec<f64> = thresholds
                    .iter()
                    .map(|t| threshold_value(t, &nums))
                    .collect();
                let mut idx = icon_index(n, &cuts, thresholds);
                if *reverse && !thresholds.is_empty() {
                    idx = thresholds.len() - 1 - idx;
                }
                icon = Some(CfIcon {
                    set: icon_set.clone(),
                    index: idx as u32,
                    hide_value: !*show_value,
                });
            }
            ConditionalRule::Other { stop_if_true, .. } => {
                // Unsupported rules do not match; stopIfTrue cannot fire.
                let _ = stop_if_true;
            }
        }
    }
    icon
}

impl From<CfIcon> for IconView {
    fn from(value: CfIcon) -> Self {
        IconView {
            set: value.set,
            index: value.index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CellAddress, CellRange, DumpCell, SheetState};
    use crate::sheet_props::default_sheet_view;
    use std::collections::BTreeMap;

    fn sheet_with_sales() -> ActiveWorksheet {
        let cells = [
            (3u32, 1781345.0),
            (4, 534389.0),
            (5, 1009268.0),
            (6, 899999.0),
            (7, 2345184.0),
            (8, 900000.0),
            (9, 1567090.0),
            (10, 34678.0),
        ]
        .into_iter()
        .map(|(row, n)| DumpCell {
            row,
            col: 1,
            value: DumpCellValue::Number(n),
            type_tag: None,
            formula: None,
            style_id: None,
            rich_text: None,
            array_range: None,
            legacy_formula: None,
            dynamic_array: None,
            spill_anchor: None,
            cell_metadata: None,
            value_metadata: None,
        })
        .collect();
        ActiveWorksheet {
            name: "Regional sales".into(),
            sheet_id: 1,
            state: SheetState::Visible,
            part_name: None,
            default_row_height: 15.0,
            default_col_width: None,
            format: None,
            view: default_sheet_view(),
            columns: vec![],
            merges: vec![],
            row_info: BTreeMap::new(),
            cells,
            data_validations: vec![],
            comment_addresses: vec![],
            hyperlinks: vec![],
            ext_lst: vec![],
            sparkline_groups: vec![],
            drawings: vec![],
            conditional_formats: vec![ConditionalFormat {
                ranges: vec![CellRange {
                    start: CellAddress { row: 3, col: 1 },
                    end: CellAddress { row: 10, col: 1 },
                }],
                rules: vec![
                    ConditionalRule::CellIs {
                        operator: "greaterThanOrEqual".into(),
                        formulas: vec!["900000".into()],
                        style: crate::model::DifferentialStyle {
                            font: None,
                            fill: None,
                            border: None,
                            num_fmt: None,
                        },
                        priority: 1,
                        stop_if_true: true,
                    },
                    ConditionalRule::IconSet {
                        icon_set: "3TrafficLights1".into(),
                        thresholds: vec![
                            CfvoThreshold {
                                kind: "percent".into(),
                                value: Some("0".into()),
                                gte: None,
                            },
                            CfvoThreshold {
                                kind: "num".into(),
                                value: Some("0".into()),
                                gte: Some(false),
                            },
                            CfvoThreshold {
                                kind: "num".into(),
                                value: Some("900000".into()),
                                gte: Some(false),
                            },
                        ],
                        priority: 2,
                        stop_if_true: false,
                        reverse: false,
                        show_value: true,
                    },
                ],
            }],
            dimension: None,
        }
    }

    #[test]
    fn icon_set_stop_if_true_matches_regional_sales() {
        // Catches: cellIs stopIfTrue not suppressing iconSet so CF samples show icons on >=900k.
        let sheet = sheet_with_sales();
        let index = CellIndex::build(&sheet);
        assert!(cf_icon_at(&sheet, &index, 3, 1).is_none()); // 1781345
        let west = cf_icon_at(&sheet, &index, 4, 1).expect("west icon");
        assert_eq!(west.set, "3TrafficLights1");
        assert_eq!(west.index, 1); // yellow bucket
        assert!(cf_icon_at(&sheet, &index, 8, 1).is_none()); // exactly 900000 stops
        let terr = cf_icon_at(&sheet, &index, 10, 1).expect("territories");
        assert_eq!(terr.index, 1);
    }

    #[test]
    fn loads_regional_sales_icons_from_fixture() {
        // Catches: CF blocks not read from worksheet shell so activeTab Regional sales paints no icons.
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/real/xlsx/poi-ConditionalFormattingSamples.xlsx"
        ))
        .expect("fixture");
        let wb = crate::load_workbook(&bytes).expect("wb");
        assert_eq!(wb.active_sheet, 9);
        let sheet = wb.active_worksheet.as_ref().expect("active");
        assert_eq!(sheet.name, "Regional sales");
        assert!(!sheet.conditional_formats.is_empty());
        let index = CellIndex::build(sheet);
        assert!(cf_icon_at(sheet, &index, 3, 1).is_none());
        let west = cf_icon_at(sheet, &index, 4, 1).expect("west");
        assert_eq!(west.set, "3TrafficLights1");
        assert_eq!(west.index, 1);
    }
}
