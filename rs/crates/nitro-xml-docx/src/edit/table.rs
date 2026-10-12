// SPDX-License-Identifier: MIT
//! Table cell text and row/column structure edits (#79).
//!
//! Matches `parity/edit/apply.mjs` / `parity/undo/session.mjs` docx table ops.

use std::any::Any;

use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde_json::{json, Value};

use super::host::DocxHost;

fn host_mut(host: &mut dyn Any) -> Result<&mut DocxHost, EditError> {
    host.downcast_mut::<DocxHost>()
        .ok_or_else(|| EditError::Other {
            message: "expected DocxHost".into(),
        })
}

fn block_index(args: &Value) -> Result<usize, EditError> {
    args.get("blockIndex")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .ok_or_else(|| EditError::Other {
            message: "missing blockIndex".into(),
        })
}

fn empty_cell(id: &str) -> Value {
    json!({
        "paragraphs": [{
            "type": "paragraph",
            "id": id,
            "runs": [{ "text": "" }],
        }],
    })
}

/// Set the first paragraph of a table cell to a single plain run (TS `setTableCellText`).
pub fn set_table_cell_text(
    host: &mut DocxHost,
    block_index: usize,
    row: usize,
    col: usize,
    text: &str,
) -> Result<(), EditError> {
    let table = host
        .table_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let rows = table
        .get_mut("rows")
        .and_then(|r| r.as_array_mut())
        .ok_or_else(|| EditError::Other {
            message: format!("table {block_index} missing rows"),
        })?;
    let row_arr = rows.get_mut(row).ok_or_else(|| EditError::Other {
        message: format!("row {row} missing"),
    })?;
    let cells = row_arr.as_array_mut().ok_or_else(|| EditError::Other {
        message: format!("row {row} not array"),
    })?;
    let cell = cells.get_mut(col).ok_or_else(|| EditError::Other {
        message: format!("cell {row},{col} missing"),
    })?;
    let para = json!({
        "type": "paragraph",
        "id": format!("p-{row}-{col}"),
        "runs": [{ "text": text }],
    });
    if let Some(obj) = cell.as_object_mut() {
        let paragraphs = obj
            .entry("paragraphs")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| EditError::Other {
                message: "paragraphs not array".into(),
            })?;
        if paragraphs.is_empty() {
            paragraphs.push(para);
        } else {
            paragraphs[0] = para;
        }
    }
    host.mark_touched(block_index);
    Ok(())
}

pub fn insert_table_row(
    host: &mut DocxHost,
    block_index: usize,
    at: usize,
) -> Result<(), EditError> {
    let table = host
        .table_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let rows = table
        .get_mut("rows")
        .and_then(|r| r.as_array_mut())
        .ok_or_else(|| EditError::Other {
            message: "table missing rows".into(),
        })?;
    let cols = rows
        .first()
        .and_then(|r| r.as_array())
        .map(|c| c.len())
        .unwrap_or(1);
    let row: Vec<Value> = (0..cols)
        .map(|c| empty_cell(&format!("r{at}-c{c}")))
        .collect();
    let at = at.min(rows.len());
    rows.insert(at, Value::Array(row));
    host.mark_touched(block_index);
    Ok(())
}

pub fn delete_table_row(
    host: &mut DocxHost,
    block_index: usize,
    at: usize,
) -> Result<(), EditError> {
    let table = host
        .table_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let rows = table
        .get_mut("rows")
        .and_then(|r| r.as_array_mut())
        .ok_or_else(|| EditError::Other {
            message: "table missing rows".into(),
        })?;
    if rows.len() <= 1 {
        return Err(EditError::Other {
            message: "cannot delete last table row".into(),
        });
    }
    if at >= rows.len() {
        return Err(EditError::Other {
            message: format!("row {at} missing"),
        });
    }
    rows.remove(at);
    host.mark_touched(block_index);
    Ok(())
}

