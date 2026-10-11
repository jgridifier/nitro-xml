// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/comments.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::{parse_xml_str, Element};

use crate::json_util::insert_str;
use crate::xml_util::{by_name_w, get_w, text_content, ElementExt};

const W14_NS: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const W15_NS: &str = "http://schemas.microsoft.com/office/word/2012/wordml";

pub fn parse_comments(comments_xml: &str, extended_xml: Option<&str>) -> Vec<Value> {
    let doc = parse_xml_str(comments_xml).expect("comments xml");
    let comment_elements = by_name_w(&doc.root, "comment");
    let mut para_id_to_comment_id = std::collections::HashMap::new();
    let mut comments: Vec<Value> = comment_elements
        .iter()
        .map(|element| {
            let id = get_w(Some(element), "id").unwrap_or_else(|| "0".to_string());
            let text = by_name_w(element, "p")
                .iter()
                .map(|p| paragraph_text(p))
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();
            let mut obj = serde_json::Map::new();
            obj.insert("id".into(), json!(id));
            obj.insert(
                "author".into(),
                json!(get_w(Some(element), "author").unwrap_or_else(|| "Unknown".to_string())),
            );
            obj.insert("text".into(), json!(text));
            insert_str(&mut obj, "initials", get_w(Some(element), "initials"));
            insert_str(&mut obj, "date", get_w(Some(element), "date"));
            let paragraphs = by_name_w(element, "p");
            if let Some(p) = paragraphs.first() {
                if let Some(para_id) = attr_ns_any(p, W14_NS, "paraId") {
                    para_id_to_comment_id.insert(para_id, id.clone());
                }
            }
            Value::Object(obj)
        })
        .collect();

    if let Some(ext_xml) = extended_xml {
        let extended = parse_xml_str(ext_xml).expect("comments extended xml");
        for entry in elements_by_ns_local(&extended.root, W15_NS, "commentEx") {
            let para_id = attr_ns_any(entry, W15_NS, "paraId");
            let comment = para_id
                .and_then(|pid| para_id_to_comment_id.get(&pid))
                .and_then(|cid| {
                    comments
                        .iter_mut()
                        .find(|c| c.get("id") == Some(&json!(cid)))
                });
            if let Some(comment) = comment {
                if let Some(obj) = comment.as_object_mut() {
                    let done = attr_ns_any(entry, W15_NS, "done");
                    if done.as_deref() == Some("1") || done.as_deref() == Some("true") {
                        obj.insert("resolved".into(), json!(true));
                    }
                    let parent_para = attr_ns_any(entry, W15_NS, "paraIdParent");
                    if let Some(pp) = parent_para {
                        if let Some(parent_id) = para_id_to_comment_id.get(&pp) {
                            obj.insert("parentId".into(), json!(parent_id));
                        }
                    }
                }
            }
        }
    }
    comments
}

fn paragraph_text(paragraph: &Element) -> String {
    by_name_w(paragraph, "t")
        .iter()
        .map(|t| text_content(t))
        .collect::<Vec<_>>()
        .join("")
}

fn attr_ns_any(el: &Element, namespace: &str, local: &str) -> Option<String> {
    el.attrs
        .iter()
        .find(|a| a.local == local && a.ns.as_deref() == Some(namespace))
        .map(|a| a.value.clone())
}

fn elements_by_ns_local<'a>(root: &'a Element, namespace: &str, local: &str) -> Vec<&'a Element> {
    let mut out = Vec::new();
    collect_ns_local(root, namespace, local, &mut out);
    out
}

fn collect_ns_local<'a>(el: &'a Element, namespace: &str, local: &str, out: &mut Vec<&'a Element>) {
    if el.local == local && el.ns.as_deref() == Some(namespace) {
        out.push(el);
    }
    for child in el.element_children() {
        collect_ns_local(child, namespace, local, out);
    }
}
