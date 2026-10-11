// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/flow-table.ts; changes listed in PROVENANCE.md.

use super::input::LayoutTable;
use super::measure::FakeMeasurer;
use super::page_cursor::PageCursor;
use super::result::{LayoutBlockBox, LayoutTableBox, LayoutTableRowBox};
use super::table_layout::{layout_row, split_row_at_height, RowLayout};

pub fn place_table(
    cursor: &mut PageCursor,
    table: &LayoutTable,
    measurer: &FakeMeasurer,
    note: &mut dyn FnMut(&str),
) {
    let first_body_row = table
        .rows
        .iter()
        .position(|row| !row.is_header.unwrap_or(false))
        .unwrap_or(table.rows.len());
    let header_count = first_body_row;
    let mut fragment_rows: Vec<LayoutTableRowBox> = Vec::new();
    let mut fragment_height = 0.0;

    let table_width = table.width_px.unwrap_or(cursor.column_width_px());
    let x_offset = match table.alignment.as_deref() {
        Some("center") => ((cursor.column_width_px() - table_width) / 2.0).max(0.0),
        Some("right") => (cursor.column_width_px() - table_width).max(0.0),
        _ => table.indent_px.unwrap_or(0.0),
    };

    for (row_index, row) in table.rows.iter().enumerate() {
        let cant_split = row.cant_split.unwrap_or(false) || row_index < header_count;
        let mut remaining_row: Option<RowLayout> = Some(layout_row(row, cursor.column_width_px(), measurer, note, None));
        while let Some(row_layout) = remaining_row.take() {
            let remaining = cursor.remaining_height_px() - fragment_height;
            if row_layout.height_px <= remaining {
                fragment_rows.push(LayoutTableRowBox {
                    y_px: fragment_height,
                    height_px: row_layout.height_px,
                    repeated: false,
                    cells: row_layout.cells,
                    geometry: Some(row_layout.geometry),
                });
                fragment_height += row_layout.height_px;
                continue;
            }
            if cant_split {
                if !fragment_rows.is_empty() || !cursor.at_column_top() {
                    if !fragment_rows.is_empty() {
                        cursor.place(
                            LayoutBlockBox::Table(LayoutTableBox {
                                kind: "table".into(),
                                block_id: table.id.clone(),
                                x_px: if x_offset != 0.0 { Some(x_offset) } else { None },
                                y_px: 0.0,
                                height_px: fragment_height,
                                rows: fragment_rows.clone(),
                            }),
                            fragment_height,
                        );
                        fragment_rows.clear();
                        fragment_height = 0.0;
                    }
                    cursor.new_column();
                    for header in table.rows.iter().take(header_count) {
                        let layout = layout_row(header, cursor.column_width_px(), measurer, note, None);
                        fragment_rows.push(LayoutTableRowBox {
                            y_px: fragment_height,
                            height_px: layout.height_px,
                            repeated: true,
                            cells: layout.cells,
                            geometry: Some(layout.geometry),
                        });
                        fragment_height += layout.height_px;
                    }
                    remaining_row = Some(layout_row(row, cursor.column_width_px(), measurer, note, None));
                    continue;
                }
                if row_layout.height_px > cursor.remaining_height_px() - fragment_height {
                    note("A table row marked \"keep row together\" is taller than an empty page/column.");
                }
                fragment_rows.push(LayoutTableRowBox {
                    y_px: fragment_height,
                    height_px: row_layout.height_px,
                    repeated: false,
                    cells: row_layout.cells,
                    geometry: Some(row_layout.geometry),
                });
                fragment_height += row_layout.height_px;
                continue;
            }
            let row_height_px = row_layout.height_px;
            let row_cells = row_layout.cells.clone();
            let row_geometry = row_layout.geometry.clone();
            let split = split_row_at_height(row_layout, remaining.max(0.0));
            if !split.before.cells.iter().any(|c| !c.is_empty())
                && cursor.at_column_top()
                && fragment_rows.iter().all(|r| r.repeated)
            {
                note("A table row cannot fit a line in an empty page/column and was placed without splitting.");
                fragment_rows.push(LayoutTableRowBox {
                    y_px: fragment_height,
                    height_px: row_height_px,
                    repeated: false,
                    cells: row_cells,
                    geometry: Some(row_geometry),
                });
                fragment_height += row_height_px;
                continue;
            }
            let previous_width = cursor.column_width_px();
            if split.before.cells.iter().any(|c| !c.is_empty()) {
                fragment_rows.push(LayoutTableRowBox {
                    y_px: fragment_height,
                    height_px: split.before.height_px,
                    repeated: false,
                    cells: split.before.cells,
                    geometry: Some(split.before.geometry),
                });
                fragment_height += split.before.height_px;
            }
            if !fragment_rows.is_empty() {
                cursor.place(
                    LayoutBlockBox::Table(LayoutTableBox {
                        kind: "table".into(),
                        block_id: table.id.clone(),
                        x_px: if x_offset != 0.0 { Some(x_offset) } else { None },
                        y_px: 0.0,
                        height_px: fragment_height,
                        rows: fragment_rows.clone(),
                    }),
                    fragment_height,
                );
                fragment_rows.clear();
                fragment_height = 0.0;
            }
            cursor.new_column();
            for header in table.rows.iter().take(header_count) {
                let layout = layout_row(header, cursor.column_width_px(), measurer, note, None);
                fragment_rows.push(LayoutTableRowBox {
                    y_px: fragment_height,
                    height_px: layout.height_px,
                    repeated: true,
                    cells: layout.cells,
                    geometry: Some(layout.geometry),
                });
                fragment_height += layout.height_px;
            }
            remaining_row = split.after.map(|after| {
                if previous_width != cursor.column_width_px() {
                    let mut patched = row.clone();
                    patched.height_px = Some(0.0);
                    patched.height_rule = Some("atLeast".into());
                    layout_row(&patched, cursor.column_width_px(), measurer, note, Some(&after.cells))
                } else {
                    after
                }
            });
        }
    }
    if !fragment_rows.is_empty() {
        cursor.place(
            LayoutBlockBox::Table(LayoutTableBox {
                kind: "table".into(),
                block_id: table.id.clone(),
                x_px: if x_offset != 0.0 { Some(x_offset) } else { None },
                y_px: 0.0,
                height_px: fragment_height,
                rows: fragment_rows,
            }),
            fragment_height,
        );
    }
}
