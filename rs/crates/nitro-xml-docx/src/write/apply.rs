// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/write.ts; changes listed in PROVENANCE.md.

use serde_json::Value;

use nitro_xml_opc::xml::{ns, Document, Element, Node};

use super::element::{first_w_mut, is_body_block, make_w, replace_body_blocks, set_w_attr};
use super::paragraph::{create_paragraph, write_paragraph};
use super::table::write_table;
use crate::edit::host::DocxHost;

fn twips(pixels: f64) -> String {
    (pixels * 15.0).round().to_string()
}

fn block_id(block: &Value) -> Option<&str> {
    block.get("id").and_then(|v| v.as_str())
}

fn original_block_nodes(body: &Element) -> Vec<Element> {
    body.children
        .iter()
        .filter_map(|n| match n {
            Node::Element(e) if is_body_block(e) => Some(e.clone()),
            _ => None,
        })
        .collect()
}

pub fn apply_model(doc: &mut Document, host: &DocxHost) -> Result<(), String> {
    let body_mut = find_body_mut(&mut doc.root)?;
    let old_nodes = original_block_nodes(body_mut);
    let mut old_by_id: std::collections::HashMap<String, Element> =
        std::collections::HashMap::new();
    for (block, node) in host.base_blocks.iter().zip(old_nodes.iter()) {
        if let Some(id) = block_id(block) {
            old_by_id.insert(id.to_owned(), node.clone());
        }
    }
    let bases: std::collections::HashMap<String, &Value> = host
        .base_blocks
        .iter()
        .filter_map(|b| block_id(b).map(|id| (id.to_owned(), b)))
        .collect();

    let mut outputs: Vec<Element> = Vec::new();
    for block in &host.blocks {
        let id = block_id(block).ok_or_else(|| "block missing id".to_string())?;
        let ty = block
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("paragraph");
        let base = bases.get(id);
        let old = old_by_id.get(id);
        if ty == "paragraph" {
            let node = if let Some(old) = old {
                write_paragraph(block, old, base.copied())?
            } else {
                create_paragraph(block)?
            };
            outputs.push(node);
        } else if ty == "table" {
            let node = if let Some(old) = old {
                write_table(block, old, base.copied())?
            } else {
                return Err("Creating new tables is not supported".into());
            };
            outputs.push(node);
        } else {
            return Err(format!("unsupported block type {ty}"));
        }
    }

    replace_body_blocks(body_mut, &outputs);

    if let Some(page) = host.page.as_ref() {
        patch_page_section(body_mut, page)?;
    }

    Ok(())
}

fn find_body_mut(root: &mut Element) -> Result<&mut Element, String> {
    if root.local == "body" && root.ns.as_deref() == Some(ns::W) {
        return Ok(root);
    }
    for child in &mut root.children {
        if let Node::Element(e) = child {
            if let Ok(found) = find_body_mut(e) {
                return Ok(found);
            }
        }
    }
    Err("w:body not found".into())
}

fn patch_page_section(body: &mut Element, page: &Value) -> Result<(), String> {
    let width = page.get("width").and_then(|v| v.as_f64()).unwrap_or(612.0);
    let height = page.get("height").and_then(|v| v.as_f64()).unwrap_or(792.0);
    let margin_left = page
        .get("marginLeft")
        .and_then(|v| v.as_f64())
        .unwrap_or(72.0);
    let margin_right = page
        .get("marginRight")
        .and_then(|v| v.as_f64())
        .unwrap_or(72.0);
    let margin_top = page
        .get("marginTop")
        .and_then(|v| v.as_f64())
        .unwrap_or(72.0);
    let margin_bottom = page
        .get("marginBottom")
        .and_then(|v| v.as_f64())
        .unwrap_or(72.0);

    let sect_idx = body.children.iter().position(
        |n| matches!(n, Node::Element(e) if e.local == "sectPr" && e.ns.as_deref() == Some(ns::W)),
    );
    if sect_idx.is_none() {
        body.children.push(Node::Element(make_w("sectPr")));
    }
    let sect_idx = body
        .children
        .iter()
        .position(|n| matches!(n, Node::Element(e) if e.local == "sectPr"))
        .expect("sectPr");

    let Node::Element(section) = &mut body.children[sect_idx] else {
        return Err("sectPr missing".into());
    };

    if let Some(sz) = first_w_mut(section, "pgSz") {
        set_w_attr(sz, "w", &twips(width));
        set_w_attr(sz, "h", &twips(height));
    } else {
        let mut size = make_w("pgSz");
        set_w_attr(&mut size, "w", &twips(width));
        set_w_attr(&mut size, "h", &twips(height));
        section.children.push(Node::Element(size));
    }

    if let Some(margins) = first_w_mut(section, "pgMar") {
        set_w_attr(margins, "left", &twips(margin_left));
        set_w_attr(margins, "right", &twips(margin_right));
        set_w_attr(margins, "top", &twips(margin_top));
        set_w_attr(margins, "bottom", &twips(margin_bottom));
        for (name, fallback) in [("header", "720"), ("footer", "720"), ("gutter", "0")] {
            if !margins
                .attrs
                .iter()
                .any(|a| a.local == name && a.ns.as_deref() == Some(ns::W))
            {
                set_w_attr(margins, name, fallback);
            }
        }
    } else {
        let mut margins = make_w("pgMar");
        set_w_attr(&mut margins, "left", &twips(margin_left));
        set_w_attr(&mut margins, "right", &twips(margin_right));
        set_w_attr(&mut margins, "top", &twips(margin_top));
        set_w_attr(&mut margins, "bottom", &twips(margin_bottom));
        for (name, fallback) in [("header", "720"), ("footer", "720"), ("gutter", "0")] {
            set_w_attr(&mut margins, name, fallback);
        }
        section.children.push(Node::Element(margins));
    }
    Ok(())
}
