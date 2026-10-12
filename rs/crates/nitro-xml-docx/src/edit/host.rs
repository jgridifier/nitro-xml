// SPDX-License-Identifier: MIT
//! Mutable docx edit host: paragraph/run JSON model (#78).
//!
//! Mirrors the TS `loadDocx` model shape used by `parity/edit` / `parity/undo`.
//! Package writers land elsewhere; this host owns in-memory blocks for edit/undo parity.

use std::any::Any;
use std::collections::BTreeSet;

use nitro_xml_opc::edit::{EditHost, EditScope, HostSnapshot};
use serde_json::{json, Map, Value};

use nitro_xml_opc::opc::Package;

use crate::body::read_package;

/// In-memory docx document the text/format/table/section runners mutate.
#[derive(Debug, Clone)]
pub struct DocxHost {
    pub format: String,
    pub blocks: Vec<Value>,
    /// Model at open time (TS `binding.base.blocks` for save identity).
    pub base_blocks: Vec<Value>,
    /// Original `word/document.xml` text for in-place block rewrite (#80).
    pub source_document_xml: String,
    pub numbering_catalog: Option<Value>,
    pub paragraph_styles: Option<Value>,
    pub character_styles: Option<Value>,
    /// Section properties including header/footer stories (#79).
    pub sections: Option<Value>,
    pub page: Option<Value>,
    pub even_and_odd_headers: bool,
    pub comments: Option<Vec<Value>>,
    pub comments_touched: bool,
    /// Block indices touched this session (for differential dumps).
    pub touched: BTreeSet<usize>,
}

