// SPDX-License-Identifier: MIT
//! Mutable pptx edit host: slide-owned elements plus text/fill overlays (#84).

use std::any::Any;
use std::collections::BTreeSet;

use nitro_xml_opc::drawingml::{
    patch_shape_fill, patch_shape_line, resolve_xfrm_emu, EMU_PER_PIXEL,
};
use nitro_xml_opc::edit::{EditHost, EditScope, HostSnapshot};
use serde::{Deserialize, Serialize};
use serde_json::{json, Number, Value};

/// Serialize a finite f64 the way `JSON.stringify` does (whole numbers as integers).
fn js_num(n: f64) -> Value {
    if n.is_finite() && n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
        Value::Number((n as i64).into())
    } else if let Some(num) = Number::from_f64(n) {
        Value::Number(num)
    } else {
        Value::Null
    }
}

fn js_opt(n: Option<f64>) -> Value {
    match n {
        Some(v) => js_num(v),
        None => Value::Null,
    }
}

use crate::presentation::Presentation;
use crate::slide::DumpElement;

use super::shapes::{self, BulletInfoSnap, TextSegmentSnap, TextStyleSnap};
use super::table::TableModel;

/// CSS-pixel geometry fields omitted from a patch keep the current value.
#[derive(Debug, Clone, Copy, Default)]
pub struct GeometryPatch {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub rotation: Option<f64>,
}

/// Fields merged into `textStyle`. `None` means leave the current value.
#[derive(Debug, Clone, Copy, Default)]
pub struct TextStylePatch<'a> {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub font_size: Option<f64>,
    pub color: Option<&'a str>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
}

/// A new shape appended at the front of the slide (z-order end).
#[derive(Debug, Clone, Copy)]
pub struct NewShape<'a> {
    pub shape_type: &'a str,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub id: Option<&'a str>,
}

/// One slide's editable element list (layout/master decorations are not included).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditableSlide {
    /// Stable rail id. Loaded slides use the part path (`ppt/slides/slide1.xml`).
    pub id: String,
    pub path: String,
    /// Speaker notes. `None` serialises as JSON null in the parity snapshot.
    #[serde(default)]
    pub notes: Option<String>,
    pub elements: Vec<EditableElement>,
    /// Part cloned on duplicate (`sourceSlideId`). Omitted from the parity snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_slide_id: Option<String>,
}

/// One thumbnail-rail cell. `preview` stays empty until a painter fills it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideThumbnail {
    pub slide_index: u32,
    pub id: String,
    pub preview: Option<Vec<u8>>,
}

/// Slim editor element. Snapshot fields match `parity/undo/snapshot.mjs` `snapshotPptx`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditableElement {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_emu: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_emu: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_emu: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_emu: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_style: Option<TextStyleSnap>,
    #[serde(default)]
    pub text_segments: Vec<TextSegmentSnap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<f64>,
    #[serde(default)]
    pub children: Vec<EditableElement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape_type: Option<String>,
    /// Index of this shape among the slide part's spTree children at open.
    /// `None` for shapes created in this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_sp_index: Option<usize>,
    /// When set, this element serializes through another element's source XML
    /// node (upstream `cloneElement` shares `rawXml`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml_share_of: Option<String>,
    /// Parsed `a:tbl`. Omitted from the snapshot when the element is not a table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<TableModel>,
    /// The table grid was inserted or edited this session, so save rewrites `a:tbl`.
    /// Omitted when false so an unedited host snapshot stays the same shape.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub table_authored: bool,
    /// PNG bytes for a picture inserted this session. Omitted so an unedited
    /// snapshot stays the same shape. Existing package pictures keep `None`
    /// and are rewritten from their source `p:pic`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_png: Option<Vec<u8>>,
}

