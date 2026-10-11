// SPDX-License-Identifier: MIT
#![cfg(not(target_arch = "wasm32"))]

//! Differential corpus: Rust `parse_cell_input` vs recorded upstream `parseCellInput` outputs.

use nitro_xml_xlsx::numfmt::{parse_cell_input, CellValue, FormatOptions};

#[test]
fn input_corpus_matches_upstream() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/input_corpus.json")).expect("corpus json");
    let mut diffs = 0usize;
    let mut shown = 0usize;
    for (i, row) in rows.iter().enumerate() {
        let text = row["text"].as_str().unwrap_or("");
        let date1904 = row["date1904"].as_bool().unwrap_or(false);
        let got = parse_cell_input(
            text,
            &FormatOptions {
                date1904,
                width: None,
                measure: None,
            },
        );
        let exp_value = cell_of(row);
        let exp_fmt = row["numFmt"].as_str();
        let exp_formula = row["formula"].as_str();
        let value_ok = values_close(&got.value, &exp_value);
        let fmt_ok = got.num_fmt.as_deref() == exp_fmt;
        let formula_ok = got.formula.as_deref() == exp_formula;
        if !value_ok || !fmt_ok || !formula_ok {
            diffs += 1;
            if shown < 16 {
                shown += 1;
                eprintln!(
                    "diff {i}: text {text:?} date1904 {date1904}\n  rust  value={:?} numFmt={:?} formula={:?}\n  expect value={:?} numFmt={:?} formula={:?}",
                    got.value, got.num_fmt, got.formula, exp_value, exp_fmt, exp_formula
                );
            }
        }
    }
    assert_eq!(
        diffs, 0,
        "{diffs} diffs against upstream parseCellInput corpus"
    );
    assert!(!rows.is_empty());
}

fn values_close(got: &CellValue, exp: &CellValue) -> bool {
    match (got, exp) {
        (CellValue::Number(a), CellValue::Number(b)) => {
            if (a.is_nan() && b.is_nan()) || (*a == 0.0 && *b == 0.0) {
                true
            } else {
                let scale = 1e9_f64.max(a.abs()).max(b.abs());
                (a - b).abs() <= scale * 1e-9 || (a - b).abs() <= 1e-9
            }
        }
        (a, b) => a == b,
    }
}

fn cell_of(row: &serde_json::Value) -> CellValue {
    match row["k"].as_str() {
        Some("n") => {
            let bits = u64::from_str_radix(row["b"].as_str().unwrap_or("0"), 16).expect("bits");
            CellValue::Number(f64::from_bits(bits))
        }
        Some("t") => CellValue::Text(row["s"].as_str().unwrap_or("").to_string()),
        Some("b") => CellValue::Bool(row["v"].as_bool().unwrap_or(false)),
        Some("e") => CellValue::Error(row["s"].as_str().unwrap_or("#VALUE!").to_string()),
        _ => CellValue::Blank,
    }
}
