// SPDX-License-Identifier: MIT
//! Formula reference scan used to mark dependents stale (no recalculation in E1).

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use regex::Regex;
use std::sync::OnceLock;

use crate::address::parse_address;
use crate::model::CellRange;

#[derive(Debug)]
pub struct FormulaRef {
    pub sheet_name: Option<String>,
    pub range: CellRange,
}

pub fn formula_refs(formula: &str) -> Vec<FormulaRef> {
    let blanked = blank_strings(formula);
    let mut out = Vec::new();
    for caps in cell_ref_re().captures_iter(&blanked) {
        let start = caps.get(0).map(|m| m.start()).unwrap_or(0);
        if start > 0 {
            let prev = blanked[..start].chars().next_back();
            if prev.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
                continue;
            }
        }
        let sheet = caps
            .get(1)
            .map(|m| m.as_str().replace("''", "'"))
            .or_else(|| caps.get(2).map(|m| m.as_str().to_owned()));
        let Some(start_addr) = caps.get(3).and_then(|m| parse_address(m.as_str())) else {
            continue;
        };
        let end_addr = caps
            .get(4)
            .and_then(|m| parse_address(m.as_str()))
            .unwrap_or(start_addr);
        out.push(FormulaRef {
            sheet_name: sheet,
            range: CellRange {
                start: start_addr,
                end: end_addr,
            },
        });
    }
    out
}

/// Parsed references per formula text. Edits re-run the stale scan on every keystroke; the
/// regex pass is the expensive part and a formula's references never change, so they are
/// parsed once per distinct text.
#[derive(Debug, Default, Clone)]
pub struct RefCache(HashMap<String, Arc<[FormulaRef]>>);

impl RefCache {
    fn refs(&mut self, formula: &str) -> Arc<[FormulaRef]> {
        if let Some(hit) = self.0.get(formula) {
            return Arc::clone(hit);
        }
        let parsed: Arc<[FormulaRef]> = formula_refs(formula).into();
        self.0.insert(formula.to_owned(), Arc::clone(&parsed));
        parsed
    }

    /// Drop entries once the cache holds far more texts than the workbook has formulas.
    fn trim(&mut self, live: usize) {
        if self.0.len() > live.saturating_mul(2) + 64 {
            self.0.clear();
        }
    }
}

/// Formula cells (sheet, row, col) that are edited themselves or read an edited cell.
pub fn stale_formula_cells<'a>(
    cache: &mut RefCache,
    sheet_names: &[&str],
    formulas: impl Iterator<Item = (u32, u32, u32, &'a str)>,
    precedents: &BTreeSet<(u32, u32, u32)>,
) -> BTreeSet<(u32, u32, u32)> {
    let mut stale = BTreeSet::new();
    let mut live = 0;
    for (sheet, row, col, formula) in formulas {
        live += 1;
        let edited = precedents.contains(&(sheet, row, col));
        if edited
            || cache
                .refs(formula)
                .iter()
                .any(|r| ref_hits(sheet_names, sheet, r, precedents))
        {
            stale.insert((sheet, row, col));
        }
    }
    cache.trim(live);
    stale
}

fn ref_hits(
    sheet_names: &[&str],
    current: u32,
    formula_ref: &FormulaRef,
    precedents: &BTreeSet<(u32, u32, u32)>,
) -> bool {
    let target = if let Some(name) = &formula_ref.sheet_name {
        match sheet_names
            .iter()
            .position(|sheet| sheet.eq_ignore_ascii_case(name))
        {
            Some(index) => index as u32,
            None => return false,
        }
    } else {
        current
    };
    let r0 = formula_ref.range.start.row.min(formula_ref.range.end.row);
    let r1 = formula_ref.range.start.row.max(formula_ref.range.end.row);
    let c0 = formula_ref.range.start.col.min(formula_ref.range.end.col);
    let c1 = formula_ref.range.start.col.max(formula_ref.range.end.col);
    precedents
        .range((target, r0, 0)..=(target, r1, u32::MAX))
        .any(|(_, _, col)| *col >= c0 && *col <= c1)
}

fn blank_strings(formula: &str) -> String {
    let chars: Vec<char> = formula.chars().collect();
    let mut out = String::with_capacity(formula.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '"' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        out.push(' ');
        i += 1;
        while i < chars.len() {
            if chars[i] == '"' {
                if chars.get(i + 1) == Some(&'"') {
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                }
                out.push(' ');
                i += 1;
                break;
            }
            out.push(' ');
            i += 1;
        }
    }
    out
}

fn cell_ref_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)(?:(?:'((?:[^']|'')*)'|([A-Za-z_][A-Za-z0-9_.]*))!)?(\$?[A-Za-z]{1,3}\$?[0-9]{1,7})(?::(\$?[A-Za-z]{1,3}\$?[0-9]{1,7}))?"#,
        )
        .expect("cell ref regex")
    })
}
