// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/block-parser.ts (parseRun); changes listed in
// PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::Element;

use crate::breaks::{classify_break, BreakKind};
use crate::drawing::{parse_drawing, DrawingContext};
use crate::field_runs::parse_field_flags;
use crate::revisions::run_format_revision;
use crate::run_properties::parse_run_properties;
use crate::xml_util::{first_w, get_w, named_mc_choice, named_w, text_content, ElementExt};

pub fn parse_run(node: &Element, revision: Option<Value>, ctx: Option<&DrawingContext>) -> Value {
    let props = first_w(Some(node), "rPr");
    let content = run_content(node);
    let drawing = find_drawing(&content);
    if let (Some(drawing_el), Some(context)) = (drawing, ctx) {
        let mut run = json!({
            "text": "",
            "image": parse_drawing(drawing_el, context),
        });
        merge_run_props(&mut run, props);
        apply_revision(&mut run, revision, props);
        return run;
    }
    let mut break_kind: Option<BreakKind> = None;
    let mut note_reference: Option<Value> = None;
    let mut note_mark: Option<&str> = None;
    let mut field_char: Option<String> = None;
    let field_code = if !content.is_empty() && content.iter().all(|c| named_w(c, "instrText")) {
        Some(
            content
                .iter()
                .map(|c| text_content(c))
                .collect::<Vec<_>>()
                .join(""),
        )
    } else {
        None
    };
    let only = if content.len() == 1 {
        Some(content[0])
    } else {
        None
    };
    if let Some(only) = only {
        if named_w(only, "br") {
            let kind = classify_break(only);
            if matches!(kind, BreakKind::Page | BreakKind::Column) {
                break_kind = Some(kind);
            }
        } else if named_w(only, "footnoteReference") {
            if let Some(id) = get_w(Some(only), "id") {
                note_reference = Some(json!({ "kind": "footnote", "id": id }));
            }
        } else if named_w(only, "endnoteReference") {
            if let Some(id) = get_w(Some(only), "id") {
                note_reference = Some(json!({ "kind": "endnote", "id": id }));
            }
        } else if named_w(only, "footnoteRef") {
            note_mark = Some("footnote");
        } else if named_w(only, "endnoteRef") {
            note_mark = Some("endnote");
        } else if named_w(only, "fldChar") {
            field_char = get_w(Some(only), "fldCharType");
        }
    }
    let text = if break_kind.is_some()
        || note_reference.is_some()
        || note_mark.is_some()
        || field_char.is_some()
        || field_code.is_some()
    {
        String::new()
    } else {
        content
            .iter()
            .map(|child| {
                if named_w(child, "t") || named_w(child, "delText") {
                    text_content(child)
                } else if named_w(child, "tab") {
                    "\t".to_string()
                } else if named_w(child, "br") || named_w(child, "cr") {
                    "\n".to_string()
                } else if named_w(child, "noBreakHyphen") {
                    "‑".to_string()
                } else {
                    String::new()
                }
            })
            .collect::<Vec<_>>()
            .join("")
    };
    let mut run = json!({ "text": text });
    merge_run_props(&mut run, props);
    if let Some(k) = break_kind {
        let key = if k == BreakKind::Page {
            "page"
        } else {
            "column"
        };
        if let Some(obj) = run.as_object_mut() {
            obj.insert("break".into(), json!(key));
        }
    }
    if let Some(nr) = note_reference {
        if let Some(obj) = run.as_object_mut() {
            obj.insert("noteReference".into(), nr);
        }
    }
    if let Some(nm) = note_mark {
        if let Some(obj) = run.as_object_mut() {
            obj.insert("noteMark".into(), json!(nm));
        }
    }
    if let Some(fc) = field_char {
        if matches!(fc.as_str(), "begin" | "separate" | "end") {
            if let Some(obj) = run.as_object_mut() {
                obj.insert("fieldChar".into(), json!(fc));
            }
            if let Some(only) = only {
                if let Some(flags) = parse_field_flags(only) {
                    if let Some(obj) = run.as_object_mut() {
                        obj.insert("fieldFlags".into(), flags);
                    }
                }
            }
        }
    }
    if let Some(code) = field_code {
        if let Some(obj) = run.as_object_mut() {
            obj.insert("fieldCode".into(), json!(code));
        }
    }
    apply_revision(&mut run, revision, props);
    run
}

fn merge_run_props(run: &mut Value, props: Option<&Element>) {
    let formatting = parse_run_properties(props);
    if let Some(obj) = run.as_object_mut() {
        if let Some(fmt) = formatting.as_object() {
            for (k, v) in fmt {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
}

fn apply_revision(run: &mut Value, revision: Option<Value>, props: Option<&Element>) {
    let had_wrapper = revision.is_some();
    let run_revision = revision.or_else(|| run_format_revision(props));
    if let Some(rev) = run_revision {
        if let Some(obj) = run.as_object_mut() {
            obj.insert("revision".into(), rev);
        }
    }
    if had_wrapper {
        if let Some(fmt_rev) = run_format_revision(props) {
            if let Some(obj) = run.as_object_mut() {
                obj.insert("formatRevision".into(), fmt_rev);
            }
        }
    }
}

fn run_content(node: &Element) -> Vec<&Element> {
    node.element_children()
        .into_iter()
        .filter(|c| !named_w(c, "rPr") && !crate::breaks::is_cached_page_break(c))
        .collect()
}

fn find_drawing<'a>(content: &[&'a Element]) -> Option<&'a Element> {
    for child in content {
        if named_w(child, "drawing") || named_w(child, "pict") {
            return Some(child);
        }
        if child.local == "AlternateContent" {
            let choice = child
                .element_children()
                .into_iter()
                .find(|n| named_mc_choice(n));
            if let Some(choice) = choice {
                if let Some(found) = find_drawing(&choice.element_children()) {
                    return Some(found);
                }
            }
        }
    }
    None
}
