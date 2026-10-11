// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/notes.ts (formatNoteNumber, numberNotesInOrder); changes listed in PROVENANCE.md.

use std::collections::HashMap;

use serde_json::Value;

pub fn format_note_number(value: i32, format: Option<&str>) -> String {
    match format {
        Some("upperRoman") => to_roman(value).to_uppercase(),
        Some("lowerRoman") => to_roman(value).to_lowercase(),
        Some("upperLetter") => to_letters(value).to_uppercase(),
        Some("lowerLetter") => to_letters(value),
        Some("decimalZero") => {
            if value < 10 {
                format!("0{}", value)
            } else {
                value.to_string()
            }
        }
        _ => value.to_string(),
    }
}

fn to_roman(value: i32) -> String {
    let numerals = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut remaining = value.max(1);
    let mut result = String::new();
    for (amount, numeral) in numerals {
        while remaining >= amount {
            result.push_str(numeral);
            remaining -= amount;
        }
    }
    if result.is_empty() {
        "I".into()
    } else {
        result
    }
}

fn to_letters(value: i32) -> String {
    let mut remaining = value.max(1);
    let mut result = String::new();
    while remaining > 0 {
        remaining -= 1;
        result.insert(0, (b'a' + (remaining % 26) as u8) as char);
        remaining /= 26;
    }
    if result.is_empty() {
        "a".into()
    } else {
        result
    }
}

pub fn number_notes_in_order(blocks: &[Value], kind: &str) -> HashMap<String, i32> {
    let mut order = HashMap::new();
    walk_blocks(blocks, kind, &mut order);
    order
}

fn walk_blocks(blocks: &[Value], kind: &str, order: &mut HashMap<String, i32>) {
    for block in blocks {
        if block.get("type").and_then(|v| v.as_str()) == Some("paragraph") {
            walk_paragraph(block, kind, order);
        } else if let Some(rows) = block.get("rows").and_then(|v| v.as_array()) {
            for row in rows {
                if let Some(cells) = row.as_array() {
                    for cell in cells {
                        if let Some(paragraphs) = cell.get("paragraphs").and_then(|v| v.as_array()) {
                            walk_blocks(paragraphs, kind, order);
                        }
                    }
                }
            }
        }
    }
}

fn walk_paragraph(paragraph: &Value, kind: &str, order: &mut HashMap<String, i32>) {
    if let Some(runs) = paragraph.get("runs").and_then(|v| v.as_array()) {
        for run in runs {
            if let Some(reference) = run.get("noteReference") {
                if reference.get("kind").and_then(|v| v.as_str()) == Some(kind) {
                    if let Some(id) = reference.get("id").and_then(|v| v.as_str()) {
                        if !order.contains_key(id) {
                            order.insert(id.to_string(), order.len() as i32 + 1);
                        }
                    }
                }
            }
        }
    }
}
