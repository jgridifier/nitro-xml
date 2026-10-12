// SPDX-License-Identifier: MIT
//! `edits/*.json` script types — same vocabulary as the TS edit-parity harness (#50 / #63).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::edit::error::EditError;

/// Every `op` enum value in `edits/schema.json`.
pub const SCHEMA_OP_IDS: &[&str] = &[
    "setCellInput",
    "setCellValue",
    "applyStyle",
    "applyCellStyle",
    "addSheet",
    "renameSheet",
    "deleteSheet",
    "insertRows",
    "deleteRows",
    "insertColumns",
    "deleteColumns",
    "batch",
    "setParagraphText",
    "setRunFormatting",
    "setParagraphAlign",
    "setTableCellText",
    "insertTableRow",
    "deleteTableRow",
    "insertTableColumn",
    "deleteTableColumn",
    "setShapeText",
    "patchShapeGeometry",
    "setShapeTextStyle",
    "addShape",
    "deleteShape",
    "duplicateShape",
    "bringForward",
    "sendBackward",
    "bringToFront",
    "sendToBack",
    "groupShapes",
    "ungroupShapes",
    "setShapeFill",
    "setShapeLine",
    "acceptRevision",
    "rejectRevision",
    "acceptAllRevisions",
    "rejectAllRevisions",
    "addComment",
    "replyToComment",
    "resolveComment",
    "deleteComment",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditFormat {
    Xlsx,
    Docx,
    Pptx,
}

impl EditFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xlsx => "xlsx",
            Self::Docx => "docx",
            Self::Pptx => "pptx",
        }
    }
}

