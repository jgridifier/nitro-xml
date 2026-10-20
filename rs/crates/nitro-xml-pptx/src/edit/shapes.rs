// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/editor/render/element-operations.ts,
// src/core/pptx/editor/render/remap-text.ts (plain-text path),
// src/core/pptx/core/utils/group-ops.ts and
// src/core/pptx/editor/render/editor-mutations.ts; changes listed in PROVENANCE.md.

//! Shape command runners: geometry, text, z-order, group, fill and line (#84).

use std::any::Any;
use std::collections::HashSet;

use nitro_xml_opc::drawingml::{resolve_xfrm_emu, EMU_PER_PIXEL};
use nitro_xml_opc::edit::{ChangeKind, ChangeSet, Command, CommandRegistry, EditError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::host::{EditableElement, GeometryPatch, NewShape, PptxHost, TextStylePatch};

/// Duplicate nudge in CSS pixels (`DUPLICATE_OFFSET_PX` / `DEFAULT_DUPLICATE_OFFSET`).
pub const DUPLICATE_OFFSET_PX: f64 = 20.0;

/// Default solid fill from `createShapeElement` (`ElementFactory`).
pub const DEFAULT_SHAPE_FILL: &str = "#4472C4";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZOrder {
    BringForward,
    SendBackward,
    BringToFront,
    SendToBack,
}

/// Editor text style projected by `snapshotPptx` (bold/italic/fontSize/color).
/// `autoFit` / `autoFitMode` are omitted from the snapshot until an edit sets them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TextStyleSnap {
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// `Some` means the edit authored `a:rPr/@u` (`sng` or absent). `None` leaves the file's underline alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    /// `Some` means the edit authored `a:rPr/@strike` (`sngStrike` or absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_fit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_fit_mode: Option<String>,
}

/// Bullet metadata carried on the first segment of a paragraph (`bulletInfo`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BulletInfoSnap {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub char: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_num_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_num_start_at: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraph_index: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub none: bool,
}

/// One text segment. Style flags are the snapshot fields (`seg.bold`, not `seg.style`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TextSegmentSnap {
    pub text: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strike: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Paragraph `a:pPr/@algn` on the first segment of a paragraph (`center`, not `ctr`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bullet_info: Option<BulletInfoSnap>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_paragraph_break: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_line_break: bool,
}

impl TextSegmentSnap {
    fn from_style(text: impl Into<String>, style: Option<&TextStyleSnap>) -> Self {
        let style = style.cloned().unwrap_or_default();
        Self {
            text: text.into(),
            bold: style.bold,
            italic: style.italic,
            font_size: style.font_size,
            color: style.color,
            ..Self::default()
        }
    }

    pub(super) fn copy_run_style(&self) -> Self {
        Self {
            text: String::new(),
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            strike: self.strike,
            font_size: self.font_size,
            color: self.color.clone(),
            ..Self::default()
        }
    }
}

/// Minimal `remapTextToSegments`: paragraph split on `\n`, proportional runs,
/// leftover characters stay on the last segment. Bullet/field/equation metadata
/// is out of scope for the edit-script corpus.
pub fn remap_text_to_segments(
    new_text: &str,
    original: &[TextSegmentSnap],
    element_style: Option<&TextStyleSnap>,
) -> Vec<TextSegmentSnap> {
    if original.is_empty() {
        return vec![TextSegmentSnap::from_style(new_text, element_style)];
    }

    let mut original_paras: Vec<Vec<&TextSegmentSnap>> = vec![Vec::new()];
    for seg in original {
        if seg.text == "\n" {
            original_paras.push(Vec::new());
        } else {
            original_paras.last_mut().expect("para").push(seg);
        }
    }
    let new_paras: Vec<&str> = new_text.split('\n').collect();
    let mut output = Vec::new();
    for (pi, para) in new_paras.iter().enumerate() {
        if pi > 0 {
            let break_style = original_paras
                .get(pi - 1)
                .and_then(|p| p.first().copied())
                .or_else(|| original.iter().find(|s| s.text != "\n"));
            output.push(TextSegmentSnap {
                text: "\n".to_owned(),
                bold: break_style.map(|s| s.bold).unwrap_or(false),
                italic: break_style.map(|s| s.italic).unwrap_or(false),
                underline: break_style.map(|s| s.underline).unwrap_or(false),
                strike: break_style.map(|s| s.strike).unwrap_or(false),
                font_size: break_style.and_then(|s| s.font_size),
                color: break_style.and_then(|s| s.color.clone()),
                is_paragraph_break: true,
                ..TextSegmentSnap::default()
            });
        }
        let orig = original_paras.get(pi).map(Vec::as_slice).unwrap_or(&[]);
        output.extend(remap_paragraph(para, orig, element_style));
    }
    if output.is_empty() {
        output.push(TextSegmentSnap::from_style("", element_style));
    }
    output
}