impl DocxHost {
    /// Minimal host for unit tests (no package binding).
    #[cfg(test)]
    pub(crate) fn stub(blocks: Vec<Value>) -> Self {
        let base_blocks = blocks.clone();
        Self {
            format: "docx".into(),
            blocks,
            base_blocks,
            source_document_xml: r#"<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#.into(),
            numbering_catalog: None,
            paragraph_styles: None,
            character_styles: None,
            sections: None,
            page: None,
            even_and_odd_headers: false,
            comments: None,
            comments_touched: false,
            touched: BTreeSet::new(),
        }
    }

    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let loaded = read_package(bytes).map_err(|e| e.to_string())?;
        let mut pkg = Package::open(bytes).map_err(|e| e.to_string())?;
        let source_document_xml = pkg
            .text("word/document.xml")
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "word/document.xml missing".to_string())?
            .to_owned();
        let m = &loaded.model;
        let blocks = m.blocks.clone();
        Ok(Self {
            format: m.format.clone(),
            blocks: blocks.clone(),
            base_blocks: blocks,
            source_document_xml,
            numbering_catalog: m.numbering_catalog.clone(),
            paragraph_styles: m.paragraph_styles.clone(),
            character_styles: m.character_styles.clone(),
            sections: m.sections.as_ref().map(|s| json!(s)),
            page: Some(m.page.clone()),
            even_and_odd_headers: m.even_and_odd_headers,
            comments: m.comments.clone(),
            comments_touched: false,
            touched: BTreeSet::new(),
        })
    }

    /// True when the live model differs from the package-open snapshot (TS save dirty check).
    pub fn is_dirty(&self) -> bool {
        !blocks_json_equal(&self.blocks, &self.base_blocks)
    }

    pub fn mark_touched(&mut self, block: usize) {
        self.touched.insert(block);
    }

    pub fn mark_all_blocks_touched(&mut self) {
        for i in 0..self.blocks.len() {
            self.touched.insert(i);
        }
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
        let mut snap = json!({ "format": "docx", "blocks": blocks });
        if self.comments_touched {
            if let Some(c) = &self.comments {
                snap.as_object_mut()
                    .expect("object")
                    .insert("comments".into(), project_comments(c));
            }
        }
        snap
    }

    /// Canonical undo-parity projection — matches `parity/undo/snapshot.mjs` `snapshotDocx`.
    pub fn undo_parity_snapshot(&self) -> Value {
        let blocks: Vec<Value> = self
            .blocks
            .iter()
            .enumerate()
            .map(|(index, b)| project_block(index, b))
            .collect();
        let mut snap = json!({ "format": "docx", "blocks": blocks });
        if let Some(c) = &self.comments {
            snap.as_object_mut()
                .expect("object")
                .insert("comments".into(), project_comments(c));
        }
        snap
    }

    fn state_json(&self) -> Value {
        json!({
            "format": self.format,
            "blocks": self.blocks,
            "baseBlocks": self.base_blocks,
            "sourceDocumentXml": self.source_document_xml,
            "numberingCatalog": self.numbering_catalog,
            "paragraphStyles": self.paragraph_styles,
            "characterStyles": self.character_styles,
            "sections": self.sections,
            "page": self.page,
            "evenAndOddHeaders": self.even_and_odd_headers,
            "comments": self.comments,
            "commentsTouched": self.comments_touched,
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
        if let Some(arr) = v.get("baseBlocks").and_then(|x| x.as_array()) {
            self.base_blocks = arr.clone();
        }
        if let Some(s) = v.get("sourceDocumentXml").and_then(|x| x.as_str()) {
            self.source_document_xml = s.to_owned();
        }
        self.numbering_catalog = v.get("numberingCatalog").cloned().filter(|x| !x.is_null());
        self.paragraph_styles = v.get("paragraphStyles").cloned().filter(|x| !x.is_null());
        self.character_styles = v.get("characterStyles").cloned().filter(|x| !x.is_null());
        self.sections = v.get("sections").cloned().filter(|x| !x.is_null());
        self.page = v.get("page").cloned().filter(|x| !x.is_null());
        self.even_and_odd_headers = v
            .get("evenAndOddHeaders")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        self.comments = v.get("comments").and_then(|x| x.as_array()).cloned();
        self.comments_touched = v
            .get("commentsTouched")
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

fn project_revision(rev: Option<&Value>) -> Value {
    match rev {
        None => Value::Null,
        Some(r) => {
            let mut obj = Map::new();
            if let Some(id) = r.get("id") {
                obj.insert("id".into(), id.clone());
            }
            if let Some(kind) = r.get("kind") {
                obj.insert("kind".into(), kind.clone());
            }
            if let Some(author) = r.get("author") {
                obj.insert("author".into(), author.clone());
            }
            if let Some(mv) = r.get("move") {
                obj.insert("move".into(), mv.clone());
            }
            Value::Object(obj)
        }
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
        "commentIds": r.get("commentIds").cloned().unwrap_or(Value::Null),
        "revision": project_revision(r.get("revision")),
        "formatRevision": project_revision(r.get("formatRevision")),
    })
}

fn project_comments(comments: &[Value]) -> Value {
    let rows: Vec<Value> = comments
        .iter()
        .map(|c| {
            json!({
                "id": c.get("id").cloned().unwrap_or(Value::Null),
                "author": c.get("author").cloned().unwrap_or(Value::Null),
                "text": c.get("text").cloned().unwrap_or(Value::Null),
                "resolved": c.get("resolved").and_then(|v| v.as_bool()).unwrap_or(false),
                "parentId": c.get("parentId").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();
    Value::Array(rows)
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
                                                            "markRevision": project_revision(p.get("markRevision")),
                                                            "formatRevision": project_revision(p.get("formatRevision")),
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
        "markRevision": project_revision(b.get("markRevision")),
        "formatRevision": project_revision(b.get("formatRevision")),
        "runs": runs,
    })
}

/// Build a single-run paragraph replacement payload (TS `setParagraphText`).
fn blocks_json_equal(a: &[Value], b: &[Value]) -> bool {
    serde_json::to_vec(a).ok() == serde_json::to_vec(b).ok()
}

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
