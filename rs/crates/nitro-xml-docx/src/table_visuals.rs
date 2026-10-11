// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/table-visuals.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Value};

pub struct CellPlacement {
    pub row: usize,
    pub last_row: usize,
    pub column: usize,
    pub last_column: usize,
    pub row_count: usize,
    pub column_count: usize,
}

pub fn resolve_cell_visuals(
    table: &Value,
    cell: &Value,
    placement: &CellPlacement,
    _styles: Option<&Value>,
) -> Value {
    let shading_fill = cell.get("shadingFill").and_then(|v| v.as_str());
    let has_border_info = table.get("borders").is_some() || cell.get("borders").is_some();
    if !has_border_info {
        return if let Some(fill) = shading_fill {
            json!({ "shadingFill": fill })
        } else {
            json!({})
        };
    }
    let mut borders = json!({});
    for side in ["top", "bottom", "left", "right"] {
        let on_boundary = match side {
            "top" => placement.row == 0,
            "bottom" => placement.last_row == placement.row_count - 1,
            "left" => placement.column == 0,
            "right" => placement.last_column == placement.column_count - 1,
            _ => false,
        };
        let value = cell
            .get("borders")
            .and_then(|b| b.get(side))
            .or_else(|| {
                table.get("borders").and_then(|b| {
                    if on_boundary {
                        b.get(side)
                    } else if side == "top" || side == "bottom" {
                        b.get("insideH")
                    } else {
                        b.get("insideV")
                    }
                })
            });
        if let Some(v) = value {
            borders[side] = v.clone();
        } else {
            borders[side] = json!({ "style": "none" });
        }
    }
    let mut out = json!({ "borders": borders });
    if let Some(fill) = shading_fill {
        out["shadingFill"] = json!(fill);
    }
    out
}
