// SPDX-License-Identifier: MIT
#![cfg(not(target_arch = "wasm32"))]

//! 100_000 random (value, format) pairs compared with upstream `formatValue`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use nitro_xml_xlsx::numfmt::{format_value, CellValue, FormatOptions};

const PAIRS: usize = 100_000;

struct Rng(u64);

impl Rng {
    fn u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.u64() % n as u64) as usize
    }
}

#[test]
fn random_pairs_match_upstream_format_value() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/numfmt_corpus.json")).expect("corpus json");
    let mut formats: Vec<String> = rows
        .iter()
        .filter_map(|row| row["format"].as_str().map(str::to_string))
        .collect();
    formats.sort();
    formats.dedup();
    assert!(!formats.is_empty());

    let mut rng = Rng(0x17_0f_4a_7c_u64);
    let mut inputs: Vec<serde_json::Value> = Vec::with_capacity(PAIRS);
    for _ in 0..PAIRS {
        inputs.push(sample(&mut rng, &formats));
    }

    let dir = std::env::temp_dir().join(format!("numfmt-oracle-{}", std::process::id()));
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

    let mjs = upstream_mjs();
    let bun = bun_bin();
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
        let got = format_rust(row);
        let exp_text = expected["text"].as_str().unwrap_or("");
        let exp_color = expected["color"].as_str();
        if got.text != exp_text || got.color.as_deref() != exp_color {
            diffs += 1;
            if shown < 12 {
                shown += 1;
                eprintln!(
                    "diff {i}: input {row} rust {:?} color {:?} upstream {exp_text:?} color {exp_color:?}",
                    got.text, got.color
                );
            }
        }
    }
    let lines = output.lines().count();
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(lines, PAIRS, "oracle returned {lines} lines");
    assert_eq!(diffs, 0, "{diffs} diffs against upstream formatValue");
}

fn format_rust(row: &serde_json::Value) -> nitro_xml_xlsx::numfmt::FormattedValue {
    let value = cell_of(row);
    let format = row["f"].as_str().unwrap_or("");
    let date1904 = row["d"].as_bool().unwrap_or(false);
    let width = row.get("w").and_then(|v| v.as_f64());
    let factor = row.get("m").and_then(|v| v.as_f64());
    if let Some(factor) = factor {
        let measure = |text: &str| text.chars().count() as f64 * factor;
        format_value(
            &value,
            format,
            &FormatOptions {
                date1904,
                width,
                measure: Some(&measure),
            },
        )
    } else {
        format_value(
            &value,
            format,
            &FormatOptions {
                date1904,
                width,
                measure: None,
            },
        )
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

fn sample(rng: &mut Rng, formats: &[String]) -> serde_json::Value {
    let format = &formats[rng.below(formats.len())];
    let date1904 = rng.below(5) == 0;
    let mut row = serde_json::json!({ "f": format, "d": date1904 });
    match rng.below(16) {
        0..=10 => {
            let n = sample_f64(rng);
            row["k"] = serde_json::json!("n");
            row["b"] = serde_json::json!(format!("{:016x}", n.to_bits()));
            if rng.below(8) == 0 {
                row["w"] = serde_json::json!(1.0 + rng.below(24) as f64);
                if rng.below(3) == 0 {
                    row["m"] = serde_json::json!(if rng.below(2) == 0 { 1.0 } else { 7.0 });
                }
            }
        }
        11 => {
            row["k"] = serde_json::json!("t");
            row["s"] = serde_json::json!(sample_text(rng));
        }
        12 => {
            row["k"] = serde_json::json!("b");
            row["v"] = serde_json::json!(rng.below(2) == 0);
        }
        13 => {
            row["k"] = serde_json::json!("z");
        }
        _ => {
            row["k"] = serde_json::json!("e");
            const ERRORS: [&str; 7] = [
                "#DIV/0!", "#N/A", "#NAME?", "#NULL!", "#NUM!", "#REF!", "#VALUE!",
            ];
            row["s"] = serde_json::json!(ERRORS[rng.below(ERRORS.len())]);
        }
    }
    row
}

fn sample_f64(rng: &mut Rng) -> f64 {
    match rng.below(12) {
        0 => 0.0,
        1 => -0.0,
        2 => f64::NAN,
        3 => {
            if rng.below(2) == 0 {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            }
        }
        4 => (rng.below(200_001) as i64 - 100_000) as f64,
        5 => {
            let n = rng.below(1_000_000) as f64;
            let scale = 10f64.powi(rng.below(8) as i32);
            let v = n / scale;
            if rng.below(2) == 0 {
                -v
            } else {
                v
            }
        }
        6 => {
            let day = rng.below(60_000) as f64;
            let frac = rng.below(86_400) as f64 / 86_400.0;
            let v = day + frac;
            if rng.below(8) == 0 {
                -v
            } else {
                v
            }
        }
        7 => {
            let exp = rng.below(700) as i32 - 350;
            let mant = (rng.u64() >> 12) as f64 / ((1u64 << 52) as f64);
            let sign = if rng.below(2) == 0 { 1.0 } else { -1.0 };
            sign * mant * 10f64.powi(exp)
        }
        8 => f64::from_bits(rng.u64()),
        9 => {
            let digits = 15 + rng.below(6);
            let mut v = 0.0;
            for _ in 0..digits {
                v = v * 10.0 + rng.below(10) as f64;
            }
            if rng.below(2) == 0 {
                -v
            } else {
                v
            }
        }
        10 => f64::from_bits(rng.u64() & 0x000f_ffff_ffff_ffff),
        _ => (rng.below(10_000) as f64 - 5_000.0) / (1.0 + rng.below(100) as f64),
    }
}

fn sample_text(rng: &mut Rng) -> String {
    const ALPH: &[u8] = b"abcXYZ 0129.@#";
    let n = rng.below(12);
    (0..n)
        .map(|_| ALPH[rng.below(ALPH.len())] as char)
        .collect()
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

const { formatValue } = await import(pathToFileURL(process.argv[2]).href);
const input = readFileSync(process.argv[3], "utf8");
const lines = input.split("\n").filter((line) => line.length > 0);
const out = [];
for (const line of lines) {
  const row = JSON.parse(line);
  let value;
  if (row.k === "n") {
    const buf = new ArrayBuffer(8);
    new BigUint64Array(buf)[0] = BigInt("0x" + row.b);
    value = new Float64Array(buf)[0];
  } else if (row.k === "t") value = row.s;
  else if (row.k === "b") value = row.v;
  else if (row.k === "e") value = { error: row.s };
  else value = null;
  const opts = {};
  if (row.d) opts.date1904 = true;
  if (typeof row.w === "number") opts.width = row.w;
  if (typeof row.m === "number") opts.measure = (text) => text.length * row.m;
  const formatted = formatValue(value, row.f, opts);
  out.push(JSON.stringify({ text: formatted.text, color: formatted.color ?? null }));
}
writeFileSync(process.argv[4], out.join("\n") + "\n");
"#;