fn remap_paragraph(
    para_new: &str,
    orig: &[&TextSegmentSnap],
    element_style: Option<&TextStyleSnap>,
) -> Vec<TextSegmentSnap> {
    if orig.is_empty() {
        return vec![TextSegmentSnap::from_style(para_new, element_style)];
    }
    if para_new.is_empty() {
        let first = orig[0];
        return vec![TextSegmentSnap {
            text: String::new(),
            bold: first.bold,
            italic: first.italic,
            underline: first.underline,
            strike: first.strike,
            font_size: first.font_size,
            color: first.color.clone(),
            align: first.align.clone(),
            bullet_info: first.bullet_info.clone(),
            ..TextSegmentSnap::default()
        }];
    }
    let total: usize = orig.iter().map(|s| s.text.len()).sum();
    if total == 0 {
        let first = orig[0];
        return vec![TextSegmentSnap {
            text: para_new.to_owned(),
            bold: first.bold,
            italic: first.italic,
            underline: first.underline,
            strike: first.strike,
            font_size: first.font_size,
            color: first.color.clone(),
            align: first.align.clone(),
            bullet_info: first.bullet_info.clone(),
            ..TextSegmentSnap::default()
        }];
    }
    let mut remapped = Vec::new();
    let mut new_pos = 0usize;
    let chars: Vec<char> = para_new.chars().collect();
    for (i, orig_seg) in orig.iter().enumerate() {
        if new_pos >= chars.len() {
            break;
        }
        let is_last = i + 1 == orig.len();
        let orig_len = orig_seg.text.chars().count();
        let seg_text: String = if is_last {
            chars[new_pos..].iter().collect()
        } else {
            let end = (new_pos + orig_len).min(chars.len());
            chars[new_pos..end].iter().collect()
        };
        let advance = if is_last {
            chars.len() - new_pos
        } else {
            orig_len
        };
        if !seg_text.is_empty() {
            remapped.push(TextSegmentSnap {
                text: seg_text,
                bold: orig_seg.bold,
                italic: orig_seg.italic,
                underline: orig_seg.underline,
                strike: orig_seg.strike,
                font_size: orig_seg.font_size,
                color: orig_seg.color.clone(),
                align: if i == 0 { orig_seg.align.clone() } else { None },
                bullet_info: if i == 0 {
                    orig_seg.bullet_info.clone()
                } else {
                    None
                },
                ..TextSegmentSnap::default()
            });
        }
        new_pos += advance;
    }
    if remapped.is_empty() {
        let first = orig[0];
        remapped.push(TextSegmentSnap {
            text: para_new.to_owned(),
            bold: first.bold,
            italic: first.italic,
            underline: first.underline,
            strike: first.strike,
            font_size: first.font_size,
            color: first.color.clone(),
            align: first.align.clone(),
            bullet_info: first.bullet_info.clone(),
            ..TextSegmentSnap::default()
        });
    }
    remapped
}

