// SPDX-License-Identifier: MIT
//! View-IR: ordered painted nodes per slide (parity/oracle/pptx.ts).

use serde::Serialize;
use serde_json::{json, Value};

use crate::presentation::Presentation;
use crate::slide::DumpElement;

#[derive(Debug, Clone, Serialize)]
#[allow(clippy::struct_field_names)]
pub struct ViewIrNode {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<ViewIrNode>>,
}

#[derive(Debug, Default)]
pub struct ViewIrCollector {
    pub nodes: Vec<ViewIrNode>,
}

impl ViewIrCollector {
    pub fn push_painted(
        &mut self,
        paint_type: &str,
        el: &DumpElement,
        children: Option<Vec<ViewIrNode>>,
    ) {
        let type_ = normalize_view_type(paint_type, &el.type_);
        self.nodes.push(ViewIrNode {
            type_,
            id: el.id.clone(),
            name: el.name.clone(),
            x: el.x,
            y: el.y,
            width: el.width,
            height: el.height,
            text: el.text.clone(),
            children,
        });
    }

    pub fn push_other_kind(&mut self, kind: &str, index: usize) {
        self.nodes.push(ViewIrNode {
            type_: map_other_kind(kind),
            id: format!("other-{}-{}", kind, index),
            name: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            text: None,
            children: None,
        });
    }
}

fn normalize_view_type(paint_type: &str, model_type: &str) -> String {
    if paint_type != "element" {
        return paint_type.to_owned();
    }
    if model_type == "picture" {
        return "image".to_owned();
    }
    model_type.to_owned()
}

fn map_other_kind(kind: &str) -> String {
    match kind {
        "media" => "video".to_owned(),
        "smartArt" => "smartArt".to_owned(),
        k => k.to_owned(),
    }
}

pub fn dump_element_tree(el: &DumpElement) -> ViewIrNode {
    let type_ = if el.type_ == "picture" {
        "image".to_owned()
    } else {
        el.type_.clone()
    };
    let children = el
        .children
        .as_ref()
        .map(|kids| kids.iter().map(dump_element_tree).collect());
    ViewIrNode {
        type_,
        id: el.id.clone(),
        name: el.name.clone(),
        x: el.x,
        y: el.y,
        width: el.width,
        height: el.height,
        text: el.text.clone(),
        children,
    }
}

pub fn dump_pptx_view_ir(pres: &mut Presentation) -> Result<Value, crate::presentation::Error> {
    pres.ensure_all_slides()?;
    let (width, height) = pres.canvas_size();
    let count = pres.slide_count();
    let slides: Vec<Value> = (0..count)
        .map(|index| {
            let path = pres.slide_paths()[index].clone();
            let slide = pres.slide(index).expect("parsed");
            let elements: Vec<Value> = slide
                .elements
                .iter()
                .map(|el| serde_json::to_value(dump_element_tree(el)).unwrap_or(json!({})))
                .collect();
            json!({
                "index": index,
                "name": path,
                "hidden": false,
                "width": width,
                "height": height,
                "elements": elements,
            })
        })
        .collect();
    Ok(json!({
        "format": "pptx",
        "slideCount": slides.len(),
        "slides": slides,
        "warnings": [],
    }))
}

pub fn dump_pptx_view_ir_string(
    pres: &mut Presentation,
) -> Result<String, crate::presentation::Error> {
    dump_pptx_view_ir(pres).map(|v| v.to_string())
}
