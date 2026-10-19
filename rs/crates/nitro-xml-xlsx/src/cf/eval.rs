// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/cf-evaluator.ts,
// cf-helpers.ts, cf-values.ts, data-bar-geometry.ts (cellIs/iconSet/colorScale/dataBar/
// expression path); changes listed in PROVENANCE.md.

use std::collections::HashMap;

use crate::cf::expr::{eval_cf_expression, eval_cf_formula_number, eval_cf_formula_text};
use crate::model::{
    ActiveWorksheet, CfvoThreshold, Color, ConditionalFormat, ConditionalRule, DifferentialStyle,
    DumpCellValue, ThemePalette,
};
use crate::sheet_access::{get_cell, range_contains, CellIndex};
use crate::view::colors::{mix_colors, resolve_color};
use crate::view::types::{DataBarView, IconView};

/// Prepared color-scale endpoints (threshold cuts + resolved CSS), keyed by format index.
#[derive(Debug, Clone)]
struct ColorScalePrep {
    cuts: Vec<f64>,
    css: Vec<String>,
}

/// Per-paint memo for CF work shared across viewport cells.
///
/// Without this, [`cf_at`] re-scans and re-sorts every CF range on **every** cell in the
/// viewport — O(cells × range) on first paint — and rebuilds cut/color vectors per cell.
/// Live grid paint threads one cache for the frame.
#[derive(Debug, Default)]
pub struct CfPaintCache {
    range_nums: HashMap<usize, Vec<f64>>,
    value_counts: HashMap<usize, HashMap<String, usize>>,
    color_scales: HashMap<usize, ColorScalePrep>,
    /// `(format_idx, rule_idx)` in priority order; built once per frame.
    ordered_rules: Option<Vec<(usize, usize)>>,
    /// Union bbox of all CF ranges (`r0,c0,r1,c1`); `None` until computed, `Some(None)` if empty.
    cover_bbox: Option<Option<(u32, u32, u32, u32)>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CfIcon {
    pub set: String,
    pub index: u32,
    pub hide_value: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CfApplied {
    pub icon: Option<CfIcon>,
    pub color_scale: Option<String>,
    pub data_bar: Option<DataBarView>,
    pub style: Option<DifferentialStyle>,
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

fn cached_range_numbers<'a>(
    cache: &'a mut CfPaintCache,
    format_idx: usize,
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    format: &ConditionalFormat,
) -> &'a [f64] {
    cache
        .range_nums
        .entry(format_idx)
        .or_insert_with(|| range_numbers(sheet, index, format))
}

fn cached_value_counts<'a>(
    cache: &'a mut CfPaintCache,
    format_idx: usize,
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    format: &ConditionalFormat,
) -> &'a HashMap<String, usize> {
    cache
        .value_counts
        .entry(format_idx)
        .or_insert_with(|| range_value_counts(sheet, index, format))
}

fn ensure_ordered_rules(cache: &mut CfPaintCache, sheet: &ActiveWorksheet) {
    if cache.ordered_rules.is_some() {
        return;
    }
    let mut entries: Vec<(usize, usize, i32, usize)> = Vec::new();
    let mut order = 0usize;
    for (format_idx, format) in sheet.conditional_formats.iter().enumerate() {
        for (rule_idx, rule) in format.rules.iter().enumerate() {
            entries.push((format_idx, rule_idx, rule_priority(rule), order));
            order += 1;
        }
    }
    entries.sort_by(|a, b| a.2.cmp(&b.2).then(a.3.cmp(&b.3)));
    cache.ordered_rules = Some(entries.into_iter().map(|(f, r, _, _)| (f, r)).collect());
}

