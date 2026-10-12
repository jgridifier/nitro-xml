// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/write-paragraph.ts; changes listed in PROVENANCE.md.

use serde_json::Value;

use nitro_xml_opc::xml::{Element, Node};

use super::element::{first_w_mut, make_w, remove_w_children, set_w_attr};
use super::inline::{collect_inline_slots, replaceable_inline_indices};
use super::run::create_run;
use crate::paragraph_alignment::paragraph_justification_for_write;
pub fn write_paragraph(
    paragraph: &Value,
    node: &Element,
    base: Option<&Value>,
) -> Result<Element, String> {
    if base.is_some_and(|b| paragraph == b) {
        return Ok(node.clone());
    }
    let slots = collect_inline_slots(node).ok_or_else(|| {
        format!(
            "Cannot edit paragraph {}: unsupported inline content",
            paragraph.get("id").and_then(|v| v.as_str()).unwrap_or("?")
        )
    })?;
    let mut out = node.clone();

    if base.is_none()
        || base.and_then(|b| b.get("align")) != paragraph.get("align")
        || base.and_then(|b| b.get("justification")) != paragraph.get("justification")
        || base.and_then(|b| b.get("direction")) != paragraph.get("direction")
    {
        if first_w_mut(&mut out, "pPr").is_none() {
            out.children.insert(0, Node::Element(make_w("pPr")));
        }
        let p_pr = first_w_mut(&mut out, "pPr").expect("pPr");
        remove_w_children(p_pr, "jc");
        if let Some(jc) = paragraph_justification_for_write(paragraph) {
            let mut align_el = make_w("jc");
            set_w_attr(&mut align_el, "val", &jc);
            p_pr.children.push(Node::Element(align_el));
        }
    }

    let runs = paragraph
        .get("runs")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "paragraph missing runs".to_string())?;
    let base_runs = base.and_then(|b| b.get("runs").and_then(|r| r.as_array()));

    let mut new_runs: Vec<Element> = Vec::new();
    for (i, run) in runs.iter().enumerate() {
        let base_run = base_runs.and_then(|br| br.get(i));
        let old = slots
            .get(i)
            .and_then(|s| s.element.as_ref())
            .or_else(|| slots.first().and_then(|s| s.element.as_ref()));
        new_runs.push(create_run(run, base_run, old));
    }

    let indices = replaceable_inline_indices(&out);
    for (child_idx, run_el) in indices.iter().zip(new_runs.iter()) {
        if *child_idx < out.children.len() {
            out.children[*child_idx] = Node::Element(run_el.clone());
        }
    }
    if new_runs.len() > indices.len() {
        let insert_at = out
            .children
            .iter()
            .position(|n| matches!(n, Node::Element(e) if e.local == "pPr"))
            .map(|i| i + 1)
            .unwrap_or(0)
            + indices.len();
        for run in new_runs.iter().skip(indices.len()) {
            out.children.insert(insert_at, Node::Element(run.clone()));
        }
    } else if new_runs.len() < indices.len() {
        for extra in indices.iter().skip(new_runs.len()) {
            if *extra < out.children.len() {
                out.children.remove(*extra);
            }
        }
    }

    if let Some(p_pr) = first_w_mut(&mut out, "pPr") {
        if p_pr.children.is_empty() {
            out.children
                .retain(|n| !matches!(n, Node::Element(e) if e.local == "pPr"));
        }
    }

    Ok(out)
}

pub fn create_paragraph(paragraph: &Value) -> Result<Element, String> {
    let node = make_w("p");
    write_paragraph(paragraph, &node, None)
}