pub fn reorder(elements: &[EditableElement], id: &str, op: ZOrder) -> Vec<EditableElement> {
    let Some(idx) = elements.iter().position(|el| el.id == id) else {
        return elements.to_vec();
    };
    match op {
        ZOrder::BringToFront => {
            if idx + 1 == elements.len() {
                return elements.to_vec();
            }
            let mut rest: Vec<_> = elements
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != idx)
                .map(|(_, el)| el.clone())
                .collect();
            rest.push(elements[idx].clone());
            rest
        }
        ZOrder::SendToBack => {
            if idx == 0 {
                return elements.to_vec();
            }
            let mut out = Vec::with_capacity(elements.len());
            out.push(elements[idx].clone());
            out.extend(
                elements
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != idx)
                    .map(|(_, el)| el.clone()),
            );
            out
        }
        ZOrder::BringForward => {
            if idx + 1 == elements.len() {
                return elements.to_vec();
            }
            let mut next = elements.to_vec();
            next.swap(idx, idx + 1);
            next
        }
        ZOrder::SendBackward => {
            if idx == 0 {
                return elements.to_vec();
            }
            let mut next = elements.to_vec();
            next.swap(idx, idx - 1);
            next
        }
    }
}

/// `groupElements`: union bbox, children relative to the group's top-left,
/// inserted where the lowest-index member was. Fewer than 2 matches is a no-op.
pub fn group_elements(
    elements: &[EditableElement],
    ids: &[String],
    group_id: &str,
) -> Vec<EditableElement> {
    let id_set: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut gathered: Vec<&EditableElement> = Vec::new();
    let mut insertion_index = None;
    for (i, el) in elements.iter().enumerate() {
        if id_set.contains(el.id.as_str()) {
            if insertion_index.is_none() {
                insertion_index = Some(i);
            }
            gathered.push(el);
        }
    }
    if gathered.len() < 2 {
        return elements.to_vec();
    }
    let mut min_x = gathered[0].x;
    let mut min_y = gathered[0].y;
    let mut max_x = gathered[0].x + gathered[0].width;
    let mut max_y = gathered[0].y + gathered[0].height;
    for el in gathered.iter().skip(1) {
        min_x = min_x.min(el.x);
        min_y = min_y.min(el.y);
        max_x = max_x.max(el.x + el.width);
        max_y = max_y.max(el.y + el.height);
    }
    let emu = EMU_PER_PIXEL as f64;
    let children: Vec<EditableElement> = gathered
        .iter()
        .map(|el| {
            let mut child = (*el).clone();
            child.x = el.x - min_x;
            child.y = el.y - min_y;
            child.x_emu = Some(resolve_xfrm_emu(child.x, child.x_emu, emu));
            child.y_emu = Some(resolve_xfrm_emu(child.y, child.y_emu, emu));
            child
        })
        .collect();
    let mut group = EditableElement::blank(group_id, "group");
    group.x = min_x;
    group.y = min_y;
    group.width = max_x - min_x;
    group.height = max_y - min_y;
    group.x_emu = Some(resolve_xfrm_emu(group.x, None, emu));
    group.y_emu = Some(resolve_xfrm_emu(group.y, None, emu));
    group.width_emu = Some(resolve_xfrm_emu(group.width, None, emu));
    group.height_emu = Some(resolve_xfrm_emu(group.height, None, emu));
    group.children = children;

    let remaining: Vec<EditableElement> = elements
        .iter()
        .filter(|el| !id_set.contains(el.id.as_str()))
        .cloned()
        .collect();
    let insertion = insertion_index.unwrap_or(0);
    let mut adjusted = 0usize;
    for el in elements.iter().take(insertion) {
        if !id_set.contains(el.id.as_str()) {
            adjusted += 1;
        }
    }
    let mut out = remaining;
    out.insert(adjusted, group);
    out
}

/// `ungroupElements`: children return to slide space. A missing `childIds` entry keeps the child id.
pub fn ungroup_elements(
    elements: &[EditableElement],
    group_id: &str,
    child_ids: &[String],
) -> Vec<EditableElement> {
    let Some(group_index) = elements.iter().position(|el| el.id == group_id) else {
        return elements.to_vec();
    };
    let group = &elements[group_index];
    if group.type_ != "group" {
        return elements.to_vec();
    }
    let emu = EMU_PER_PIXEL as f64;
    let expanded: Vec<EditableElement> = group
        .children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            let mut promoted = child.clone();
            if let Some(id) = child_ids.get(index) {
                if !id.is_empty() {
                    promoted.id = id.clone();
                }
            }
            promoted.x = child.x + group.x;
            promoted.y = child.y + group.y;
            promoted.x_emu = Some(resolve_xfrm_emu(promoted.x, promoted.x_emu, emu));
            promoted.y_emu = Some(resolve_xfrm_emu(promoted.y, promoted.y_emu, emu));
            promoted
        })
        .collect();
    let mut out = Vec::with_capacity(elements.len() - 1 + expanded.len());
    out.extend(elements[..group_index].iter().cloned());
    out.extend(expanded);
    out.extend(elements[group_index + 1..].iter().cloned());
    out
}

