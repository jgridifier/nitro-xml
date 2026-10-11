// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/parse-revisions.ts; changes listed in
// PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::{ns, Element};

use crate::xml_util::{first_w, get_w, named_w, serialize_element, ElementExt};

const WORD_DATE_UTC_NS: &str = "http://schemas.microsoft.com/office/word/2010/wordml";

#[derive(Default, Clone)]
pub struct OpenMoves {
    pub move_from: Vec<MoveRange>,
    pub move_to: Vec<MoveRange>,
}

#[derive(Clone)]
pub struct MoveRange {
    pub name: String,
    pub id: String,
}

pub trait RunCollector {
    fn parse_run(&mut self, run: &Element, revision: Option<Value>) -> Value;
    fn parse_other(&mut self, item: &Element) -> Option<Vec<Value>>;
    fn resolve_link(&self, hyperlink: &Element) -> Option<Value>;
}

pub fn is_comment_reference_run(run: &Element) -> bool {
    let content: Vec<_> = run
        .element_children()
        .into_iter()
        .filter(|c| !named_w(c, "rPr"))
        .collect();
    !content.is_empty() && content.iter().all(|c| named_w(c, "commentReference"))
}

fn revision_from(node: &Element, kind: &str) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("kind".into(), json!(kind));
    obj.insert(
        "author".into(),
        json!(get_w(Some(node), "author").unwrap_or_else(|| "Unknown".to_string())),
    );
    obj.insert(
        "id".into(),
        json!(get_w(Some(node), "id").unwrap_or_else(|| "0".to_string())),
    );
    if let Some(date) = get_w(Some(node), "date") {
        obj.insert("date".into(), json!(date));
    }
    if let Some(date_utc) = node.attr_ns(WORD_DATE_UTC_NS, "dateUtc") {
        obj.insert("dateUtc".into(), json!(date_utc));
    }
    Value::Object(obj)
}

pub fn paragraph_mark_revision(p_pr: Option<&Element>) -> Option<Value> {
    let r_pr = first_w(p_pr, "rPr");
    if let Some(ins) = first_w(r_pr, "ins") {
        return Some(revision_from(ins, "insert"));
    }
    if let Some(del) = first_w(r_pr, "del") {
        return Some(revision_from(del, "delete"));
    }
    None
}

pub fn paragraph_format_revision(p_pr: Option<&Element>) -> Option<Value> {
    let change = first_w(p_pr, "pPrChange")?;
    let mut rev = revision_from(change, "paragraphChange");
    if let Some(previous) = first_w(Some(change), "pPr") {
        if let Some(obj) = rev.as_object_mut() {
            obj.insert(
                "previousParagraphPropertiesXml".into(),
                json!(serialize_element(previous)),
            );
        }
    }
    Some(rev)
}

pub fn run_format_revision(r_pr: Option<&Element>) -> Option<Value> {
    let change = first_w(r_pr, "rPrChange")?;
    let mut rev = revision_from(change, "formatChange");
    if let Some(previous) = first_w(Some(change), "rPr") {
        if let Some(obj) = rev.as_object_mut() {
            obj.insert(
                "previousRunPropertiesXml".into(),
                json!(serialize_element(previous)),
            );
        }
    }
    Some(rev)
}

fn revision_wrapper_kind(local: &str) -> Option<&'static str> {
    match local {
        "ins" => Some("insert"),
        "del" => Some("delete"),
        "moveFrom" => Some("moveFrom"),
        "moveTo" => Some("moveTo"),
        _ => None,
    }
}

fn track_move_range(item: &Element, moves: &mut OpenMoves) -> bool {
    if item.ns.is_some() && item.ns.as_deref() != Some(ns::W) {
        return false;
    }
    let (kind, start) = match item.local.as_str() {
        "moveFromRangeStart" => ("moveFrom", true),
        "moveFromRangeEnd" => ("moveFrom", false),
        "moveToRangeStart" => ("moveTo", true),
        "moveToRangeEnd" => ("moveTo", false),
        _ => return false,
    };
    let id = get_w(Some(item), "id").unwrap_or_default();
    let open = if kind == "moveFrom" {
        &mut moves.move_from
    } else {
        &mut moves.move_to
    };
    if start {
        open.push(MoveRange {
            name: get_w(Some(item), "name").unwrap_or_else(|| id.clone()),
            id,
        });
    } else if let Some(index) = open.iter().position(|e| e.id == id) {
        open.remove(index);
    }
    true
}

