// SPDX-License-Identifier: MIT

use nitro_xml_xlsx::numfmt::{format_value, CellValue, FormatOptions};

/// Display text for every corpus row equals the recorded Excel / upstream text.
#[test]
fn corpus_matches_recorded_display() {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/numfmt_corpus.json")).expect("corpus json");
    assert!(
        rows.len() > 900,
        "corpus has the excel cases plus leap-day rows"
    );
    for (i, row) in rows.iter().enumerate() {
        let value = cell_of(&row["value"]);
        let format = row["format"].as_str().unwrap_or("");
        let date1904 = row
            .get("date1904")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let width = row.get("width").and_then(|v| v.as_f64());
        let factor = row.get("measure").and_then(|v| v.as_f64());
        let expected = row["text"].as_str().expect("text");
        let expected_color = row.get("color").and_then(|v| v.as_str());
        let got = if let Some(factor) = factor {
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
        };
        assert_eq!(
            got.text, expected,
            "row {i} format {format:?} value {value:?} date1904 {date1904} width {width:?}"
        );
        if row.get("color").is_some() {
            assert_eq!(
                got.color.as_deref(),
                expected_color,
                "row {i} color format {format:?}"
            );
        }
    }
}

fn cell_of(v: &serde_json::Value) -> CellValue {
    match v {
        serde_json::Value::Null => CellValue::Blank,
        serde_json::Value::Bool(b) => CellValue::Bool(*b),
        serde_json::Value::Number(n) => {
            CellValue::Number(n.as_f64().expect("corpus number is an f64"))
        }
        serde_json::Value::String(s) => CellValue::Text(s.clone()),
        serde_json::Value::Object(map) => {
            if map.get("nan").and_then(|x| x.as_bool()) == Some(true) {
                CellValue::Number(f64::NAN)
            } else if let Some(inf) = map.get("inf").and_then(|x| x.as_i64()) {
                CellValue::Number(if inf < 0 {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                })
            } else if let Some(err) = map.get("error").and_then(|x| x.as_str()) {
                CellValue::Error(err.to_string())
            } else if let Some(bits) = map.get("bits").and_then(|x| x.as_str()) {
                let u = u64::from_str_radix(bits, 16).expect("hex f64 bits");
                CellValue::Number(f64::from_bits(u))
            } else {
                panic!("unknown corpus value {v}");
            }
        }
        serde_json::Value::Array(_) => panic!("array is not a cell value"),
    }
}