pub(super) fn host_mut(host: &mut dyn Any) -> Result<&mut PptxHost, EditError> {
    host.downcast_mut::<PptxHost>()
        .ok_or_else(|| EditError::Other {
            message: "host is not PptxHost".into(),
        })
}

pub(super) fn map_err(err: String) -> EditError {
    EditError::Other { message: err }
}

pub(super) fn arg_u32_opt(args: &Value, key: &str) -> Option<u32> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as u32)
}

pub(super) fn arg_u32(args: &Value, key: &str) -> Result<u32, EditError> {
    arg_u32_opt(args, key).ok_or_else(|| EditError::Other {
        message: format!("missing or invalid `{key}`"),
    })
}

pub(super) fn arg_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, EditError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| EditError::Other {
            message: format!("missing `{key}`"),
        })
}

pub(super) fn arg_f64_opt(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(|v| v.as_f64())
}

pub(super) fn arg_bool_opt(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

pub(super) fn arg_id(args: &Value) -> Option<&str> {
    args.get("elementId").and_then(|v| v.as_str())
}

/// `None` = key absent, `Some(None)` = JSON null, `Some(Some)` = string.
fn arg_nullable_string(args: &Value, key: &str) -> Option<Option<String>> {
    match args.get(key) {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(s)) => Some(Some(s.clone())),
        Some(_) => Some(None),
    }
}

fn string_list(args: &Value, key: &str) -> Result<Vec<String>, EditError> {
    let arr = args
        .get(key)
        .and_then(|v| v.as_array())
        .ok_or_else(|| EditError::Other {
            message: format!("missing `{key}`"),
        })?;
    Ok(arr
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect())
}

fn change(host: &PptxHost, slide: u32, kind: ChangeKind, label: &str) -> ChangeSet {
    let mut cs = ChangeSet::new(kind, label);
    if let Ok(slide) = host.slide(slide) {
        cs.dirty_parts.push(slide.path.clone());
    }
    cs
}