impl EditableElement {
    pub fn blank(id: impl Into<String>, type_: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            type_: type_.into(),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            rotation: 0.0,
            x_emu: None,
            y_emu: None,
            width_emu: None,
            height_emu: None,
            text: None,
            text_style: None,
            text_segments: Vec::new(),
            fill_color: None,
            fill_mode: None,
            stroke_color: None,
            stroke_width: None,
            children: Vec::new(),
            shape_type: None,
            source_sp_index: None,
            xml_share_of: None,
            table: None,
            table_authored: false,
            image_png: None,
        }
    }

    /// Write pixel geometry and refresh stored EMUs (`resolve_xfrm_emu`).
    pub fn set_box(&mut self, x: f64, y: f64, width: f64, height: f64, rotation: f64) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
        self.rotation = rotation;
        self.sync_emus();
    }

    pub fn sync_emus(&mut self) {
        let emu = EMU_PER_PIXEL as f64;
        self.x_emu = Some(resolve_xfrm_emu(self.x, self.x_emu, emu));
        self.y_emu = Some(resolve_xfrm_emu(self.y, self.y_emu, emu));
        self.width_emu = Some(resolve_xfrm_emu(self.width, self.width_emu, emu));
        self.height_emu = Some(resolve_xfrm_emu(self.height, self.height_emu, emu));
    }

    pub fn patch_fill(&mut self, color: Option<Option<String>>, mode: Option<&str>) {
        patch_shape_fill(&mut self.fill_color, &mut self.fill_mode, color, mode);
    }

    pub fn patch_line(&mut self, color: Option<Option<String>>, width: Option<f64>) {
        patch_shape_line(&mut self.stroke_color, &mut self.stroke_width, color, width);
    }
}

/// In-memory presentation the shape command runners mutate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PptxHost {
    pub slides: Vec<EditableSlide>,
    /// OPC part names mutated since open (or since the restored snapshot).
    #[serde(default)]
    pub dirty_parts: BTreeSet<String>,
    /// Slide list changed (add, delete, duplicate, reorder). The package writer
    /// rebuilds `sldIdLst`, relationships and slide parts. Omitted when false so
    /// undo snapshots of an unedited host stay the same shape.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub structure_dirty: bool,
}

impl PptxHost {
    /// Load slide-owned elements. Layout/master decorations stay out of the editor model
    /// so element indexes match the TS `PptxHandler` slide list.
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let mut pres = Presentation::open(bytes).map_err(|e| e.to_string())?;
        pres.ensure_all_slides().map_err(|e| e.to_string())?;
        let count = pres.slide_count();
        let mut slides = Vec::with_capacity(count);
        for i in 0..count {
            let slide = pres.slide(i).map_err(|e| e.to_string())?;
            let path = slide.path.clone();
            let notes = slide.notes_text.clone();
            let elements = slide
                .elements
                .iter()
                .filter(|el| el.id.starts_with(&path))
                .enumerate()
                .map(|(i, el)| {
                    let mut editable = editable_from_dump(el);
                    editable.source_sp_index = Some(i);
                    editable
                })
                .collect();
            slides.push(EditableSlide {
                id: path.clone(),
                path,
                notes,
                elements,
                source_slide_id: None,
            });
        }
        Ok(Self {
            slides,
            dirty_parts: BTreeSet::new(),
            structure_dirty: false,
        })
    }

    /// True when a shape command has marked a slide part, or the slide list changed.
    pub fn is_dirty(&self) -> bool {
        self.structure_dirty || !self.dirty_parts.is_empty()
    }

    pub(crate) fn touch(&mut self, slide_index: u32) -> Result<(), String> {
        let path = self
            .slides
            .get(slide_index as usize)
            .ok_or_else(|| format!("slide {slide_index} missing"))?
            .path
            .clone();
        self.dirty_parts.insert(path);
        Ok(())
    }

    pub fn slide_mut(&mut self, index: u32) -> Result<&mut EditableSlide, String> {
        self.slides
            .get_mut(index as usize)
            .ok_or_else(|| format!("slide {index} missing"))
    }

    pub fn slide(&self, index: u32) -> Result<&EditableSlide, String> {
        self.slides
            .get(index as usize)
            .ok_or_else(|| format!("slide {index} missing"))
    }

    /// `snapshotPptx` projection (also the undo-parity snapshot).
    pub fn model_snapshot(&self) -> Value {
        self.undo_parity_snapshot()
    }

    /// Thumbnail rail in slide order. Preview bytes are a stub (`None`).
    pub fn thumbnails(&self) -> Vec<SlideThumbnail> {
        self.slides
            .iter()
            .enumerate()
            .map(|(slide_index, slide)| SlideThumbnail {
                slide_index: slide_index as u32,
                id: slide.id.clone(),
                preview: None,
            })
            .collect()
    }

    pub fn undo_parity_snapshot(&self) -> Value {
        let slides: Vec<Value> = self
            .slides
            .iter()
            .enumerate()
            .map(|(slide_index, slide)| {
                json!({
                    "slideIndex": slide_index,
                    "id": slide.id,
                    "notes": slide.notes,
                    "elements": slide.elements.iter().map(project_element).collect::<Vec<_>>(),
                })
            })
            .collect();
        let thumbnails: Vec<Value> = slides
            .iter()
            .map(|slide| {
                json!({
                    "slideIndex": slide["slideIndex"],
                    "id": slide["id"],
                    "preview": Value::Null,
                })
            })
            .collect();
        json!({
            "format": "pptx",
            "slides": slides,
            "thumbnails": thumbnails,
        })
    }
}