/// One editor op from `edits/<format>/*.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum Op {
    #[serde(rename = "setCellInput")]
    SetCellInput {
        sheet: u32,
        row: u32,
        col: u32,
        text: String,
    },
    #[serde(rename = "setCellValue")]
    SetCellValue {
        sheet: u32,
        row: u32,
        col: u32,
        value: Value,
    },
    #[serde(rename = "applyStyle")]
    ApplyStyle {
        sheet: u32,
        range: String,
        patch: Value,
    },
    #[serde(rename = "applyCellStyle")]
    ApplyCellStyle {
        sheet: u32,
        range: String,
        name: String,
    },
    #[serde(rename = "addSheet")]
    AddSheet {
        name: String,
        #[serde(default)]
        at: Option<u32>,
    },
    #[serde(rename = "renameSheet")]
    RenameSheet { index: u32, name: String },
    #[serde(rename = "deleteSheet")]
    DeleteSheet { index: u32 },
    #[serde(rename = "insertRows")]
    InsertRows { sheet: u32, at: u32, count: u32 },
    #[serde(rename = "deleteRows")]
    DeleteRows { sheet: u32, at: u32, count: u32 },
    #[serde(rename = "insertColumns")]
    InsertColumns { sheet: u32, at: u32, count: u32 },
    #[serde(rename = "deleteColumns")]
    DeleteColumns { sheet: u32, at: u32, count: u32 },
    #[serde(rename = "batch")]
    Batch {
        #[serde(default)]
        label: Option<String>,
        ops: Vec<Op>,
    },
    #[serde(rename = "setParagraphText")]
    SetParagraphText {
        #[serde(rename = "blockIndex")]
        block_index: u32,
        text: String,
        #[serde(default)]
        #[serde(flatten)]
        rest: Map<String, Value>,
    },
    #[serde(rename = "setRunFormatting")]
    SetRunFormatting {
        #[serde(rename = "blockIndex")]
        block_index: u32,
        #[serde(default)]
        #[serde(flatten)]
        rest: Map<String, Value>,
    },
    #[serde(rename = "setParagraphAlign")]
    SetParagraphAlign {
        #[serde(rename = "blockIndex")]
        block_index: u32,
        align: String,
    },
    #[serde(rename = "setTableCellText")]
    SetTableCellText {
        #[serde(rename = "blockIndex")]
        block_index: u32,
        row: u32,
        col: u32,
        text: String,
    },
    #[serde(rename = "insertTableRow")]
    InsertTableRow {
        #[serde(rename = "blockIndex")]
        block_index: u32,
        at: u32,
    },
    #[serde(rename = "deleteTableRow")]
    DeleteTableRow {
        #[serde(rename = "blockIndex")]
        block_index: u32,
        at: u32,
    },
    #[serde(rename = "insertTableColumn")]
    InsertTableColumn {
        #[serde(rename = "blockIndex")]
        block_index: u32,
        at: u32,
    },
    #[serde(rename = "deleteTableColumn")]
    DeleteTableColumn {
        #[serde(rename = "blockIndex")]
        block_index: u32,
        at: u32,
    },
    #[serde(rename = "setShapeText")]
    SetShapeText {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(rename = "elementIndex")]
        element_index: u32,
        text: String,
    },
    #[serde(rename = "patchShapeGeometry")]
    PatchShapeGeometry {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(rename = "elementIndex")]
        element_index: u32,
        #[serde(default)]
        #[serde(flatten)]
        rest: Map<String, Value>,
    },
    #[serde(rename = "setShapeTextStyle")]
    SetShapeTextStyle {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(rename = "elementIndex")]
        element_index: u32,
        #[serde(default)]
        #[serde(flatten)]
        rest: Map<String, Value>,
    },
    #[serde(rename = "addShape")]
    AddShape {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(rename = "shapeType")]
        shape_type: String,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        #[serde(default)]
        id: Option<String>,
    },
    #[serde(rename = "deleteShape")]
    DeleteShape {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(default, rename = "elementIndex")]
        element_index: Option<u32>,
        #[serde(default, rename = "elementId")]
        element_id: Option<String>,
    },
    #[serde(rename = "duplicateShape")]
    DuplicateShape {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(default, rename = "elementIndex")]
        element_index: Option<u32>,
        #[serde(default, rename = "elementId")]
        element_id: Option<String>,
        #[serde(default, rename = "newId")]
        new_id: Option<String>,
    },
    #[serde(rename = "bringForward")]
    BringForward {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(default, rename = "elementIndex")]
        element_index: Option<u32>,
        #[serde(default, rename = "elementId")]
        element_id: Option<String>,
    },
    #[serde(rename = "sendBackward")]
    SendBackward {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(default, rename = "elementIndex")]
        element_index: Option<u32>,
        #[serde(default, rename = "elementId")]
        element_id: Option<String>,
    },
    #[serde(rename = "bringToFront")]
    BringToFront {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(default, rename = "elementIndex")]
        element_index: Option<u32>,
        #[serde(default, rename = "elementId")]
        element_id: Option<String>,
    },
    #[serde(rename = "sendToBack")]
    SendToBack {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(default, rename = "elementIndex")]
        element_index: Option<u32>,
        #[serde(default, rename = "elementId")]
        element_id: Option<String>,
    },
    #[serde(rename = "groupShapes")]
    GroupShapes {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(rename = "elementIds")]
        element_ids: Vec<String>,
        #[serde(rename = "groupId")]
        group_id: String,
    },
    #[serde(rename = "ungroupShapes")]
    UngroupShapes {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(default, rename = "groupId")]
        group_id: Option<String>,
        #[serde(default, rename = "elementId")]
        element_id: Option<String>,
        #[serde(default, rename = "childIds")]
        child_ids: Option<Vec<String>>,
    },
    #[serde(rename = "setShapeFill")]
    SetShapeFill {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(default, rename = "elementIndex")]
        element_index: Option<u32>,
        #[serde(default, rename = "elementId")]
        element_id: Option<String>,
        #[serde(default)]
        #[serde(flatten)]
        rest: Map<String, Value>,
    },
    #[serde(rename = "setShapeLine")]
    SetShapeLine {
        #[serde(rename = "slideIndex")]
        slide_index: u32,
        #[serde(default, rename = "elementIndex")]
        element_index: Option<u32>,
        #[serde(default, rename = "elementId")]
        element_id: Option<String>,
        #[serde(default)]
        #[serde(flatten)]
        rest: Map<String, Value>,
    },
    #[serde(rename = "acceptRevision")]
    AcceptRevision { id: String },
    #[serde(rename = "rejectRevision")]
    RejectRevision { id: String },
    #[serde(rename = "acceptAllRevisions")]
    AcceptAllRevisions,
    #[serde(rename = "rejectAllRevisions")]
    RejectAllRevisions,
    #[serde(rename = "addComment")]
    AddComment {
        #[serde(rename = "blockIndex")]
        block_index: u32,
        #[serde(default)]
        #[serde(rename = "runIndex")]
        run_index: Option<u32>,
        author: String,
        text: String,
        #[serde(default)]
        id: Option<String>,
    },
    #[serde(rename = "replyToComment")]
    ReplyToComment {
        #[serde(rename = "parentId")]
        parent_id: String,
        author: String,
        text: String,
        #[serde(default)]
        id: Option<String>,
    },
    #[serde(rename = "resolveComment")]
    ResolveComment {
        id: String,
        #[serde(default)]
        resolved: Option<bool>,
    },
    #[serde(rename = "deleteComment")]
    DeleteComment { id: String },
}

