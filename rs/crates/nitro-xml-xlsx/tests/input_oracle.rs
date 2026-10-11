// SPDX-License-Identifier: MIT
#![cfg(not(target_arch = "wasm32"))]

//! Live differential: Rust `parse_cell_input` vs upstream `parseCellInput` (bun + built dist).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use nitro_xml_xlsx::numfmt::{parse_cell_input, CellValue, FormatOptions};

#[test]
fn parse_cell_input_matches_upstream_oracle() {
    let mjs = upstream_mjs();
    let bun = bun_bin();
    if !mjs.is_file() {
        eprintln!(
            "skip input oracle: missing {} (init/build upstream submodule)",
            mjs.display()
        );
        return;
    }
    if Command::new(&bun).arg("--version").output().is_err() {
        eprintln!("skip input oracle: bun not available ({bun})");
        return;
    }

    let corpus: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/input_corpus.json")).expect("corpus");
    let mut inputs: Vec<serde_json::Value> = corpus
        .iter()
        .map(|row| {
            serde_json::json!({
                "t": row["text"],
                "d": row["date1904"].as_bool().unwrap_or(false),
            })
        })
        .collect();
    // Extra strings that stress date/number boundaries.
    for t in [
        "Feb-30",
        "Sept 5",
        "1 1/2",
        "($1,234.50)",
        "2023-03-15T08:30",
        "12:00:00.0",
        "#VALUE!",
        "general",
    ] {
        inputs.push(serde_json::json!({ "t": t, "d": false }));
    }

    let dir = std::env::temp_dir().join(format!("input-oracle-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir");
    let input_path = dir.join("in.jsonl");
    let output_path = dir.join("out.jsonl");
    let script_path = dir.join("oracle.mjs");
    let mut body = String::new();
    for row in &inputs {
        body.push_str(&serde_json::to_string(row).expect("row"));
        body.push('\n');
    }
    fs::write(&input_path, body).expect("write input");
    fs::write(&script_path, ORACLE_JS).expect("write script");

    let status = Command::new(&bun)
        .arg(&script_path)
        .arg(&mjs)
        .arg(&input_path)
        .arg(&output_path)
        .status()
        .unwrap_or_else(|err| panic!("spawn {bun}: {err}"));
    assert!(status.success(), "bun oracle exited {status}");

    let output = fs::read_to_string(&output_path).expect("read oracle output");
    let mut diffs = 0usize;
    let mut shown = 0usize;
    for (i, (row, line)) in inputs.iter().zip(output.lines()).enumerate() {
        let expected: serde_json::Value = serde_json::from_str(line).expect("oracle line");
        let text = row["t"].as_str().unwrap_or("");
        let date1904 = row["d"].as_bool().unwrap_or(false);
        let got = parse_cell_input(
            text,
            &FormatOptions {
                date1904,
                width: None,
                measure: None,
            },
        );
        let exp_value = cell_of(&expected);
        let exp_fmt = expected["numFmt"].as_str();
        let exp_formula = expected["formula"].as_str();
        let value_ok = values_close(&got.value, &exp_value);
        let fmt_ok = got.num_fmt.as_deref() == exp_fmt;
        let formula_ok = got.formula.as_deref() == exp_formula;
        if !value_ok || !fmt_ok || !formula_ok {
            diffs += 1;
            if shown < 16 {
                shown += 1;
                eprintln!(
                    "diff {i}: text {text:?}\n  rust  {:?}/{:?}/{:?}\n  up    {:?}/{:?}/{:?}",
                    got.value, got.num_fmt, got.formula, exp_value, exp_fmt, exp_formula
                );
            }
        }
    }
    let lines = output.lines().count();
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(lines, inputs.len(), "oracle returned {lines} lines");
    assert_eq!(diffs, 0, "{diffs} diffs against upstream parseCellInput");
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

fn upstream_mjs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../upstream/src/core/dist/xlsx/index.mjs")
}

fn bun_bin() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let candidate = PathBuf::from(home).join(".bun/bin/bun");
    if candidate.is_file() {
        candidate.to_string_lossy().into_owned()
    } else {
        "bun".into()
    }
}

const ORACLE_JS: &str = r#"
import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const { parseCellInput } = await import(pathToFileURL(process.argv[2]).href);
const input = readFileSync(process.argv[3], "utf8");
const lines = input.split("\n").filter((line) => line.length > 0);
const out = [];
function enc(v) {
  if (v === null || v === undefined) return { k: "z" };
  if (typeof v === "number") {
    const buf = new ArrayBuffer(8);
    new Float64Array(buf)[0] = v;
    return { k: "n", b: new BigUint64Array(buf)[0].toString(16).padStart(16, "0") };
  }
  if (typeof v === "boolean") return { k: "b", v };
  if (typeof v === "object" && v && "error" in v) return { k: "e", s: v.error };
  return { k: "t", s: String(v) };
}
for (const line of lines) {
  const row = JSON.parse(line);
  const opts = {};
  if (row.d) opts.date1904 = true;
  const parsed = parseCellInput(row.t, opts);
  out.push(JSON.stringify({
    ...enc(parsed.value),
    numFmt: parsed.numFmt ?? null,
    formula: parsed.formula ?? null,
  }));
}
writeFileSync(process.argv[4], out.join("\n") + "\n");
"#;
