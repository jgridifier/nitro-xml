// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/resolve-table.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Map, Value};

fn style_chain(style_id: Option<&str>, catalog: Option<&Value>) -> Vec<Value> {
    let style_id = match style_id {
        Some(id) => id,
        None => return Vec::new(),
    };
    let styles = catalog
        .and_then(|c| c.get("styles"))
        .and_then(|s| s.as_object());
    let styles = match styles {
        Some(s) => s,
        None => return Vec::new(),
    };
    let mut chain = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut current: Option<String> = Some(style_id.to_string());
    while let Some(id) = current {
        if visited.contains(&id) {
            break;
        }
        let style = match styles.get(&id).cloned() {
            Some(s) => s,
            None => break,
        };
        visited.insert(id);
        current = style
            .get("basedOn")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        chain.push(style);
    }
    chain.reverse();
    chain
}

fn look_bool(look: Option<&Value>, key: &str) -> bool {
    look.and_then(|l| l.get(key)).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn applicable_regions(
    look: Option<&Value>,
    row_index: usize,
    row_count: usize,
    col_index: usize,
    col_count: usize,
) -> Vec<&'static str> {
    let mut regions = vec!["wholeTable"];
    if !look_bool(look, "noVBand") {
        regions.push(if col_index % 2 == 0 {
            "band1Vert"
        } else {
            "band2Vert"
        });
    }
    if !look_bool(look, "noHBand") {
        regions.push(if row_index % 2 == 0 {
            "band1Horz"
        } else {
            "band2Horz"
        });
    }
    if look_bool(look, "lastColumn") && col_index == col_count - 1 {
        regions.push("lastCol");
    }
    if look_bool(look, "firstColumn") && col_index == 0 {
        regions.push("firstCol");
    }
    if look_bool(look, "lastRow") && row_index == row_count - 1 {
        regions.push("lastRow");
    }
    if look_bool(look, "firstRow") && row_index == 0 {
        regions.push("firstRow");
    }
    regions
}

fn merge_borders(into: &mut Map<String, Value>, layer: &Value) {
    if let Some(b) = layer.get("borders").and_then(|v| v.as_object()) {
        let target = into
            .entry("borders".to_string())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("borders object");
        for (k, v) in b {
            target.insert(k.clone(), v.clone());
        }
    }
}

pub fn resolve_table_style_formatting(
    table_style_id: Option<&str>,
    catalog: Option<&Value>,
    look: Option<&Value>,
    row_index: usize,
    row_count: usize,
    col_index: usize,
    col_count: usize,
) -> Value {
    let chain = style_chain(table_style_id, catalog);
    let regions = applicable_regions(look, row_index, row_count, col_index, col_count);
    let mut result = Map::new();
    for style in chain {
        if let Some(borders) = style.get("borders") {
            merge_borders(&mut result, &json!({ "borders": borders }));
        }
        if let Some(fill) = style.get("shadingFill") {
            result.insert("shadingFill".to_string(), fill.clone());
        }
        let conditional = style.get("conditional").and_then(|v| v.as_object());
        if let Some(conditional) = conditional {
            for region in &regions {
                if let Some(formatting) = conditional.get(*region) {
                    merge_borders(&mut result, formatting);
                    if let Some(fill) = formatting.get("shadingFill") {
                        result.insert("shadingFill".to_string(), fill.clone());
                    }
                    if let Some(theme_fill) = formatting.get("shadingThemeFill") {
                        result.insert("shadingThemeFill".to_string(), theme_fill.clone());
                    }
                    if let Some(run) = formatting.get("run") {
                        let merged = result
                            .entry("run".to_string())
                            .or_insert_with(|| json!({}))
                            .as_object_mut()
                            .expect("run object");
                        if let Some(layer) = run.as_object() {
                            for (k, v) in layer {
                                merged.insert(k.clone(), v.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    Value::Object(result)
}