fn cell_in_any_cf(cache: &mut CfPaintCache, sheet: &ActiveWorksheet, row: u32, col: u32) -> bool {
    if cache.cover_bbox.is_none() {
        let mut bbox: Option<(u32, u32, u32, u32)> = None;
        for format in &sheet.conditional_formats {
            for r in &format.ranges {
                bbox = Some(match bbox {
                    None => (r.start.row, r.start.col, r.end.row, r.end.col),
                    Some((r0, c0, r1, c1)) => (
                        r0.min(r.start.row),
                        c0.min(r.start.col),
                        r1.max(r.end.row),
                        c1.max(r.end.col),
                    ),
                });
            }
        }
        cache.cover_bbox = Some(bbox);
    }
    match cache.cover_bbox {
        Some(Some((r0, c0, r1, c1))) => row >= r0 && row <= r1 && col >= c0 && col <= c1,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)] // mirrors threshold + color resolve inputs for one CF format
fn cached_color_scale<'a>(
    cache: &'a mut CfPaintCache,
    format_idx: usize,
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    format: &ConditionalFormat,
    thresholds: &[CfvoThreshold],
    colors: &[Color],
    theme: &ThemePalette,
    indexed: &[&str],
) -> &'a ColorScalePrep {
    if !cache.color_scales.contains_key(&format_idx) {
        let nums = cached_range_numbers(cache, format_idx, sheet, index, format).to_vec();
        let cuts: Vec<f64> = thresholds
            .iter()
            .map(|t| threshold_value(sheet, index, t, &nums))
            .collect();
        let css: Vec<String> = colors
            .iter()
            .map(|c| resolve_rgb(c, theme, indexed))
            .collect();
        cache
            .color_scales
            .insert(format_idx, ColorScalePrep { cuts, css });
    }
    cache.color_scales.get(&format_idx).expect("just inserted")
}

