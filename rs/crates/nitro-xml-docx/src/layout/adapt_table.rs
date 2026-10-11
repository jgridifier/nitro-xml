// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/adapt-table.ts; changes listed in PROVENANCE.md.

use serde_json::Value;

use crate::body::DocumentModel;
use crate::table_visuals::{resolve_cell_visuals, CellPlacement};
use crate::theme_color::resolve_theme_color_reference;

use super::input::{
    LayoutBorder, LayoutCellBorders, LayoutCellPadding, LayoutParagraph, LayoutTable,
    LayoutTableCell, LayoutTableRow,
};
use super::units::twips_f64_to_px;

pub fn css_hex(value: Option<&str>) -> Option<String> {
    let hex = value?.replace('#', "");
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("#{}", hex))
    } else {
        None
    }
}

pub fn border(side: Option<&Value>, theme: Option<&Value>) -> Option<LayoutBorder> {
    let side = side?;
    let style = side.get("style").and_then(|v| v.as_str())?;
    if style == "none" || style == "nil" {
        return None;
    }
    let resolved_style = if style.starts_with("double") {
        "double"
    } else if style.to_ascii_lowercase().contains("dot") {
        "dotted"
    } else if style.to_ascii_lowercase().contains("dash") {
        "dashed"
    } else {
        "solid"
    };
    let themed = side
        .get("themeColor")
        .and_then(|tc| theme.and_then(|t| resolve_theme_color_reference(tc, t)));
    let hex = css_hex(side.get("color").and_then(|v| v.as_str()));
    let width_px = ((side.get("sizeEighthPoints").and_then(|v| v.as_f64()).unwrap_or(4.0) / 8.0)
        * 96.0
        / 72.0)
        .max(if resolved_style == "double" { 3.0 } else { 1.0 });
    Some(LayoutBorder {
        width_px,
        style: resolved_style.into(),
        color: themed.or(hex).unwrap_or_else(|| "#000000".into()),
    })
}

pub fn adapt_table(
    table: &Value,
    model: &DocumentModel,
    adapt_paragraph: &mut dyn FnMut(&Value) -> LayoutParagraph,
) -> LayoutTable {
    let grid = table
        .get("grid")
        .and_then(|v| v.as_array())
        .map(|cols| cols.iter().filter_map(|c| c.as_f64().map(twips_f64_to_px)).collect::<Vec<_>>());
    let column_count = grid
        .as_ref()
        .map(|g| g.len())
        .unwrap_or_else(|| {
            table
                .get("rows")
                .and_then(|v| v.as_array())
                .map(|rows| rows.iter().map(|r| r.as_array().map(|a| a.len()).unwrap_or(0)).max().unwrap_or(1))
                .unwrap_or(1)
        });
    let rows = table.get("rows").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let grid_indexes: Vec<Vec<usize>> = rows
        .iter()
        .map(|row| {
            let mut index = 0usize;
            row.as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|cell| {
                    let start = index;
                    index += cell.get("gridSpan").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                    start
                })
                .collect()
        })
        .collect();
    LayoutTable {
        kind: "table".into(),
        id: table.get("id").and_then(|v| v.as_str()).unwrap_or("").into(),
        width_px: grid.as_ref().map(|g| g.iter().sum()),
        indent_px: table
            .get("indentTwips")
            .and_then(|v| v.as_f64())
            .map(twips_f64_to_px),
        alignment: table
            .get("alignment")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        rows: rows
            .iter()
            .enumerate()
            .map(|(row_index, row)| {
                let row_props = table
                    .get("rowProperties")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.get(row_index));
                LayoutTableRow {
                    cant_split: row_props.and_then(|p| p.get("cantSplit")).and_then(|v| v.as_bool()),
                    height_px: row_props
                        .and_then(|p| p.get("heightTwips"))
                        .and_then(|v| v.as_f64())
                        .map(twips_f64_to_px),
                    height_rule: row_props
                        .and_then(|p| p.get("heightRule"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    is_header: row_props.and_then(|p| p.get("header")).and_then(|v| v.as_bool()),
                    cells: row
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .enumerate()
                        .map(|(cell_index, cell)| {
                            let start = grid_indexes[row_index][cell_index];
                            let span = cell.get("gridSpan").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                            let placement = CellPlacement {
                                row: row_index,
                                last_row: row_index,
                                column: start,
                                last_column: start + span - 1,
                                row_count: rows.len(),
                                column_count,
                            };
                            let visuals = resolve_cell_visuals(
                                table,
                                cell,
                                &placement,
                                model.table_styles.as_ref(),
                            );
                            let mut borders = LayoutCellBorders {
                                top: None,
                                right: None,
                                bottom: None,
                                left: None,
                            };
                            if let Some(b) = visuals.get("borders") {
                                for side in ["top", "right", "bottom", "left"] {
                                    if let Some(resolved) = border(b.get(side), model.theme.as_ref()) {
                                        match side {
                                            "top" => borders.top = Some(resolved),
                                            "right" => borders.right = Some(resolved),
                                            "bottom" => borders.bottom = Some(resolved),
                                            "left" => borders.left = Some(resolved),
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            let margins = cell.get("margins");
                            let padding = LayoutCellPadding {
                                top: twips_f64_to_px(margins.and_then(|m| m.get("top")).and_then(|v| v.as_f64()).unwrap_or(0.0)),
                                right: twips_f64_to_px(
                                    margins.and_then(|m| m.get("right")).and_then(|v| v.as_f64()).unwrap_or(108.0),
                                ),
                                bottom: twips_f64_to_px(
                                    margins.and_then(|m| m.get("bottom")).and_then(|v| v.as_f64()).unwrap_or(0.0),
                                ),
                                left: twips_f64_to_px(
                                    margins.and_then(|m| m.get("left")).and_then(|v| v.as_f64()).unwrap_or(108.0),
                                ),
                            };
                            let width = grid.as_ref().map(|g| g[start..start + span].iter().sum());
                            let x_px = grid.as_ref().map(|g| g[..start].iter().sum());
                            LayoutTableCell {
                                paragraphs: cell
                                    .get("paragraphs")
                                    .and_then(|v| v.as_array())
                                    .map(|ps| ps.iter().map(|p| adapt_paragraph(p)).collect())
                                    .unwrap_or_default(),
                                width_px: width.or_else(|| {
                                    cell.get("widthTwips").and_then(|v| v.as_f64()).map(twips_f64_to_px)
                                }),
                                x_px,
                                padding: Some(padding),
                                borders: if borders.top.is_some()
                                    || borders.right.is_some()
                                    || borders.bottom.is_some()
                                    || borders.left.is_some()
                                {
                                    Some(borders)
                                } else {
                                    None
                                },
                                shading: css_hex(visuals.get("shadingFill").and_then(|v| v.as_str())),
                                vertical_align: cell
                                    .get("verticalAlign")
                                    .and_then(|v| v.as_str())
                                    .map(|v| if v == "both" { "top".into() } else { v.into() }),
                            }
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}
