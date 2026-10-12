// SPDX-License-Identifier: MIT
//! Mutable docx edit host: paragraph/run JSON model (#78).
//!
//! Mirrors the TS `loadDocx` model shape used by `parity/edit` / `parity/undo`.
//! Package writers land elsewhere; this host owns in-memory blocks for edit/undo parity.

use std::any::Any;
use std::collections::BTreeSet;

use nitro_xml_opc::edit::{EditHost, EditScope, HostSnapshot};
use serde_json::{json, Map, Value};

use crate::body::read_package;

/// In-memory docx document the text/format/table/section runners mutate.
#[derive(Debug, Clone)]
pub struct DocxHost {
    pub format: String,
    pub blocks: Vec<Value>,
    pub numbering_catalog: Option<Value>,
    pub paragraph_styles: Option<Value>,
    pub character_styles: Option<Value>,
    /// Section properties including header/footer stories (#79).
    pub sections: Option<Value>,
    pub page: Option<Value>,
    pub even_and_odd_headers: bool,
    /// Block indices touched this session (for differential dumps).
    pub touched: BTreeSet<usize>,
}

impl DocxHost {
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let loaded = read_package(bytes).map_err(|e| e.to_string())?;
        let m = &loaded.model;
        Ok(Self {
            format: m.format.clone(),
            blocks: m.blocks.clone(),
            numbering_catalog: m.numbering_catalog.clone(),
            paragraph_styles: m.paragraph_styles.clone(),
            character_styles: m.character_styles.clone(),
            sections: m.sections.as_ref().map(|s| json!(s)),
            page: Some(m.page.clone()),
            even_and_odd_headers: m.even_and_odd_headers,
            touched: BTreeSet::new(),
        })
    }

    pub fn mark_touched(&mut self, block: usize) {
        self.touched.insert(block);
    }

    pub fn paragraph_mut(&mut self, block_index: usize) -> Result<&mut Value, String> {
        let block = self
            .blocks
            .get_mut(block_index)
            .ok_or_else(|| format!("block {block_index} missing"))?;
        let ty = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty != "paragraph" {
            return Err(format!("block {block_index} is not a paragraph (got {ty})"));
        }
        Ok(block)
    }

    pub fn paragraph(&self, block_index: usize) -> Result<&Value, String> {
        let block = self
            .blocks
            .get(block_index)
            .ok_or_else(|| format!("block {block_index} missing"))?;
        let ty = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty != "paragraph" {
            return Err(format!("block {block_index} is not a paragraph (got {ty})"));
        }
        Ok(block)
    }

    pub fn table_mut(&mut self, block_index: usize) -> Result<&mut Value, String> {
        let block = self
            .blocks
            .get_mut(block_index)
            .ok_or_else(|| format!("block {block_index} missing"))?;
        let ty = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if ty != "table" {
            return Err(format!("block {block_index} is not a table (got {ty})"));
        }
        Ok(block)
    }

    /// Differential dump: touched paragraphs only (edit-parity model compare).
    pub fn model_snapshot(&self) -> Value {
        let mut blocks = Vec::new();
        for &i in &self.touched {
            if let Some(b) = self.blocks.get(i) {
                blocks.push(project_block(i, b));
            }
        }
        json!({ "format": "docx", "blocks": blocks })
    }

    /// Canonical undo-parity projection — matches `parity/undo/snapshot.mjs` `snapshotDocx`.
    pub fn undo_parity_snapshot(&self) -> Value {
        let blocks: Vec<Value> = self
            .blocks
            .iter()
            .enumerate()
            .map(|(index, b)| project_block(index, b))
            .collect();
        json!({ "format": "docx", "blocks": blocks })
    }

    fn state_json(&self) -> Value {
        json!({
            "format": self.format,
            "blocks": self.blocks,
            "numberingCatalog": self.numbering_catalog,
            "paragraphStyles": self.paragraph_styles,
            "characterStyles": self.character_styles,
            "sections": self.sections,
            "page": self.page,
            "evenAndOddHeaders": self.even_and_odd_headers,
            "touched": self.touched.iter().copied().collect::<Vec<_>>(),
        })
    }

    fn apply_state_json(&mut self, v: &Value) -> Result<(), String> {
        self.format = v
            .get("format")
            .and_then(|x| x.as_str())
            .unwrap_or("docx")
            .to_owned();
        self.blocks = v
            .get("blocks")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        self.numbering_catalog = v.get("numberingCatalog").cloned().filter(|x| !x.is_null());
        self.paragraph_styles = v.get("paragraphStyles").cloned().filter(|x| !x.is_null());
        self.character_styles = v.get("characterStyles").cloned().filter(|x| !x.is_null());
        self.sections = v.get("sections").cloned().filter(|x| !x.is_null());
        self.page = v.get("page").cloned().filter(|x| !x.is_null());
        self.even_and_odd_headers = v
            .get("evenAndOddHeaders")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        self.touched.clear();
        if let Some(arr) = v.get("touched").and_then(|x| x.as_array()) {
            for x in arr {
                if let Some(i) = x.as_u64() {
                    self.touched.insert(i as usize);
                }
            }
        }
        Ok(())
    }
}