impl Op {
    /// Registry / journal command id — the schema `op` string.
    pub fn command_id(&self) -> &'static str {
        match self {
            Self::SetCellInput { .. } => "setCellInput",
            Self::SetCellValue { .. } => "setCellValue",
            Self::ApplyStyle { .. } => "applyStyle",
            Self::ApplyCellStyle { .. } => "applyCellStyle",
            Self::AddSheet { .. } => "addSheet",
            Self::RenameSheet { .. } => "renameSheet",
            Self::DeleteSheet { .. } => "deleteSheet",
            Self::InsertRows { .. } => "insertRows",
            Self::DeleteRows { .. } => "deleteRows",
            Self::InsertColumns { .. } => "insertColumns",
            Self::DeleteColumns { .. } => "deleteColumns",
            Self::Batch { .. } => "batch",
            Self::SetParagraphText { .. } => "setParagraphText",
            Self::SetRunFormatting { .. } => "setRunFormatting",
            Self::SetParagraphAlign { .. } => "setParagraphAlign",
            Self::SetTableCellText { .. } => "setTableCellText",
            Self::InsertTableRow { .. } => "insertTableRow",
            Self::DeleteTableRow { .. } => "deleteTableRow",
            Self::InsertTableColumn { .. } => "insertTableColumn",
            Self::DeleteTableColumn { .. } => "deleteTableColumn",
            Self::SetShapeText { .. } => "setShapeText",
            Self::PatchShapeGeometry { .. } => "patchShapeGeometry",
            Self::SetShapeTextStyle { .. } => "setShapeTextStyle",
            Self::AddShape { .. } => "addShape",
            Self::DeleteShape { .. } => "deleteShape",
            Self::DuplicateShape { .. } => "duplicateShape",
            Self::BringForward { .. } => "bringForward",
            Self::SendBackward { .. } => "sendBackward",
            Self::BringToFront { .. } => "bringToFront",
            Self::SendToBack { .. } => "sendToBack",
            Self::GroupShapes { .. } => "groupShapes",
            Self::UngroupShapes { .. } => "ungroupShapes",
            Self::SetShapeFill { .. } => "setShapeFill",
            Self::SetShapeLine { .. } => "setShapeLine",
            Self::AcceptRevision { .. } => "acceptRevision",
            Self::RejectRevision { .. } => "rejectRevision",
            Self::AcceptAllRevisions => "acceptAllRevisions",
            Self::RejectAllRevisions => "rejectAllRevisions",
            Self::AddComment { .. } => "addComment",
            Self::ReplyToComment { .. } => "replyToComment",
            Self::ResolveComment { .. } => "resolveComment",
            Self::DeleteComment { .. } => "deleteComment",
        }
    }

    /// JSON args object (without the `op` field) for registry runners / the journal.
    pub fn args_json(&self) -> Result<Value, EditError> {
        let mut v =
            serde_json::to_value(self).map_err(|e| EditError::invalid_script(e.to_string()))?;
        if let Value::Object(map) = &mut v {
            map.remove("op");
        }
        Ok(v)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditScript {
    pub id: String,
    pub format: EditFormat,
    pub fixture: String,
    #[serde(default)]
    pub description: Option<String>,
    pub ops: Vec<Op>,
}

/// Parse one `edits/*.json` document.
pub fn parse_edit_script(json: &str) -> Result<EditScript, EditError> {
    let raw: Value =
        serde_json::from_str(json).map_err(|e| EditError::invalid_script(e.to_string()))?;
    if let Some(ops) = raw.get("ops").and_then(|o| o.as_array()) {
        validate_ops_array(ops)?;
    }
    serde_json::from_value(raw).map_err(|e| EditError::invalid_script(e.to_string()))
}

fn validate_ops_array(ops: &[Value]) -> Result<(), EditError> {
    for (i, op) in ops.iter().enumerate() {
        let name = op
            .get("op")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EditError::invalid_script(format!("ops[{i}] missing op")))?;
        if !SCHEMA_OP_IDS.contains(&name) {
            return Err(EditError::UnknownOp {
                op: name.to_owned(),
            });
        }
        if name == "batch" {
            if let Some(inner) = op.get("ops").and_then(|o| o.as_array()) {
                validate_ops_array(inner)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_unknown_op_name_rejected_at_parse() {
        // Catches: typos in edits/*.json must fail closed, not map to a wrong command.
        let err = parse_edit_script(
            r#"{"id":"x","format":"xlsx","fixture":"f","ops":[{"op":"notARealOp"}]}"#,
        )
        .unwrap_err();
        assert_eq!(err.kind(), "unknown_op");
    }
}