/// Register every pptx shape op from `edits/schema.json`.
pub fn register_shape_commands(registry: &mut CommandRegistry) {
    registry.register(
        Command::stub("setShapeText", "setShapeText").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let text = arg_str(args, "text")?.to_owned();
            h.set_shape_text(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                &text,
            )
            .map_err(map_err)?;
            Ok(change(h, slide, ChangeKind::Other, "setShapeText"))
        }),
    );
    registry.register(
        Command::stub("patchShapeGeometry", "patchShapeGeometry").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            h.patch_geometry(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                GeometryPatch {
                    x: arg_f64_opt(args, "x"),
                    y: arg_f64_opt(args, "y"),
                    width: arg_f64_opt(args, "width"),
                    height: arg_f64_opt(args, "height"),
                    rotation: arg_f64_opt(args, "rotation"),
                },
            )
            .map_err(map_err)?;
            Ok(change(
                h,
                slide,
                ChangeKind::Structure,
                "patchShapeGeometry",
            ))
        }),
    );
    registry.register(
        Command::stub("setShapeTextStyle", "setShapeTextStyle").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let color = args
                .get("color")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            h.set_text_style(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                TextStylePatch {
                    bold: arg_bool_opt(args, "bold"),
                    italic: arg_bool_opt(args, "italic"),
                    font_size: arg_f64_opt(args, "fontSize"),
                    color: color.as_deref(),
                    underline: arg_bool_opt(args, "underline"),
                    strike: arg_bool_opt(args, "strike"),
                },
            )
            .map_err(map_err)?;
            Ok(change(h, slide, ChangeKind::Format, "setShapeTextStyle"))
        }),
    );
    registry.register(
        Command::stub("addShape", "addShape").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let shape_type = arg_str(args, "shapeType")?;
            let x = arg_f64_opt(args, "x").ok_or_else(|| EditError::Other {
                message: "addShape missing x".into(),
            })?;
            let y = arg_f64_opt(args, "y").ok_or_else(|| EditError::Other {
                message: "addShape missing y".into(),
            })?;
            let width = arg_f64_opt(args, "width").ok_or_else(|| EditError::Other {
                message: "addShape missing width".into(),
            })?;
            let height = arg_f64_opt(args, "height").ok_or_else(|| EditError::Other {
                message: "addShape missing height".into(),
            })?;
            let id = args.get("id").and_then(|v| v.as_str());
            h.add_shape(
                slide,
                NewShape {
                    shape_type,
                    x,
                    y,
                    width,
                    height,
                    id,
                },
            )
            .map_err(map_err)?;
            Ok(change(h, slide, ChangeKind::Structure, "addShape"))
        }),
    );
    registry.register(
        Command::stub("deleteShape", "deleteShape").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            h.delete_shape(slide, arg_id(args), arg_u32_opt(args, "elementIndex"))
                .map_err(map_err)?;
            Ok(change(h, slide, ChangeKind::Structure, "deleteShape"))
        }),
    );
    registry.register(
        Command::stub("duplicateShape", "duplicateShape").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let new_id = args.get("newId").and_then(|v| v.as_str());
            h.duplicate_shape(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                new_id,
            )
            .map_err(map_err)?;
            Ok(change(h, slide, ChangeKind::Structure, "duplicateShape"))
        }),
    );
    for (id, op) in [
        ("bringForward", ZOrder::BringForward),
        ("sendBackward", ZOrder::SendBackward),
        ("bringToFront", ZOrder::BringToFront),
        ("sendToBack", ZOrder::SendToBack),
    ] {
        registry.register(Command::stub(id, id).with_runner(move |host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            h.reorder(slide, arg_id(args), arg_u32_opt(args, "elementIndex"), op)
                .map_err(map_err)?;
            Ok(change(h, slide, ChangeKind::Structure, id))
        }));
    }
    registry.register(
        Command::stub("groupShapes", "groupShapes").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let ids = string_list(args, "elementIds")?;
            let group_id = arg_str(args, "groupId")?.to_owned();
            h.group_shapes(slide, &ids, &group_id).map_err(map_err)?;
            Ok(change(h, slide, ChangeKind::Structure, "groupShapes"))
        }),
    );
    registry.register(
        Command::stub("ungroupShapes", "ungroupShapes").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let group_id = args
                .get("groupId")
                .and_then(|v| v.as_str())
                .or_else(|| arg_id(args))
                .ok_or_else(|| EditError::Other {
                    message: "ungroupShapes missing groupId".into(),
                })?
                .to_owned();
            let child_ids = args
                .get("childIds")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            h.ungroup_shapes(slide, &group_id, &child_ids)
                .map_err(map_err)?;
            Ok(change(h, slide, ChangeKind::Structure, "ungroupShapes"))
        }),
    );
    registry.register(
        Command::stub("setShapeFill", "setShapeFill").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            let mode = args.get("fillMode").and_then(|v| v.as_str());
            h.set_fill(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                arg_nullable_string(args, "fillColor"),
                mode,
            )
            .map_err(map_err)?;
            Ok(change(h, slide, ChangeKind::Format, "setShapeFill"))
        }),
    );
    registry.register(
        Command::stub("setShapeLine", "setShapeLine").with_runner(|host, args| {
            let h = host_mut(host)?;
            let slide = arg_u32(args, "slideIndex")?;
            h.set_line(
                slide,
                arg_id(args),
                arg_u32_opt(args, "elementIndex"),
                arg_nullable_string(args, "strokeColor"),
                arg_f64_opt(args, "strokeWidth"),
            )
            .map_err(map_err)?;
            Ok(change(h, slide, ChangeKind::Format, "setShapeLine"))
        }),
    );
}