impl EditHost for DocxHost {
    fn capture(&self, _scopes: &[EditScope]) -> HostSnapshot {
        HostSnapshot(serde_json::to_vec(&self.state_json()).unwrap_or_default())
    }

    fn restore(&mut self, snap: &HostSnapshot) {
        if snap.0.is_empty() {
            return;
        }
        if let Ok(v) = serde_json::from_slice::<Value>(&snap.0) {
            let _ = self.apply_state_json(&v);
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}

fn project_run(r: &Value) -> Value {
    json!({
        "text": r.get("text").and_then(|t| t.as_str()).unwrap_or(""),
        "bold": r.get("bold").and_then(|b| b.as_bool()).unwrap_or(false),
        "italic": r.get("italic").and_then(|b| b.as_bool()).unwrap_or(false),
        "underline": r.get("underline").and_then(|b| b.as_bool()).unwrap_or(false),
        "fontSize": r.get("fontSize").cloned().unwrap_or(Value::Null),
        "color": r.get("color").cloned().unwrap_or(Value::Null),
    })
}

fn project_block(index: usize, b: &Value) -> Value {
    let ty = b
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("paragraph");
    if ty == "table" {
        let rows: Vec<Value> = b
            .get("rows")
            .and_then(|r| r.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        let cells: Vec<Value> = row
                            .as_array()
                            .map(|cells| {
                                cells
                                    .iter()
                                    .map(|cell| {
                                        let paragraphs: Vec<Value> = cell
                                            .get("paragraphs")
                                            .and_then(|p| p.as_array())
                                            .map(|paras| {
                                                paras
                                                    .iter()
                                                    .map(|p| {
                                                        json!({
                                                            "align": p.get("align").cloned().unwrap_or(Value::Null),
                                                            "runs": p.get("runs").and_then(|r| r.as_array()).map(|rs| {
                                                                rs.iter().map(project_run).collect::<Vec<_>>()
                                                            }).unwrap_or_default(),
                                                        })
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default();
                                        json!({ "paragraphs": paragraphs })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        Value::Array(cells)
                    })
                    .collect()
            })
            .unwrap_or_default();
        return json!({ "index": index, "type": "table", "rows": rows });
    }
    let runs: Vec<Value> = b
        .get("runs")
        .and_then(|r| r.as_array())
        .map(|rs| rs.iter().map(project_run).collect())
        .unwrap_or_default();
    json!({
        "index": index,
        "type": ty,
        "align": b.get("align").cloned().unwrap_or(Value::Null),
        "runs": runs,
    })
}

/// Build a single-run paragraph replacement payload (TS `setParagraphText`).
pub fn make_run(text: &str, attrs: &Map<String, Value>) -> Value {
    let mut run = Map::new();
    run.insert("text".into(), json!(text));
    for key in ["bold", "italic", "fontSize", "color", "underline"] {
        if let Some(v) = attrs.get(key) {
            run.insert(key.into(), v.clone());
        }
    }
    Value::Object(run)
}
