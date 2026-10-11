// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/table-visuals.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use crate::resolve_table::resolve_table_style_formatting;

pub struct CellPlacement {
    pub row: usize,
    pub last_row: usize,
    pub column: usize,
    pub last_column: usize,
    pub row_count: usize,
    pub column_count: usize,
}

fn edge_from(borders: Option<&Value>, side: &str, on_boundary: bool) -> Option<Value> {
    let borders = borders?;
    if on_boundary {
        borders.get(side).cloned()
    } else if side == "top" || side == "bottom" {
        borders.get("insideH").cloned()
    } else {
        borders.get("insideV").cloned()
    }
}

pub fn resolve_cell_visuals(
    table: &Value,
    cell: &Value,
    placement: &CellPlacement,
    styles: Option<&Value>,
) -> Value {
    let styled = if table.get("style").and_then(|v| v.as_str()).is_some() {
        resolve_table_style_formatting(
            table.get("style").and_then(|v| v.as_str()),
            styles,
            table.get("look"),
            placement.row,
            placement.row_count,
            placement.column,
            placement.column_count,
        )
    } else {
        json!({})
    };
    let has_border_info = styled.get("borders").is_some()
        || table.get("borders").is_some()
        || cell.get("borders").is_some();
    let shading_fill = cell
        .get("shadingFill")
        .and_then(|v| v.as_str())
        .or_else(|| styled.get("shadingFill").and_then(|v| v.as_str()));
    if !has_border_info {
        return if let Some(fill) = shading_fill {
            json!({ "shadingFill": fill })
        } else {
            json!({})
        };
    }
    let boundary = |side: &str| -> bool {
        match side {
            "top" => placement.row == 0,
            "bottom" => placement.last_row == placement.row_count - 1,
            "left" => placement.column == 0,
            "right" => placement.last_column == placement.column_count - 1,
            _ => false,
        }
    };
    let none = json!({ "style": "none" });
    let mut borders = serde_json::Map::new();
    for side in ["top", "bottom", "left", "right"] {
        let resolved = cell
            .get("borders")
            .and_then(|b| b.get(side))
            .cloned()
            .or_else(|| edge_from(table.get("borders"), side, boundary(side)))
            .or_else(|| edge_from(styled.get("borders"), side, boundary(side)))
            .unwrap_or_else(|| none.clone());
        borders.insert(side.into(), resolved);
    }
    let mut out = json!({ "borders": borders });
    if let Some(fill) = shading_fill {
        out["shadingFill"] = json!(fill);
    }
    out
}