fn editable_from_dump(el: &DumpElement) -> EditableElement {
    // Own geometry matches the TS editor model (inherited xfrm is already folded
    // into x/y by the parser when the shape has no own transform — same as upstream).
    let editor = el.editor_text.as_ref();
    let has_text = el.text.is_some() || editor.is_some();
    let text_style = if has_text {
        Some(TextStyleSnap {
            bold: editor.map(|e| e.bold).unwrap_or(false),
            italic: editor.map(|e| e.italic).unwrap_or(false),
            // Authored `a:rPr` only. Paint-resolved theme sizes stay unset so
            // `gen-pptx-slides-10` (no run properties) keeps fontSize/color null.
            font_size: editor.and_then(|e| e.font_size),
            color: editor.and_then(|e| e.color.clone()),
            underline: None,
            strike: None,
            auto_fit: editor.and_then(|e| e.auto_fit),
            auto_fit_mode: editor.and_then(|e| e.auto_fit_mode.clone()),
        })
    } else {
        None
    };
    let text_segments = if let Some(editor) = editor.filter(|e| !e.segments.is_empty()) {
        editor.segments.clone()
    } else {
        match &el.text {
            Some(text) => vec![TextSegmentSnap {
                text: text.clone(),
                ..TextSegmentSnap::default()
            }],
            None => Vec::new(),
        }
    };
    EditableElement {
        id: el.id.clone(),
        type_: el.type_.clone(),
        x: el.x,
        y: el.y,
        width: el.width,
        height: el.height,
        rotation: el.rotation.unwrap_or(0.0),
        x_emu: el.x_emu,
        y_emu: el.y_emu,
        width_emu: el.width_emu,
        height_emu: el.height_emu,
        text: el.text.clone(),
        text_style,
        text_segments,
        // Resolved theme paints stay off the editor model (gen-pptx has no authored
        // fill). An authored `a:noFill` is `fillMode: none` + `fillColor: transparent`.
        fill_color: match el.fill_mode.as_deref() {
            Some("none") => Some("transparent".to_owned()),
            _ => None,
        },
        fill_mode: match el.fill_mode.as_deref() {
            Some("none") => Some("none".to_owned()),
            _ => None,
        },
        stroke_color: None,
        stroke_width: None,
        children: el
            .children
            .as_ref()
            .map(|kids| kids.iter().map(editable_from_dump).collect())
            .unwrap_or_default(),
        shape_type: el.shape_type.clone(),
        source_sp_index: None,
        xml_share_of: None,
        table: el.table_data.as_ref().map(TableModel::from_parsed),
        table_authored: false,
        image_png: None,
    }
}

fn project_element(el: &EditableElement) -> Value {
    let children = if el.type_ == "group" || !el.children.is_empty() {
        Value::Array(el.children.iter().map(project_element).collect())
    } else {
        Value::Null
    };
    let mut value = json!({
        "id": el.id,
        "x": js_num(el.x),
        "y": js_num(el.y),
        "width": js_num(el.width),
        "height": js_num(el.height),
        "rotation": js_num(el.rotation),
        "text": el.text,
        "textStyle": project_style(el.text_style.as_ref()),
        "textSegments": el.text_segments.iter().map(project_segment).collect::<Vec<_>>(),
        "fillColor": el.fill_color,
        "fillMode": el.fill_mode,
        "strokeColor": el.stroke_color,
        "strokeWidth": js_opt(el.stroke_width),
        "children": children,
    });
    if let Some(table) = &el.table {
        value["tableData"] = project_table(table);
    }
    value
}

fn project_segment(seg: &TextSegmentSnap) -> Value {
    let mut value = json!({
        "text": seg.text,
        "bold": seg.bold,
        "italic": seg.italic,
        "fontSize": js_opt(seg.font_size),
        "color": seg.color,
    });
    if seg.underline {
        value["underline"] = Value::Bool(true);
    }
    if let Some(align) = &seg.align {
        value["align"] = Value::String(align.clone());
    }
    if let Some(info) = project_bullet(seg.bullet_info.as_ref()) {
        value["bulletInfo"] = info;
    }
    value
}