pub fn insert_table_column(
    host: &mut DocxHost,
    block_index: usize,
    at: usize,
) -> Result<(), EditError> {
    let table = host
        .table_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let rows = table
        .get_mut("rows")
        .and_then(|r| r.as_array_mut())
        .ok_or_else(|| EditError::Other {
            message: "table missing rows".into(),
        })?;
    for (ri, row) in rows.iter_mut().enumerate() {
        let cells = row.as_array_mut().ok_or_else(|| EditError::Other {
            message: format!("row {ri} not array"),
        })?;
        let at = at.min(cells.len());
        cells.insert(at, empty_cell(&format!("r{ri}-c{at}")));
    }
    host.mark_touched(block_index);
    Ok(())
}

pub fn delete_table_column(
    host: &mut DocxHost,
    block_index: usize,
    at: usize,
) -> Result<(), EditError> {
    let table = host
        .table_mut(block_index)
        .map_err(|e| EditError::Other { message: e })?;
    let rows = table
        .get_mut("rows")
        .and_then(|r| r.as_array_mut())
        .ok_or_else(|| EditError::Other {
            message: "table missing rows".into(),
        })?;
    let cols = rows
        .first()
        .and_then(|r| r.as_array())
        .map(|c| c.len())
        .unwrap_or(0);
    if cols <= 1 {
        return Err(EditError::Other {
            message: "cannot delete last table column".into(),
        });
    }
    if at >= cols {
        return Err(EditError::Other {
            message: format!("col {at} missing"),
        });
    }
    for (ri, row) in rows.iter_mut().enumerate() {
        let cells = row.as_array_mut().ok_or_else(|| EditError::Other {
            message: format!("row {ri} not array"),
        })?;
        cells.remove(at);
    }
    host.mark_touched(block_index);
    Ok(())
}

pub fn register_table_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("setTableCellText", "setTableCellText").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            let row = args.get("row").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let col = args.get("col").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let text =
                args.get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| EditError::Other {
                        message: "setTableCellText missing text".into(),
                    })?;
            set_table_cell_text(h, bi, row, col, text)?;
            let mut cs = ChangeSet::new(ChangeKind::Other, format!("setTableCellText {row},{col}"));
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("insertTableRow", "insertTableRow").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            let at = args.get("at").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            insert_table_row(h, bi, at)?;
            let mut cs = ChangeSet::new(ChangeKind::Structure, format!("insertTableRow {at}"));
            cs.structural = true;
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("deleteTableRow", "deleteTableRow").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            let at = args.get("at").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            delete_table_row(h, bi, at)?;
            let mut cs = ChangeSet::new(ChangeKind::Structure, format!("deleteTableRow {at}"));
            cs.structural = true;
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("insertTableColumn", "insertTableColumn").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            let at = args.get("at").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            insert_table_column(h, bi, at)?;
            let mut cs = ChangeSet::new(ChangeKind::Structure, format!("insertTableColumn {at}"));
            cs.structural = true;
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
    registry.register(
        Command::stub("deleteTableColumn", "deleteTableColumn").with_runner(|host, args| {
            let h = host_mut(host)?;
            let bi = block_index(args)?;
            let at = args.get("at").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            delete_table_column(h, bi, at)?;
            let mut cs = ChangeSet::new(ChangeKind::Structure, format!("deleteTableColumn {at}"));
            cs.structural = true;
            cs.dirty_parts = vec!["word/document.xml".into()];
            Ok(cs)
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catches_insert_table_row_on_paragraph_is_rejected() {
        // Catches: table structure ops silently mutating a paragraph block (#79 negative).
        let mut host = DocxHost::stub(vec![json!({
            "type": "paragraph",
            "id": "p0",
            "runs": [{ "text": "x" }],
        })]);
        let err = insert_table_row(&mut host, 0, 0).unwrap_err();
        assert!(err.to_string().contains("not a table"));
    }
}
