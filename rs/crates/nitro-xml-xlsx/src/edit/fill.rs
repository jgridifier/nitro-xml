// SPDX-License-Identifier: MIT
//! `fillRange`: the fill handle (AutoFill). One command, so one undo step.

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::Value;

use crate::address::parse_range;
use crate::editor::input_text::cell_input_text;
use crate::formula_translate::translate_formula;

use super::cells::{ensure_cell, host_mut, sheet_err, write_input};

const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November",
    "December",
];

fn num(s: &str) -> Option<f64> {
    if s.is_empty() || s.starts_with('=') {
        return None;
    }
    s.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

fn fmt_num(n: f64) -> String {
    let s = format!("{:.10}", n);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// "Item 007" -> ("Item ", 7, 3 digits).
fn split_tail(s: &str) -> Option<(&str, i64, usize)> {
    let cut = s.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let digits = &s[cut..];
    if digits.is_empty() || digits.len() > 15 || s.starts_with('=') || num(s).is_some() {
        return None;
    }
    Some((&s[..cut], digits.parse().ok()?, digits.len()))
}

fn name_index(list: &[&str], s: &str) -> Option<(usize, usize, bool)> {
    let l = s.to_lowercase();
    list.iter().enumerate().find_map(|(i, n)| {
        let n = n.to_lowercase();
        (l == n || (l.len() == 3 && n.starts_with(&l))).then_some((i, if l == n { 0 } else { 3 }, s.chars().all(|c| !c.is_lowercase())))
    })
}

fn cased(name: &str, short: usize, upper: bool) -> String {
    let n = if short > 0 { &name[..short] } else { name };
    if upper { n.to_uppercase() } else { n.to_owned() }
}

/// Text for the `i`-th cell past the end of `src` (already ordered along the fill direction).
/// `dir` is +1 / -1 along the axis; `vertical` picks which axis formulas translate on.
pub fn fill_line(src: &[String], n: usize, dir: i32, vertical: bool) -> Vec<String> {
    let k = src.len();
    if k == 0 {
        return vec![String::new(); n];
    }
    // Numeric series (two or more numbers with a constant step).
    let nums: Vec<f64> = src.iter().filter_map(|s| num(s)).collect();
    if k >= 2 && nums.len() == k {
        let step = (nums[k - 1] - nums[0]) / (k - 1) as f64;
        let linear = nums.windows(2).all(|w| (w[1] - w[0] - step).abs() < 1e-9);
        if linear {
            return (1..=n).map(|i| fmt_num(nums[k - 1] + step * i as f64)).collect();
        }
    }
    // Text + trailing number ("Item 1", "Q1"), step 1 for a lone cell else the constant difference.
    let tails: Vec<_> = src.iter().filter_map(|s| split_tail(s)).collect();
    if tails.len() == k && tails.iter().all(|t| t.0 == tails[0].0) {
        let step = if k == 1 { dir as i64 } else { (tails[k - 1].1 - tails[0].1) / (k as i64 - 1) };
        let linear = tails.windows(2).all(|w| w[1].1 - w[0].1 == step);
        if linear {
            let (prefix, last, width) = (tails[0].0, tails[k - 1].1, tails[k - 1].2);
            let pad = if tails[k - 1].2 > 1 && src[k - 1].ends_with(&format!("{:0w$}", last, w = width)) { width } else { 0 };
            return (1..=n as i64)
                .map(|i| format!("{prefix}{:0w$}", (last + step * i).max(0), w = pad))
                .collect();
        }
    }
    // Weekday / month names from a lone cell.
    if k == 1 {
        for list in [&DAYS[..], &MONTHS[..]] {
            if let Some((idx, short, upper)) = name_index(list, &src[0]) {
                let len = list.len() as i64;
                return (1..=n as i64)
                    .map(|i| cased(list[(idx as i64 + dir as i64 * i).rem_euclid(len) as usize], short, upper))
                    .collect();
            }
        }
    }
    // Copy the block cyclically; formulas move with the cell.
    (0..n)
        .map(|i| {
            let off = k + i;
            let s = &src[off % k];
            if s.starts_with('=') {
                let d = dir * (off - off % k) as i32;
                let (dr, dc) = if vertical { (d, 0) } else { (0, d) };
                translate_formula(s, dr, dc)
            } else {
                s.clone()
            }
        })
        .collect()
}

pub fn register_fill_commands(registry: &mut CommandRegistry) {
    registry.register(Command::stub("fillRange", "fillRange").with_runner(|host, args| {
        let h = host_mut(host)?;
        let sheet = args.get("sheet").and_then(Value::as_u64).unwrap_or(0) as u32;
        let err = |m: &str| EditError::Other { message: m.into() };
        let range = parse_range(args.get("range").and_then(Value::as_str).ok_or_else(|| err("fillRange missing range"))?)
            .ok_or_else(|| err("fillRange bad range"))?;
        let dir = args.get("direction").and_then(Value::as_str).unwrap_or("down");
        let count = args.get("count").and_then(Value::as_u64).unwrap_or(0).min(100_000) as i64;
        let (r0, r1) = (range.start.row.min(range.end.row), range.start.row.max(range.end.row));
        let (c0, c1) = (range.start.col.min(range.end.col), range.start.col.max(range.end.col));
        let vertical = matches!(dir, "down" | "up");
        let sign: i32 = if matches!(dir, "down" | "right") { 1 } else { -1 };
        let lines = if vertical { c0..=c1 } else { r0..=r1 };
        for line in lines {
            let cell = |along: u32| if vertical { (along, line) } else { (line, along) };
            let (lo, hi) = if vertical { (r0, r1) } else { (c0, c1) };
            let along: Vec<u32> = if sign > 0 { (lo..=hi).collect() } else { (lo..=hi).rev().collect() };
            let ws = h.sheet(sheet).map_err(sheet_err)?;
            let src: Vec<String> = along
                .iter()
                .map(|&a| {
                    let (r, c) = cell(a);
                    cell_input_text(&h.workbook, ws, r, c)
                })
                .collect();
            let styles: Vec<Option<u32>> = along
                .iter()
                .map(|&a| {
                    let (r, c) = cell(a);
                    ws.cells.iter().find(|x| x.row == r && x.col == c).and_then(|x| x.style_id)
                })
                .collect();
            let edge = *along.last().unwrap() as i64;
            let n = if sign > 0 { count } else { count.min(edge) } as usize;
            let out = fill_line(&src, n, sign, vertical);
            for (i, text) in out.iter().enumerate() {
                let a = (edge + sign as i64 * (i as i64 + 1)) as u32;
                let (r, c) = cell(a);
                write_input(h, sheet, r, c, text)?;
                if let Some(id) = styles[(src.len() + i) % src.len()] {
                    let ws = h.sheet_mut(sheet).map_err(sheet_err)?;
                    ensure_cell(ws, r, c).style_id = Some(id);
                }
            }
        }
        h.dirty_sheets.insert(sheet);
        Ok(ChangeSet::new(ChangeKind::Cells, "fillRange").with_sheet(sheet))
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn catches_two_numbers_not_extending_as_a_linear_series() {
        assert_eq!(fill_line(&s(&["1", "3"]), 3, 1, true), s(&["5", "7", "9"]));
    }

    #[test]
    fn catches_lone_number_stepping_instead_of_copying() {
        assert_eq!(fill_line(&s(&["5"]), 2, 1, true), s(&["5", "5"]));
    }

    #[test]
    fn catches_text_suffix_and_weekday_series_copying() {
        assert_eq!(fill_line(&s(&["Item 08"]), 2, 1, true), s(&["Item 09", "Item 10"]));
        assert_eq!(fill_line(&s(&["Sat"]), 3, 1, true), s(&["Sun", "Mon", "Tue"]));
        assert_eq!(fill_line(&s(&["Mar"]), 2, -1, true), s(&["Feb", "Jan"]));
    }

    #[test]
    fn catches_filled_formula_keeping_its_original_references() {
        assert_eq!(fill_line(&s(&["=A1*$B$1"]), 2, 1, true), s(&["=A2*$B$1", "=A3*$B$1"]));
        assert_eq!(fill_line(&s(&["=A1+1"]), 1, 1, false), s(&["=B1+1"]));
        assert_eq!(fill_line(&s(&["=A2+1"]), 1, -1, true), s(&["=A1+1"]));
    }
}