fn project_bullet(info: Option<&BulletInfoSnap>) -> Option<Value> {
    let info = info?;
    let mut value = serde_json::Map::new();
    if let Some(ch) = &info.char {
        value.insert("char".to_owned(), Value::String(ch.clone()));
    }
    if let Some(kind) = &info.auto_num_type {
        value.insert("autoNumType".to_owned(), Value::String(kind.clone()));
    }
    if info.auto_num_type.is_some() {
        if let Some(start) = info.auto_num_start_at {
            value.insert("autoNumStartAt".to_owned(), json!(start));
        }
        if let Some(index) = info.paragraph_index {
            value.insert("paragraphIndex".to_owned(), json!(index));
        }
    }
    if info.none {
        value.insert("none".to_owned(), Value::Bool(true));
    }
    if value.is_empty() {
        None
    } else {
        Some(Value::Object(value))
    }
}

fn project_table(table: &TableModel) -> Value {
    json!({
        "columnWidths": table.column_widths.iter().copied().map(js_num).collect::<Vec<_>>(),
        "rows": table.rows.iter().map(|row| {
            let mut value = json!({
                "cells": row.cells.iter().map(project_cell).collect::<Vec<_>>(),
            });
            if let Some(height) = row.height {
                if height != 0.0 {
                    value["height"] = js_num(height);
                }
            }
            value
        }).collect::<Vec<_>>(),
    })
}

fn project_cell(cell: &super::table::TableCellModel) -> Value {
    let mut value = json!({ "text": cell.text });
    if cell.grid_span > 1 {
        value["gridSpan"] = json!(cell.grid_span);
    }
    if cell.row_span > 1 {
        value["rowSpan"] = json!(cell.row_span);
    }
    if cell.h_merge {
        value["hMerge"] = Value::Bool(true);
    }
    if cell.v_merge {
        value["vMerge"] = Value::Bool(true);
    }
    value
}

fn project_style(style: Option<&TextStyleSnap>) -> Value {
    match style {
        None => Value::Null,
        Some(s) => {
            let mut value = json!({
                "bold": s.bold,
                "italic": s.italic,
                "fontSize": js_opt(s.font_size),
                "color": s.color,
            });
            if let Some(auto_fit) = s.auto_fit {
                value["autoFit"] = Value::Bool(auto_fit);
            }
            if let Some(mode) = &s.auto_fit_mode {
                value["autoFitMode"] = Value::String(mode.clone());
            }
            value
        }
    }
}

impl PptxHost {
    pub fn resolve_index(
        slide: &EditableSlide,
        element_id: Option<&str>,
        element_index: Option<u32>,
    ) -> Result<usize, String> {
        if let Some(id) = element_id {
            return slide
                .elements
                .iter()
                .position(|el| el.id == id)
                .ok_or_else(|| format!("element {id} missing"));
        }
        let idx = element_index.unwrap_or(0) as usize;
        if idx >= slide.elements.len() {
            return Err(format!(
                "element index {idx} missing on slide {}",
                slide.path
            ));
        }
        Ok(idx)
    }

    pub fn set_shape_text(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        text: &str,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        let el = &mut slide.elements[idx];
        let segments =
            shapes::remap_text_to_segments(text, &el.text_segments, el.text_style.as_ref());
        el.text = Some(text.to_owned());
        el.text_segments = segments;
        self.touch(slide_index)
    }

    pub fn patch_geometry(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        patch: GeometryPatch,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        let el = &mut slide.elements[idx];
        el.set_box(
            patch.x.unwrap_or(el.x),
            patch.y.unwrap_or(el.y),
            patch.width.unwrap_or(el.width),
            patch.height.unwrap_or(el.height),
            patch.rotation.unwrap_or(el.rotation),
        );
        self.touch(slide_index)
    }

    pub fn set_text_style(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        patch: TextStylePatch<'_>,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        let el = &mut slide.elements[idx];
        {
            let style = el.text_style.get_or_insert_with(TextStyleSnap::default);
            if let Some(bold) = patch.bold {
                style.bold = bold;
            }
            if let Some(italic) = patch.italic {
                style.italic = italic;
            }
            if let Some(font_size) = patch.font_size {
                style.font_size = Some(font_size);
            }
            if let Some(color) = patch.color {
                style.color = Some(color.to_owned());
            }
            if let Some(underline) = patch.underline {
                style.underline = Some(underline);
            }
            if let Some(strike) = patch.strike {
                style.strike = Some(strike);
            }
        }
        if let Some(underline) = patch.underline {
            for seg in &mut el.text_segments {
                if seg.is_paragraph_break || seg.is_line_break {
                    continue;
                }
                seg.underline = underline;
            }
        }
        if let Some(strike) = patch.strike {
            for seg in &mut el.text_segments {
                if seg.is_paragraph_break || seg.is_line_break {
                    continue;
                }
                seg.strike = strike;
            }
        }
        self.touch(slide_index)
    }

