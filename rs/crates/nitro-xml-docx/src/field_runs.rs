// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/field-runs.ts and field-flags.ts;
// changes listed in PROVENANCE.md.

use std::collections::HashMap;

use serde_json::{json, Value};

use nitro_xml_opc::xml::Element;

use crate::simple_types::on_off_attribute;
use crate::xml_util::{get_w, named_w, text_content, ElementExt};

pub fn parse_field_flags(element: &Element) -> Option<Value> {
    let locked = on_off_attribute(element, "fldLock");
    let dirty = on_off_attribute(element, "dirty");
    if locked.is_none() && dirty.is_none() {
        return None;
    }
    let mut obj = serde_json::Map::new();
    if let Some(v) = locked {
        obj.insert("locked".into(), json!(v));
    }
    if let Some(v) = dirty {
        obj.insert("dirty".into(), json!(v));
    }
    Some(Value::Object(obj))
}

struct FieldStackEntry {
    instr: String,
    in_result: bool,
    locked: Option<bool>,
}

pub struct FieldTracker {
    stack: Vec<FieldStackEntry>,
}

impl FieldTracker {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn track(&mut self, element: &Element, run: &mut Value) {
        let mut result_instr: Option<String> = None;
        let mut result_locked: Option<bool> = None;
        for child in element.element_children() {
            if named_w(child, "fldChar") {
                let ftype = get_w(Some(child), "fldCharType");
                match ftype.as_deref() {
                    Some("begin") => {
                        let locked = parse_field_flags(child)
                            .and_then(|f| f.get("locked").and_then(|v| v.as_bool()));
                        self.stack.push(FieldStackEntry {
                            instr: String::new(),
                            in_result: false,
                            locked,
                        });
                    }
                    Some("separate") => {
                        if let Some(top) = self.stack.last_mut() {
                            top.in_result = true;
                        }
                    }
                    Some("end") => {
                        self.stack.pop();
                    }
                    _ => {}
                }
            } else if let Some(top) = self.stack.last_mut() {
                if named_w(child, "instrText") && !top.in_result {
                    top.instr.push_str(&text_content(child));
                } else if (named_w(child, "t") || named_w(child, "delText")) && top.in_result {
                    result_instr = Some(top.instr.trim().to_string());
                    result_locked = top.locked;
                }
            }
        }
        if let Some(instr) = result_instr.filter(|s| !s.is_empty()) {
            if let Some(obj) = run.as_object_mut() {
                obj.insert("field".into(), json!({ "instr": instr }));
                if let Some(locked) = result_locked {
                    obj.insert("fieldFlags".into(), json!({ "locked": locked }));
                }
            }
        }
    }
}

pub type FieldResultMetadata = HashMap<usize, (Value, Option<Value>)>;

pub fn story_field_results(container: &Element) -> FieldResultMetadata {
    let mut metadata = HashMap::new();
    let mut tracker = FieldTracker::new();
    fn visit(element: &Element, tracker: &mut FieldTracker, metadata: &mut FieldResultMetadata) {
        if named_w(element, "fldSimple") || named_w(element, "txbxContent") {
            return;
        }
        if named_w(element, "r") {
            let mut run = json!({ "text": "" });
            tracker.track(element, &mut run);
            if run.get("field").is_some() {
                let field = run.get("field").cloned().unwrap();
                let flags = run.get("fieldFlags").cloned();
                metadata.insert(element as *const Element as usize, (field, flags));
            }
            return;
        }
        for child in element.element_children() {
            visit(child, tracker, metadata);
        }
    }
    visit(container, &mut tracker, &mut metadata);
    metadata
}

pub fn field_placeholder_text(instr: Option<&str>) -> &'static str {
    let code = instr
        .unwrap_or("")
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_uppercase();
    match code.as_str() {
        "PAGE" => "[Page #]",
        "NUMPAGES" => "[Total pages]",
        "DATE" => "[Date]",
        "TIME" => "[Time]",
        "FILENAME" => "[File name]",
        _ if !code.is_empty() => "[Field]",
        _ => "[Field]",
    }
}
