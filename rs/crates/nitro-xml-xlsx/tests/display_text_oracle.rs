// SPDX-License-Identifier: MIT
#![cfg(not(target_arch = "wasm32"))]

//! Differential: Rust `display_text` vs upstream `displayText` over synthetic workbooks.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use nitro_xml_xlsx::display_text::{display_text, Cell};
use nitro_xml_xlsx::model::{RichTextRun, WorkbookSlice};
use nitro_xml_xlsx::numfmt::CellValue;
use nitro_xml_xlsx::workbook::{default_cell_style, default_theme};

#[test]
fn display_text_matches_upstream() {
    let mjs = upstream_mjs();
    let bun = bun_bin();
    if !mjs.is_file() {
        eprintln!(
            "skip display_text oracle: missing {} (init/build upstream)",
            mjs.display()
        );
        return;
    }
    if Command::new(&bun).arg("--version").output().is_err() {
        eprintln!("skip display_text oracle: bun not available ({bun})");
        return;
    }

    let cases = synthetic_cases();
    let dir = std::env::temp_dir().join(format!("display-oracle-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp");
    let input_path = dir.join("in.jsonl");
    let output_path = dir.join("out.jsonl");
    let script_path = dir.join("oracle.mjs");
    let mut body = String::new();
    for case in &cases {
        body.push_str(&serde_json::to_string(&case.payload).expect("row"));
        body.push('\n');
    }
    fs::write(&input_path, body).expect("write");
    fs::write(&script_path, ORACLE_JS).expect("script");

    let status = Command::new(&bun)
        .arg(&script_path)
        .arg(&mjs)
        .arg(&input_path)
        .arg(&output_path)
        .status()
        .unwrap_or_else(|err| panic!("spawn {bun}: {err}"));
    assert!(status.success(), "bun oracle exited {status}");

    let output = fs::read_to_string(&output_path).expect("out");
    let mut diffs = 0usize;
    for (i, (case, line)) in cases.iter().zip(output.lines()).enumerate() {
        let expected: serde_json::Value = serde_json::from_str(line).expect("line");
        let exp = expected["text"].as_str().unwrap_or("");
        let got = display_text(&case.book, case.cell.as_ref());
        if got != exp {
            diffs += 1;
            eprintln!(
                "diff {i}: rust {got:?} upstream {exp:?} payload {}",
                case.payload
            );
        }
    }
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(diffs, 0, "{diffs} diffs against upstream displayText");
    assert_eq!(output.lines().count(), cases.len());
}

struct Case {
    book: WorkbookSlice,
    cell: Option<Cell>,
    payload: serde_json::Value,
}

fn synthetic_cases() -> Vec<Case> {
    let mut out = Vec::new();
    out.push(case(
        false,
        "General",
        None,
        serde_json::json!({ "absent": true }),
    ));
    out.push(case(
        false,
        "General",
        Some(Cell {
            value: CellValue::Blank,
            style_id: None,
            rich_text: None,
        }),
        serde_json::json!({ "k": "z" }),
    ));
    out.push(case(
        false,
        "General",
        Some(Cell {
            value: CellValue::Bool(true),
            style_id: None,
            rich_text: None,
        }),
        serde_json::json!({ "k": "b", "v": true }),
    ));
    out.push(case(
        false,
        "General",
        Some(Cell {
            value: CellValue::Error("#N/A".into()),
            style_id: None,
            rich_text: None,
        }),
        serde_json::json!({ "k": "e", "s": "#N/A" }),
    ));
    out.push(case(
        false,
        "0.00",
        Some(Cell {
            value: CellValue::Number(1.5),
            style_id: Some(1),
            rich_text: None,
        }),
        serde_json::json!({ "k": "n", "n": 1.5, "styleId": 1, "numFmt": "0.00" }),
    ));
    out.push(case(
        false,
        "yyyy-mm-dd",
        Some(Cell {
            value: CellValue::Number(45000.0),
            style_id: Some(1),
            rich_text: None,
        }),
        serde_json::json!({ "k": "n", "n": 45000.0, "styleId": 1, "numFmt": "yyyy-mm-dd" }),
    ));
    out.push(case(
        true,
        "yyyy-mm-dd",
        Some(Cell {
            value: CellValue::Number(1.0),
            style_id: Some(1),
            rich_text: None,
        }),
        serde_json::json!({
            "k": "n", "n": 1.0, "styleId": 1, "numFmt": "yyyy-mm-dd", "date1904": true
        }),
    ));
    out.push(case(
        false,
        "0.00",
        Some(Cell {
            value: CellValue::Text("001".into()),
            style_id: Some(1),
            rich_text: Some(vec![RichTextRun {
                text: "001".into(),
                font: None,
            }]),
        }),
        serde_json::json!({
            "k": "t", "s": "001", "styleId": 1, "numFmt": "0.00", "rich": true
        }),
    ));
    out.push(case(
        false,
        "0%",
        Some(Cell {
            value: CellValue::Number(0.25),
            style_id: Some(1),
            rich_text: None,
        }),
        serde_json::json!({ "k": "n", "n": 0.25, "styleId": 1, "numFmt": "0%" }),
    ));
    out
}

fn case(date1904: bool, num_fmt: &str, cell: Option<Cell>, mut payload: serde_json::Value) -> Case {
    let mut style0 = default_cell_style();
    let mut style1 = default_cell_style();
    style1.num_fmt = num_fmt.into();
    // Keep style0 General for absent styleId paths.
    let _ = &mut style0;
    let book = WorkbookSlice {
        format: "xlsx".into(),
        date1904,
        active_sheet: 0,
        sheets: vec![],
        defined_names: vec![],
        theme: default_theme(),
        styles: vec![style0, style1],
        named_styles: vec![],
        xf_map: vec![0, 1],
        palette: None,
        dxfs: vec![],
        shared_strings: vec![],
        warnings: vec![],
        full_calc_on_load: None,
        structure_locked: None,
        calc_mode: None,
        iterate: None,
        workbook_password_hash: None,
        workbook_modern_hash: None,
        active_worksheet: None,
    };
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("date1904".into(), serde_json::json!(date1904));
        if !obj.contains_key("numFmt") {
            obj.insert("numFmt".into(), serde_json::json!(num_fmt));
        }
    }
    Case {
        book,
        cell,
        payload,
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
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// displayText is not re-exported from the built xlsx bundle; load the upstream source.
// argv[2] is .../src/core/dist/xlsx/index.mjs → .../src/core
const coreRoot = join(dirname(fileURLToPath(pathToFileURL(process.argv[2]).href)), "../..");
const { displayText } = await import(pathToFileURL(join(coreRoot, "xlsx/display-text.ts")).href);
const input = readFileSync(process.argv[3], "utf8");
const lines = input.split("\n").filter((line) => line.length > 0);
const out = [];

function cellValue(row) {
  if (row.absent) return undefined;
  if (row.k === "z") return { value: null };
  if (row.k === "n") return {
    value: row.n,
    styleId: row.styleId,
    richText: row.rich ? [{ text: String(row.n) }] : undefined,
  };
  if (row.k === "t") return {
    value: row.s,
    styleId: row.styleId,
    richText: row.rich ? [{ text: row.s }] : undefined,
  };
  if (row.k === "b") return { value: row.v, styleId: row.styleId };
  if (row.k === "e") return { value: { error: row.s }, styleId: row.styleId };
  return { value: null };
}

for (const line of lines) {
  const row = JSON.parse(line);
  const style0 = { font: {}, fill: { type: "pattern", pattern: "none" }, border: {}, numFmt: "General" };
  const style1 = { ...style0, numFmt: row.numFmt ?? "General" };
  const workbook = {
    date1904: !!row.date1904,
    styles: [style0, style1],
  };
  const cell = cellValue(row);
  out.push(JSON.stringify({ text: displayText(workbook, cell) }));
}
writeFileSync(process.argv[4], out.join("\n") + "\n");
"#;