    pub fn add_shape(&mut self, slide_index: u32, shape: NewShape<'_>) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let id = shape
            .id
            .map(str::to_owned)
            .unwrap_or_else(|| format!("added-shape-{slide_index}-{}", slide.elements.len()));
        let mut el = EditableElement::blank(id, "shape");
        el.shape_type = Some(shape.shape_type.to_owned());
        el.set_box(shape.x, shape.y, shape.width, shape.height, 0.0);
        // ElementFactory default solid fill.
        el.patch_fill(
            Some(Some(shapes::DEFAULT_SHAPE_FILL.to_owned())),
            Some("solid"),
        );
        slide.elements.push(el);
        self.touch(slide_index)
    }

    /// Append a 2×2 table (`p:graphicFrame` / `a:tbl`) on the slide.
    pub fn add_table(
        &mut self,
        slide_index: u32,
        id: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), String> {
        {
            let slide = self.slide_mut(slide_index)?;
            let mut el = EditableElement::blank(id, "table");
            el.set_box(x, y, width, height, 0.0);
            el.table = Some(super::table::TableModel::blank_grid(2, 2));
            el.table_authored = true;
            slide.elements.push(el);
        }
        self.touch(slide_index)
    }

    /// Append a picture (`p:pic` / `a:blip`) at a fixed box. `png` is written to
    /// `ppt/media` on save. Fill mode stays unset so `apply_fill` does not replace the blip.
    pub fn add_picture(&mut self, slide_index: u32, id: &str, png: &[u8]) -> Result<(), String> {
        {
            let slide = self.slide_mut(slide_index)?;
            let mut el = EditableElement::blank(id, "picture");
            el.set_box(120.0, 120.0, 160.0, 120.0, 0.0);
            el.image_png = Some(png.to_vec());
            slide.elements.push(el);
        }
        self.touch(slide_index)
    }

    /// Append a text box (`p:cNvSpPr/@txBox="1"`) on the slide.
    pub fn add_text_box(
        &mut self,
        slide_index: u32,
        id: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let mut el = EditableElement::blank(id, "text");
        el.shape_type = Some("rect".to_owned());
        el.set_box(x, y, width, height, 0.0);
        el.text = Some(String::new());
        el.text_segments
            .push(crate::edit::shapes::TextSegmentSnap::default());
        el.patch_fill(Some(None), Some("none"));
        slide.elements.push(el);
        self.touch(slide_index)
    }

    pub fn delete_shape(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        slide.elements.remove(idx);
        self.touch(slide_index)
    }

    pub fn duplicate_shape(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        new_id: Option<&str>,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        let mut copy = slide.elements[idx].clone();
        let source_id = copy.id.clone();
        // Upstream `cloneElement` shares `rawXml`. A shape that came from the
        // package (or already aliases one) keeps that node; an in-session
        // shape has no raw XML and is serialized on its own.
        let share = copy
            .xml_share_of
            .clone()
            .or_else(|| copy.source_sp_index.map(|_| source_id.clone()));
        copy.id = new_id
            .map(str::to_owned)
            .unwrap_or_else(|| format!("dup-{source_id}"));
        copy.source_sp_index = None;
        copy.xml_share_of = share;
        copy.x += shapes::DUPLICATE_OFFSET_PX;
        copy.y += shapes::DUPLICATE_OFFSET_PX;
        copy.sync_emus();
        slide.elements.push(copy);
        self.touch(slide_index)
    }

    pub fn reorder(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        op: shapes::ZOrder,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        let id = slide.elements[idx].id.clone();
        slide.elements = shapes::reorder(&slide.elements, &id, op);
        self.touch(slide_index)
    }

    pub fn group_shapes(
        &mut self,
        slide_index: u32,
        element_ids: &[String],
        group_id: &str,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        slide.elements = shapes::group_elements(&slide.elements, element_ids, group_id);
        self.touch(slide_index)
    }