fn threshold_value(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    t: &CfvoThreshold,
    nums: &[f64],
) -> f64 {
    let min = nums.first().copied().unwrap_or(0.0);
    let max = nums.last().copied().unwrap_or(0.0);
    // Upstream `n()`: literal first, else evaluate formula (incl. `$T$5` cell refs).
    let n = t
        .value
        .as_deref()
        .and_then(|v| literal_number(v).or_else(|| eval_cf_formula_number(sheet, index, v)))
        .unwrap_or(0.0);
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

fn scale_color(value: f64, cuts: &[f64], colors: &[String]) -> Option<String> {
    if cuts.is_empty() || cuts.len() != colors.len() {
        return None;
    }
    if value <= cuts[0] {
        return Some(colors[0].clone());
    }
    let last = cuts.len() - 1;
    if value >= cuts[last] {
        return Some(colors[last].clone());
    }
    for i in 0..last {
        let lo = cuts[i];
        let hi = cuts[i + 1];
        if value >= lo && value <= hi {
            let a = &colors[i];
            let b = colors.get(i + 1).unwrap_or(a);
            return Some(if hi > lo {
                mix_colors(a, b, (value - lo) / (hi - lo), true)
            } else {
                b.clone()
            });
        }
    }
    Some(colors[last].clone())
}

fn resolve_rgb(color: &Color, theme: &ThemePalette, indexed: &[&str]) -> String {
    resolve_color(Some(color), theme, Some("#638EC6"), indexed).unwrap_or_else(|| "#638EC6".into())
}

/// `(start, fraction, axis_fraction)`.
struct DataBarGeomOpts {
    /// `none` for classic bars; `automatic` when an x14 extension is linked.
    axis_none: bool,
    auto_min: bool,
    auto_max: bool,
    min_length: f64,
    max_length: f64,
}

fn data_bar_geometry(
    value: f64,
    low: f64,
    high: f64,
    opts: DataBarGeomOpts,
) -> (f64, f64, Option<f64>) {
    // Port of upstream data-bar-geometry.ts.
    let lo = if opts.auto_min { low.min(0.0) } else { low };
    let hi = if opts.auto_max { high.max(0.0) } else { high };
    let minimum = opts.min_length;
    let delta = opts.max_length - minimum;
    let interpolate =
        |ratio: f64, offset: f64| (offset + ratio.clamp(0.0, 1.0) * delta).clamp(0.0, 1.0);
    if opts.axis_none {
        let ratio = if hi > lo {
            ((value - lo) / (hi - lo)).clamp(0.0, 1.0)
        } else {
            0.5
        };
        return (0.0, interpolate(ratio, minimum), None);
    }
    if lo == 0.0 && hi == 0.0 {
        return (0.5, minimum / 2.0, Some(0.5));
    }
    if lo >= 0.0 {
        let offset = if lo == 0.0 { minimum / 2.0 } else { minimum };
        let fraction = if hi > lo {
            interpolate((value - lo) / (hi - lo), offset)
        } else {
            interpolate(0.5, minimum)
        };
        return (0.0, fraction, None);
    }
    if hi <= 0.0 {
        let offset = if hi == 0.0 { minimum / 2.0 } else { minimum };
        let fraction = if hi > lo {
            interpolate((hi - value) / (hi - lo), offset)
        } else {
            interpolate(0.5, minimum)
        };
        let axis = if hi == 0.0 {
            (1.0 + opts.max_length - minimum) / 2.0
        } else {
            opts.max_length
        };
        return (
            (axis - fraction).max(0.0),
            fraction,
            if axis > 0.0 && axis < 1.0 {
                Some(axis)
            } else {
                None
            },
        );
    }
    let axis = (1.0 - opts.max_length + minimum) / 2.0 + (-lo / (hi - lo)) * delta;
    let bounded = value.clamp(lo, hi);
    let fraction = interpolate(bounded.abs() / (hi - lo), minimum / 2.0);
    let start = if bounded < 0.0 { axis - fraction } else { axis };
    (start, fraction, Some(axis))
}

fn merge_dxf(acc: Option<DifferentialStyle>, next: &DifferentialStyle) -> DifferentialStyle {
    let Some(mut out) = acc else {
        return next.clone();
    };
    // Earlier (higher priority) wins on conflicts; later rules fill missing font bits
    // (Compare-to-totals: red from <=0.2*F plus double-underline from >=0.3*F when F is blank).
    if let Some(next_font) = next.font.as_ref() {
        out.font = Some(match out.font.take() {
            Some(prev) => {
                let mut merged = next_font.clone();
                // Overlay earlier fields onto the later base so earlier wins.
                merged = crate::view::style_view::merge_font(&merged, Some(&prev));
                merged
            }
            None => next_font.clone(),
        });
    }
    if out.fill.is_none() && next.fill.is_some() {
        out.fill = next.fill.clone();
    }
    if out.border.is_none() && next.border.is_some() {
        out.border = next.border.clone();
    }
    if out.num_fmt.is_none() && next.num_fmt.is_some() {
        out.num_fmt = next.num_fmt.clone();
    }
    out
}

fn style_nonempty(style: &DifferentialStyle) -> bool {
    style.font.is_some()
        || style.fill.is_some()
        || style.border.is_some()
        || style.num_fmt.is_some()
}

fn cell_text_value(value: Option<&DumpCellValue>) -> Option<String> {
    match value {
        Some(DumpCellValue::Error(_)) | None => None,
        Some(DumpCellValue::String(s)) => Some(s.clone()),
        Some(DumpCellValue::Number(n)) => Some(if n.fract() == 0.0 {
            format!("{n:.0}")
        } else {
            n.to_string()
        }),
        Some(DumpCellValue::NumberRaw(s)) => Some(s.clone()),
        Some(DumpCellValue::Bool(b)) => Some(if *b { "TRUE".into() } else { "FALSE".into() }),
        Some(DumpCellValue::Null) => Some(String::new()),
    }
}

fn test_text_rule(kind: &str, haystack: &str, needle: &str) -> bool {
    let text = haystack.to_ascii_lowercase();
    let needle = needle.to_ascii_lowercase();
    match kind {
        "containsText" => text.contains(&needle),
        "notContainsText" => !text.contains(&needle),
        "beginsWith" => text.starts_with(&needle),
        "endsWith" => text.ends_with(&needle),
        _ => false,
    }
}

fn rule_priority(rule: &ConditionalRule) -> i32 {
    match rule {
        ConditionalRule::CellIs { priority, .. }
        | ConditionalRule::Expression { priority, .. }
        | ConditionalRule::ColorScale { priority, .. }
        | ConditionalRule::DataBar { priority, .. }
        | ConditionalRule::IconSet { priority, .. }
        | ConditionalRule::Text { priority, .. }
        | ConditionalRule::Top10 { priority, .. }
        | ConditionalRule::AboveAverage { priority, .. }
        | ConditionalRule::UniqueDup { priority, .. }
        | ConditionalRule::Other { priority, .. } => *priority,
    }
}

fn value_key(value: &DumpCellValue) -> Option<String> {
    match value {
        DumpCellValue::Null => None,
        DumpCellValue::String(s) if s.is_empty() => None,
        DumpCellValue::String(s) => Some(format!("s:{}", s.to_ascii_lowercase())),
        DumpCellValue::Number(n) if n.is_finite() => Some(format!("n:{n}")),
        DumpCellValue::NumberRaw(s) => {
            let n: f64 = s.trim().parse().ok()?;
            if n.is_finite() {
                Some(format!("n:{n}"))
            } else {
                None
            }
        }
        DumpCellValue::Bool(b) => Some(format!("b:{b}")),
        DumpCellValue::Error(e) => Some(format!("e:{e}")),
        DumpCellValue::Number(_) => None,
    }
}

fn range_value_counts(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    format: &ConditionalFormat,
) -> std::collections::HashMap<String, usize> {
    let mut counts = std::collections::HashMap::new();
    for range in &format.ranges {
        for row in range.start.row..=range.end.row {
            for col in range.start.col..=range.end.col {
                if let Some(cell) = get_cell(sheet, index, row, col) {
                    if let Some(key) = value_key(&cell.value) {
                        *counts.entry(key).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    counts
}

fn apply_style_rule(
    result: &mut CfApplied,
    style: &DifferentialStyle,
    stop_if_true: bool,
    any: &mut bool,
) -> bool {
    if style_nonempty(style) {
        result.style = Some(merge_dxf(result.style.take(), style));
    }
    *any = true;
    if stop_if_true {
        return true; // caller should break
    }
    false
}

/// Full CF application for `(row, col)`.
pub fn cf_at(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    row: u32,
    col: u32,
    theme: &ThemePalette,
    indexed: &[&str],
) -> Option<CfApplied> {
    let mut cache = CfPaintCache::default();
    cf_at_cached(sheet, index, row, col, theme, indexed, &mut cache)
}

/// Like [`cf_at`], but reuses range scans / color-scale prep across cells in one paint frame.
pub fn cf_at_cached(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    row: u32,
    col: u32,
    theme: &ThemePalette,
    indexed: &[&str],
    cache: &mut CfPaintCache,
) -> Option<CfApplied> {
    if sheet.conditional_formats.is_empty() {
        return None;
    }
    // Fast reject for the common viewport cells outside every CF sqref.
    if !cell_in_any_cf(cache, sheet, row, col) {
        return None;
    }

    let cell = get_cell(sheet, index, row, col);
    let value = cell.map(|c| &c.value);
    let number = value.and_then(cell_number);
    let mut result = CfApplied::default();
    let mut any = false;

    ensure_ordered_rules(cache, sheet);
    let n_rules = cache.ordered_rules.as_ref().map_or(0, |v| v.len());
    for i in 0..n_rules {
        let (format_idx, rule_idx) = cache.ordered_rules.as_ref().expect("ordered")[i];
        let format = &sheet.conditional_formats[format_idx];
        let rule = &format.rules[rule_idx];
        if !format.ranges.iter().any(|r| range_contains(r, row, col)) {
            continue;
        }
        let matched = match rule {
            ConditionalRule::CellIs {
                operator,
                formulas,
                style,
                stop_if_true,
                ..
            } => {
                // Quoted string operands (`" "`) must not coerce to 0 — number≠text in Excel.
                let first = formulas.first().map(String::as_str).unwrap_or("");
                let quoted = {
                    let t = first.trim().trim_start_matches('=');
                    t.len() >= 2 && t.starts_with('"') && t.ends_with('"')
                };
                let matched = if quoted || number.is_none() {
                    if operator != "equal" && operator != "notEqual" {
                        continue;
                    }
                    let Some(hay) = cell_text_value(value) else {
                        continue;
                    };
                    let Some(rhs) = eval_cf_formula_text(sheet, index, first) else {
                        continue;
                    };
                    let eq = hay.eq_ignore_ascii_case(&rhs);
                    if operator == "equal" {
                        eq
                    } else {
                        !eq
                    }
                } else if let Some(n) = number {
                    let a = literal_number(first)
                        .or_else(|| eval_cf_formula_number(sheet, index, first))
                        .unwrap_or(0.0);
                    let b = formulas.get(1).and_then(|f| {
                        literal_number(f).or_else(|| eval_cf_formula_number(sheet, index, f))
                    });
                    test_operator(operator, n, a, b)
                } else {
                    continue;
                };
                if !matched {
                    continue;
                }
                if style_nonempty(style) {
                    result.style = Some(merge_dxf(result.style.take(), style));
                }
                any = true;
                if *stop_if_true {
                    break;
                }
                true
            }
            ConditionalRule::Expression {
                formula,
                style,
                stop_if_true,
                ..
            } => {
                let anchor = format
                    .ranges
                    .first()
                    .map(|r| (r.start.row, r.start.col))
                    .unwrap_or((row, col));
                if !eval_cf_expression(sheet, index, formula, anchor.0, anchor.1, row, col) {
                    continue;
                }
                if style_nonempty(style) {
                    result.style = Some(merge_dxf(result.style.take(), style));
                }
                any = true;
                if *stop_if_true {
                    break;
                }
                true
            }
            ConditionalRule::ColorScale {
                thresholds, colors, ..
            } => {
                if result.color_scale.is_some() {
                    continue;
                }
                let Some(n) = number else {
                    continue;
                };
                let prep = cached_color_scale(
                    cache, format_idx, sheet, index, format, thresholds, colors, theme, indexed,
                );
                if let Some(c) = scale_color(n, &prep.cuts, &prep.css) {
                    result.color_scale = Some(c);
                    any = true;
                }
                true
            }
            ConditionalRule::DataBar {
                min,
                max,
                color,
                show_value,
                min_length,
                max_length,
                border,
                auto_min,
                auto_max,
                extension_id,
                ..
            } => {
                if result.data_bar.is_some() {
                    continue;
                }
                let Some(n) = number else {
                    continue;
                };
                let nums = cached_range_numbers(cache, format_idx, sheet, index, format);
                let data_lo = threshold_value(sheet, index, min, nums);
                let data_hi = threshold_value(sheet, index, max, nums);
                // Upstream: axis is `none` without x14; `automatic` when an extension is linked.
                let has_x14 = extension_id.is_some();
                let (start, fraction, axis_frac) = data_bar_geometry(
                    n,
                    data_lo,
                    data_hi,
                    DataBarGeomOpts {
                        axis_none: !has_x14,
                        auto_min: *auto_min,
                        auto_max: *auto_max,
                        min_length: (*min_length / 100.0).clamp(0.0, 1.0),
                        max_length: (*max_length / 100.0).clamp(0.0, 1.0),
                    },
                );
                let positive = resolve_rgb(color, theme, indexed);
                // x14: negativeFill/Border #FF0000 when not same-as-positive (NewStyle).
                let (css, border_css) = if n < 0.0 && has_x14 {
                    ("#FF0000".to_string(), "#FF0000".to_string())
                } else {
                    (positive.clone(), positive.clone())
                };
                let direction = if n < 0.0 {
                    "rightToLeft"
                } else {
                    "leftToRight"
                };
                result.data_bar = Some(DataBarView {
                    fraction,
                    start: Some(start),
                    axis: axis_frac.map(|f| (f, "#000000".into())),
                    color: css,
                    negative: if n < 0.0 { Some(true) } else { None },
                    gradient: Some(true),
                    direction: Some(direction.into()),
                    border_color: if *border { Some(border_css) } else { None },
                });
                if !*show_value {
                    result.hide_value = true;
                }
                any = true;
                true
            }
            ConditionalRule::IconSet {
                icon_set,
                thresholds,
                reverse,
                show_value,
                ..
            } => {
                if result.icon.is_some() {
                    continue;
                }
                let Some(n) = number else {
                    continue;
                };
                let nums = cached_range_numbers(cache, format_idx, sheet, index, format);
                let cuts: Vec<f64> = thresholds
                    .iter()
                    .map(|t| threshold_value(sheet, index, t, nums))
                    .collect();
                let mut idx = icon_index(n, &cuts, thresholds);
                if *reverse && !thresholds.is_empty() {
                    idx = thresholds.len() - 1 - idx;
                }
                result.icon = Some(CfIcon {
                    set: icon_set.clone(),
                    index: idx as u32,
                    hide_value: !*show_value,
                });
                if !*show_value {
                    result.hide_value = true;
                }
                any = true;
                true
            }
            ConditionalRule::Text {
                kind,
                text: needle,
                style,
                stop_if_true,
                ..
            } => {
                let Some(haystack) = cell_text_value(value) else {
                    continue;
                };
                if !test_text_rule(kind, &haystack, needle) {
                    continue;
                }
                if apply_style_rule(&mut result, style, *stop_if_true, &mut any) {
                    break;
                }
                true
            }
            ConditionalRule::Top10 {
                rank,
                percent,
                bottom,
                style,
                stop_if_true,
                ..
            } => {
                let Some(n) = number else {
                    continue;
                };
                let sorted = cached_range_numbers(cache, format_idx, sheet, index, format);
                if sorted.is_empty() {
                    continue;
                }
                let count = if *percent {
                    ((sorted.len() as f64 * *rank) / 100.0).floor().max(1.0) as usize
                } else {
                    rank.max(1.0) as usize
                };
                let k = count.min(sorted.len());
                let hit = if *bottom {
                    n <= sorted[k - 1]
                } else {
                    n >= sorted[sorted.len() - k]
                };
                if !hit {
                    continue;
                }
                if apply_style_rule(&mut result, style, *stop_if_true, &mut any) {
                    break;
                }
                true
            }
            ConditionalRule::AboveAverage {
                below,
                equal_average,
                style,
                stop_if_true,
                ..
            } => {
                let Some(n) = number else {
                    continue;
                };
                let nums = cached_range_numbers(cache, format_idx, sheet, index, format);
                if nums.is_empty() {
                    continue;
                }
                let avg = nums.iter().sum::<f64>() / nums.len() as f64;
                let hit = if *below {
                    if *equal_average {
                        n <= avg
                    } else {
                        n < avg
                    }
                } else if *equal_average {
                    n >= avg
                } else {
                    n > avg
                };
                if !hit {
                    continue;
                }
                if apply_style_rule(&mut result, style, *stop_if_true, &mut any) {
                    break;
                }
                true
            }
            ConditionalRule::UniqueDup {
                duplicate,
                style,
                stop_if_true,
                ..
            } => {
                let Some(val) = value else {
                    continue;
                };
                let Some(key) = value_key(val) else {
                    continue;
                };
                let counts = cached_value_counts(cache, format_idx, sheet, index, format);
                let count = counts.get(&key).copied().unwrap_or(0);
                let hit = if *duplicate { count > 1 } else { count == 1 };
                if !hit {
                    continue;
                }
                if apply_style_rule(&mut result, style, *stop_if_true, &mut any) {
                    break;
                }
                true
            }
            ConditionalRule::Other { stop_if_true, .. } => {
                let _ = stop_if_true;
                false
            }
        };
        let _ = matched;
    }

    if any {
        if let Some(icon) = &result.icon {
            if icon.hide_value {
                result.hide_value = true;
            }
        }
        Some(result)
    } else {
        None
    }
}

/// Icon for `(row, col)` when an icon-set rule applies after higher-priority stopIfTrue rules.
pub fn cf_icon_at(
    sheet: &ActiveWorksheet,
    index: &CellIndex,
    row: u32,
    col: u32,
) -> Option<CfIcon> {
    let theme = crate::workbook::default_theme();
    cf_at(sheet, index, row, col, &theme, &[]).and_then(|a| a.icon)
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
    use crate::model::{CellAddress, CellRange, Color, DumpCell, SheetState, ThemePalette};
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
            cell_store: None,
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
                        style: DifferentialStyle {
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
    fn cell_is_quoted_space_does_not_match_number_zero() {
        // Catches: cellIs equal `" "` coerces operand to 0 so StructuredRefs paints theme-0 white over gray.
        let sheet = ActiveWorksheet {
            name: "t".into(),
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
            cell_store: None,
            cells: vec![DumpCell {
                row: 0,
                col: 0,
                value: DumpCellValue::Number(0.0),
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
            }],
            data_validations: vec![],
            comment_addresses: vec![],
            hyperlinks: vec![],
            ext_lst: vec![],
            sparkline_groups: vec![],
            drawings: vec![],
            conditional_formats: vec![ConditionalFormat {
                ranges: vec![CellRange {
                    start: CellAddress { row: 0, col: 0 },
                    end: CellAddress { row: 0, col: 0 },
                }],
                rules: vec![ConditionalRule::CellIs {
                    operator: "equal".into(),
                    formulas: vec!["\" \"".into()],
                    style: DifferentialStyle {
                        font: None,
                        fill: Some(crate::model::Fill::Pattern {
                            pattern: "solid".into(),
                            fg_color: Some(Color {
                                rgb: Some("FFFFFFFF".into()),
                                theme: None,
                                tint: None,
                                indexed: None,
                                auto: None,
                            }),
                            bg_color: None,
                        }),
                        border: None,
                        num_fmt: None,
                    },
                    priority: 1,
                    stop_if_true: false,
                }],
            }],
            dimension: None,
        };
        let index = CellIndex::build(&sheet);
        let theme = ThemePalette {
            colors: vec!["FFFFFF".into()],
            major_font: String::new(),
            minor_font: String::new(),
        };
        assert!(
            cf_at(&sheet, &index, 0, 0, &theme, &[])
                .and_then(|a| a.style)
                .is_none(),
            "number 0 must not equal quoted space"
        );
    }

    #[test]
    fn cell_is_formula_operand_from_sheet_cell() {
        // Catches: cellIs lessThan `$G$5` treats operand as 0 so Products2 never gets CF numFmt $#,##0.
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/real/xlsx/poi-ConditionalFormattingSamples.xlsx"
        ))
        .expect("fixture");
        let wb = crate::load_workbook(&bytes).expect("wb");
        let idx = wb
            .sheets
            .iter()
            .position(|s| s.name == "Products2")
            .expect("Products2");
        let mut sheet = crate::parse_worksheet_at(&bytes, &wb, idx).expect("sheet");
        sheet.ensure_dump_cells(&wb.shared_strings);
        let index = CellIndex::build(&sheet);
        // D10 = 1103.5 is NOT < 200 — no match. Find a value < 200 in D.
        // D column under lessThan $G$5(=200): pick a cell known small from oracle ($17).
        let mut matched = false;
        for row in 2..24u32 {
            if let Some(applied) = cf_at(&sheet, &index, row, 3, &wb.theme, &[]) {
                if applied
                    .style
                    .as_ref()
                    .and_then(|s| s.num_fmt.as_ref())
                    .is_some()
                {
                    matched = true;
                    break;
                }
            }
        }
        assert!(
            matched,
            "expected at least one D-column cellIs lessThan $G$5 match"
        );
        // B column equal $G$2 ("Grain")
        let mut grain = false;
        for row in 2..24u32 {
            if let Some(applied) = cf_at(&sheet, &index, row, 1, &wb.theme, &[]) {
                if applied
                    .style
                    .as_ref()
                    .and_then(|s| s.font.as_ref())
                    .is_some()
                {
                    grain = true;
                    break;
                }
            }
        }
        assert!(grain, "expected B-column equal $G$2 Grain match");
    }

    #[test]
    fn contains_text_applies_grain_dxf() {
        // Catches: containsText rules stay Other so Products1 "Grain" never gets #FFEB9C/#9C6500.
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/real/xlsx/poi-ConditionalFormattingSamples.xlsx"
        ))
        .expect("fixture");
        let wb = crate::load_workbook(&bytes).expect("wb");
        let idx = wb
            .sheets
            .iter()
            .position(|s| s.name == "Products1")
            .expect("Products1");
        let mut sheet = crate::parse_worksheet_at(&bytes, &wb, idx).expect("sheet");
        sheet.ensure_dump_cells(&wb.shared_strings);
        let index = CellIndex::build(&sheet);
        let applied = cf_at(&sheet, &index, 8, 1, &wb.theme, &[]).expect("cf");
        let style = applied.style.expect("dxf");
        let fill = style.fill.expect("fill");
        match fill {
            crate::model::Fill::Pattern {
                fg_color: Some(c), ..
            }
            | crate::model::Fill::Pattern {
                bg_color: Some(c), ..
            } => {
                let rgb = c.rgb.as_deref().unwrap_or("");
                assert!(
                    rgb.eq_ignore_ascii_case("FFFFEB9C") || rgb.eq_ignore_ascii_case("FFEB9C"),
                    "unexpected fill {rgb}"
                );
            }
            other => panic!("unexpected fill {other:?}"),
        }
        let font = style.font.expect("font");
        let color = font.color.expect("font color");
        let rgb = color.rgb.as_deref().unwrap_or("");
        assert!(
            rgb.eq_ignore_ascii_case("FF9C6500") || rgb.eq_ignore_ascii_case("9C6500"),
            "unexpected font {rgb}"
        );
    }

    #[test]
    fn formula_cfvo_reads_cell_threshold() {
        // Catches: iconSet cfvo type=formula val=$T$5 treated as 0 so NewStyle T-column icons drift.
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/real/xlsx/poi-NewStyleConditionalFormattings.xlsx"
        ))
        .expect("fixture");
        let mut wb = crate::load_workbook(&bytes).expect("wb");
        wb.ensure_active_dump_cells();
        let sheet = wb.active_worksheet.as_ref().expect("active");
        let index = CellIndex::build(sheet);
        // T2=1 → yellow (index 1); T5=20 → green (index 2); T14=-1 → red (index 0).
        let t2 = cf_icon_at(sheet, &index, 1, 19).expect("T2");
        assert_eq!(t2.set, "3TrafficLights2");
        assert_eq!(t2.index, 1, "T2=1 should be mid bucket once $T$5→20");
        let t5 = cf_icon_at(sheet, &index, 4, 19).expect("T5");
        assert_eq!(t5.index, 2, "T5=20 should reach formula threshold");
        let t14 = cf_icon_at(sheet, &index, 13, 19).expect("T14");
        assert_eq!(t14.index, 0, "T14=-1 should stay in low bucket");
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
        let mut wb = crate::load_workbook(&bytes).expect("wb");
        assert_eq!(wb.active_sheet, 9);
        wb.ensure_active_dump_cells();
        let sheet = wb.active_worksheet.as_ref().expect("active");
        assert_eq!(sheet.name, "Regional sales");
        assert!(!sheet.conditional_formats.is_empty());
        let index = CellIndex::build(sheet);
        assert!(cf_icon_at(sheet, &index, 3, 1).is_none());
        let west = cf_icon_at(sheet, &index, 4, 1).expect("west");
        assert_eq!(west.set, "3TrafficLights1");
        assert_eq!(west.index, 1);
    }

    #[test]
    fn lazy_first_view_has_cf_icons() {
        // Catches: lazy shell drops conditionalFormatting so the viewer paints no .xg-ic.
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/real/xlsx/poi-ConditionalFormattingSamples.xlsx"
        ))
        .expect("fixture");
        let first = crate::open_first_view(
            &bytes,
            crate::ViewportRect {
                r0: 0,
                c0: 0,
                r1: 30,
                c1: 12,
            },
        )
        .expect("first view");
        let active = first.workbook().active_sheet as usize;
        let sheet = first.sheet(active).expect("active sheet");
        assert!(
            !sheet.conditional_formats.is_empty(),
            "cf empty: cells={} name={}",
            sheet.cells.len(),
            sheet.name
        );
        let index = crate::sheet_access::CellIndex::build(sheet);
        assert!(
            crate::cf::cf_icon_at(sheet, &index, 4, 1).is_some(),
            "west should have icon"
        );
    }
}