pub fn collect_paragraph_runs(
    node: &Element,
    collector: &mut dyn RunCollector,
    active: &mut Vec<String>,
    moves: &mut OpenMoves,
) -> (Vec<Value>, bool) {
    let mut runs = Vec::new();
    let mut has_move = false;
    for item in node.element_children() {
        if named_w(item, "r") {
            if !is_comment_reference_run(item) {
                push_run(collector.parse_run(item, None), active, &mut runs);
            }
        } else if named_w(item, "hyperlink") {
            let link = collector.resolve_link(item);
            for child in item.element_children() {
                let parsed: Vec<Value> = if named_w(child, "r") {
                    vec![collector.parse_run(child, None)]
                } else {
                    collector.parse_other(child).unwrap_or_default()
                };
                for mut p in parsed {
                    if let Some(l) = &link {
                        if let Some(obj) = p.as_object_mut() {
                            obj.insert("link".into(), l.clone());
                        }
                    }
                    push_run(p, active, &mut runs);
                }
            }
        } else if item.ns.is_none() || item.ns.as_deref() == Some(ns::W) {
            if let Some(kind) = revision_wrapper_kind(&item.local) {
                if kind == "moveFrom" || kind == "moveTo" {
                    has_move = true;
                }
                let mut revision = revision_from(item, kind);
                let range = if kind == "moveFrom" {
                    moves.move_from.last()
                } else if kind == "moveTo" {
                    moves.move_to.last()
                } else {
                    None
                };
                if let Some(r) = range {
                    if let Some(obj) = revision.as_object_mut() {
                        obj.insert("move".into(), json!({ "name": r.name, "rangeId": r.id }));
                    }
                }
                for child in item.element_children() {
                    let parsed: Vec<Value> = if named_w(child, "r") {
                        let mut run = collector.parse_run(child, Some(revision.clone()));
                        if let Some(obj) = run.as_object_mut() {
                            obj.insert("revision".into(), revision.clone());
                        }
                        vec![run]
                    } else {
                        let mut from_other = collector.parse_other(child).unwrap_or_default();
                        for p in &mut from_other {
                            if let Some(obj) = p.as_object_mut() {
                                obj.insert("revision".into(), revision.clone());
                            }
                        }
                        from_other
                    };
                    for p in parsed {
                        push_run(p, active, &mut runs);
                    }
                }
            } else if track_move_range(item, moves) {
                continue;
            } else if named_w(item, "commentRangeStart") {
                if let Some(id) = get_w(Some(item), "id") {
                    active.push(id);
                }
            } else if named_w(item, "commentRangeEnd") {
                let id = get_w(Some(item), "id");
                if let Some(id) = id {
                    if let Some(index) = active.iter().position(|a| a == &id) {
                        active.remove(index);
                    }
                }
            } else {
                for run in collector.parse_other(item).unwrap_or_default() {
                    push_run(run, active, &mut runs);
                }
            }
        } else {
            for run in collector.parse_other(item).unwrap_or_default() {
                push_run(run, active, &mut runs);
            }
        }
    }
    (runs, has_move)
}

fn push_run(run: Value, active: &[String], runs: &mut Vec<Value>) {
    let mut run = run;
    if !active.is_empty() {
        let mut ids: Vec<String> = active.to_vec();
        if let Some(existing) = run.get("commentIds").and_then(|v| v.as_array()) {
            for id in existing {
                if let Some(s) = id.as_str() {
                    ids.push(s.to_string());
                }
            }
        }
        ids.sort();
        ids.dedup();
        if let Some(obj) = run.as_object_mut() {
            obj.insert("commentIds".into(), json!(ids));
        }
    }
    runs.push(run);
}