    pub fn ungroup_shapes(
        &mut self,
        slide_index: u32,
        group_id: &str,
        child_ids: &[String],
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let found = slide.elements.iter().any(|el| el.id == group_id);
        if !found {
            return Err(format!("element {group_id} missing"));
        }
        slide.elements = shapes::ungroup_elements(&slide.elements, group_id, child_ids);
        self.touch(slide_index)
    }

    pub fn set_fill(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        color: Option<Option<String>>,
        mode: Option<&str>,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        slide.elements[idx].patch_fill(color, mode);
        self.touch(slide_index)
    }

    pub fn set_line(
        &mut self,
        slide_index: u32,
        element_id: Option<&str>,
        element_index: Option<u32>,
        color: Option<Option<String>>,
        width: Option<f64>,
    ) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        let idx = Self::resolve_index(slide, element_id, element_index)?;
        slide.elements[idx].patch_line(color, width);
        self.touch(slide_index)
    }

    /// Insert a blank slide at `at` (append when `at` is `None`). Returns the new index.
    pub fn add_slide(&mut self, at: Option<u32>) -> Result<u32, String> {
        let len = self.slides.len();
        let index = at.map(|a| (a as usize).min(len)).unwrap_or(len);
        let id = format!("added-slide-{len}");
        self.slides.insert(
            index,
            EditableSlide {
                id: id.clone(),
                path: id,
                notes: None,
                elements: Vec::new(),
                source_slide_id: None,
            },
        );
        self.structure_dirty = true;
        Ok(index as u32)
    }

    /// Remove a slide. The last remaining slide is rejected.
    pub fn delete_slide(&mut self, index: u32) -> Result<(), String> {
        if self.slides.len() <= 1 {
            return Err("cannot delete the last slide".into());
        }
        let index = index as usize;
        if index >= self.slides.len() {
            return Err(format!("slide index {index} out of range"));
        }
        self.slides.remove(index);
        self.structure_dirty = true;
        Ok(())
    }

    /// Deep-copy the slide at `index` and insert the copy immediately after it.
    pub fn duplicate_slide(&mut self, index: u32) -> Result<u32, String> {
        let index = index as usize;
        if index >= self.slides.len() {
            return Err(format!("slide index {index} out of range"));
        }
        let len = self.slides.len();
        let source_id = self.slides[index].id.clone();
        let inherited = if source_id.starts_with("ppt/slides/") {
            source_id
        } else {
            self.slides[index]
                .source_slide_id
                .clone()
                .unwrap_or_else(|| self.slides[index].id.clone())
        };
        let mut copy = self.slides[index].clone();
        let id = format!("dup-slide-{len}");
        copy.id = id.clone();
        copy.path = id;
        copy.source_slide_id = Some(inherited);
        let prefix = format!("dup-{len}-");
        for el in &mut copy.elements {
            reassign_element_id(el, &prefix);
        }
        let insert_at = index + 1;
        self.slides.insert(insert_at, copy);
        self.structure_dirty = true;
        Ok(insert_at as u32)
    }

    /// Move the slide at `from` to index `to`. `from == to` is a no-op.
    pub fn reorder_slide(&mut self, from: u32, to: u32) -> Result<(), String> {
        let len = self.slides.len();
        let from = from as usize;
        let to = to as usize;
        if from >= len || to >= len {
            return Err("slide index out of range".into());
        }
        if from == to {
            return Ok(());
        }
        let slide = self.slides.remove(from);
        self.slides.insert(to, slide);
        self.structure_dirty = true;
        Ok(())
    }

    pub fn set_notes_text(&mut self, slide_index: u32, text: &str) -> Result<(), String> {
        let slide = self.slide_mut(slide_index)?;
        slide.notes = Some(text.to_owned());
        Ok(())
    }
}

fn reassign_element_id(el: &mut EditableElement, prefix: &str) {
    el.id = format!("{prefix}{}", el.id);
    for child in &mut el.children {
        reassign_element_id(child, prefix);
    }
}

impl EditHost for PptxHost {
    fn capture(&self, _scopes: &[EditScope]) -> HostSnapshot {
        HostSnapshot(serde_json::to_vec(self).unwrap_or_default())
    }

    fn restore(&mut self, snap: &HostSnapshot) {
        if snap.0.is_empty() {
            return;
        }
        if let Ok(restored) = serde_json::from_slice::<PptxHost>(&snap.0) {
            *self = restored;
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn as_any_ref(&self) -> &dyn Any {
        self
    }
}
