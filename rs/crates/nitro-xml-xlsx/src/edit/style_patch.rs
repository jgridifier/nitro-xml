// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/styles.ts
// (patchStyle, internStyle, applyStylePatch, StylePatch); changes listed in PROVENANCE.md.
//! Style patching and interning for xlsx cell format edits (#66).

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::model::{
    Alignment, Border, BorderEdge, CellStyle, Color, Fill, Font, GradientStop, Protection,
};
use crate::styles::{style_at, style_key};

/// A partial change to a cell format (ribbon / format dialog / edit scripts).
#[derive(Debug, Clone, Default)]
pub struct StylePatch {
    pub font: Option<Map<String, Value>>,
    pub fill: Option<Fill>,
    pub border: Option<Map<String, Value>>,
    pub num_fmt: Option<String>,
    pub alignment: Option<Map<String, Value>>,
    pub protection: Option<Map<String, Value>>,
    /// When the key is present, `None` detaches the named style.
    pub cell_style_name: Option<Option<String>>,
}

/// Parse a [`StylePatch`] from edit-script / registry JSON.
pub fn style_patch_from_value(v: &Value) -> Result<StylePatch, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "style patch must be an object".to_owned())?;
    let mut patch = StylePatch::default();
    if let Some(font) = obj.get("font") {
        patch.font = Some(
            font.as_object()
                .ok_or_else(|| "font patch must be an object".to_owned())?
                .clone(),
        );
    }
    if let Some(fill) = obj.get("fill") {
        patch.fill = Some(fill_from_json(fill)?);
    }
    if let Some(border) = obj.get("border") {
        patch.border = Some(
            border
                .as_object()
                .ok_or_else(|| "border patch must be an object".to_owned())?
                .clone(),
        );
    }
    if let Some(num_fmt) = obj.get("numFmt") {
        patch.num_fmt = Some(
            num_fmt
                .as_str()
                .ok_or_else(|| "numFmt must be a string".to_owned())?
                .to_owned(),
        );
    }
    if let Some(alignment) = obj.get("alignment") {
        patch.alignment = Some(
            alignment
                .as_object()
                .ok_or_else(|| "alignment patch must be an object".to_owned())?
                .clone(),
        );
    }
    if let Some(protection) = obj.get("protection") {
        patch.protection = Some(
            protection
                .as_object()
                .ok_or_else(|| "protection patch must be an object".to_owned())?
                .clone(),
        );
    }
    if obj.contains_key("cellStyleName") {
        let name = match &obj["cellStyleName"] {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            _ => return Err("cellStyleName must be a string or null".into()),
        };
        patch.cell_style_name = Some(name);
    }
    Ok(patch)
}

fn compact_map(value: Map<String, Value>) -> Map<String, Value> {
    value.into_iter().filter(|(_, v)| !v.is_null()).collect()
}

fn merge_font(base: &Font, patch: &Map<String, Value>) -> Font {
    let mut next = base.clone();
    for (key, entry) in patch {
        if entry.is_null() {
            match key.as_str() {
                "name" => next.name = None,
                "size" => next.size = None,
                "bold" => next.bold = None,
                "italic" => next.italic = None,
                "underline" => next.underline = None,
                "strike" => next.strike = None,
                "color" => next.color = None,
                "vertAlign" => next.vert_align = None,
                "family" => next.family = None,
                "scheme" => next.scheme = None,
                _ => {}
            }
            continue;
        }
        match key.as_str() {
            "name" => next.name = entry.as_str().map(str::to_owned),
            "size" => next.size = entry.as_f64(),
            "bold" => next.bold = entry.as_bool(),
            "italic" => next.italic = entry.as_bool(),
            "underline" => next.underline = entry.as_str().map(str::to_owned),
            "strike" => next.strike = entry.as_bool(),
            "color" => next.color = color_from_json(entry).ok(),
            "vertAlign" => next.vert_align = entry.as_str().map(str::to_owned),
            "family" => next.family = entry.as_f64(),
            "scheme" => next.scheme = entry.as_str().map(str::to_owned),
            _ => {}
        }
    }
    next
}

fn merge_alignment(base: Option<&Alignment>, patch: &Map<String, Value>) -> Option<Alignment> {
    let mut next = base.cloned().unwrap_or(Alignment {
        horizontal: None,
        vertical: None,
        wrap_text: None,
        shrink_to_fit: None,
        indent: None,
        text_rotation: None,
        reading_order: None,
    });
    for (key, entry) in patch {
        if entry.is_null() {
            match key.as_str() {
                "horizontal" => next.horizontal = None,
                "vertical" => next.vertical = None,
                "wrapText" => next.wrap_text = None,
                "shrinkToFit" => next.shrink_to_fit = None,
                "indent" => next.indent = None,
                "textRotation" => next.text_rotation = None,
                "readingOrder" => next.reading_order = None,
                _ => {}
            }
            continue;
        }
        match key.as_str() {
            "horizontal" => next.horizontal = entry.as_str().map(str::to_owned),
            "vertical" => next.vertical = entry.as_str().map(str::to_owned),
            "wrapText" => next.wrap_text = entry.as_bool(),
            "shrinkToFit" => next.shrink_to_fit = entry.as_bool(),
            "indent" => next.indent = entry.as_f64(),
            "textRotation" => next.text_rotation = entry.as_f64(),
            "readingOrder" => next.reading_order = entry.as_f64(),
            _ => {}
        }
    }
    let empty = next.horizontal.is_none()
        && next.vertical.is_none()
        && next.wrap_text.is_none()
        && next.shrink_to_fit.is_none()
        && next.indent.is_none()
        && next.text_rotation.is_none()
        && next.reading_order.is_none();
    if empty {
        None
    } else {
        Some(next)
    }
}

fn merge_protection(base: Option<&Protection>, patch: &Map<String, Value>) -> Option<Protection> {
    let mut next = base.cloned().unwrap_or(Protection {
        locked: None,
        hidden: None,
    });
    for (key, entry) in patch {
        if entry.is_null() {
            match key.as_str() {
                "locked" => next.locked = None,
                "hidden" => next.hidden = None,
                _ => {}
            }
            continue;
        }
        match key.as_str() {
            "locked" => next.locked = entry.as_bool(),
            "hidden" => next.hidden = entry.as_bool(),
            _ => {}
        }
    }
    if next.locked.is_none() && next.hidden.is_none() {
        None
    } else {
        Some(next)
    }
}

fn merge_border(base: &Border, patch: &Map<String, Value>) -> Border {
    let mut next = base.clone();
    for (key, entry) in patch {
        if entry.is_null() {
            match key.as_str() {
                "left" => next.left = None,
                "right" => next.right = None,
                "top" => next.top = None,
                "bottom" => next.bottom = None,
                "diagonal" => next.diagonal = None,
                "diagonalUp" => next.diagonal_up = None,
                "diagonalDown" => next.diagonal_down = None,
                _ => {}
            }
            continue;
        }
        match key.as_str() {
            "left" | "right" | "top" | "bottom" | "diagonal" => {
                if let Ok(edge) = border_edge_from_json(entry) {
                    match key.as_str() {
                        "left" => next.left = Some(edge),
                        "right" => next.right = Some(edge),
                        "top" => next.top = Some(edge),
                        "bottom" => next.bottom = Some(edge),
                        "diagonal" => next.diagonal = Some(edge),
                        _ => {}
                    }
                }
            }
            "diagonalUp" => next.diagonal_up = entry.as_bool(),
            "diagonalDown" => next.diagonal_down = entry.as_bool(),
            _ => {}
        }
    }
    next
}

/// A new style: `base` with `patch` applied. A key patched to JSON `null` is cleared.
pub fn patch_style(base: &CellStyle, patch: &StylePatch) -> CellStyle {
    let font = match &patch.font {
        Some(p) => merge_font(&base.font, &compact_map(p.clone())),
        None => base.font.clone(),
    };
    let fill = patch.fill.clone().unwrap_or_else(|| base.fill.clone());
    let border = match &patch.border {
        Some(p) => merge_border(&base.border, &compact_map(p.clone())),
        None => base.border.clone(),
    };
    let num_fmt = patch
        .num_fmt
        .clone()
        .unwrap_or_else(|| base.num_fmt.clone());
    let alignment = match &patch.alignment {
        Some(p) => merge_alignment(base.alignment.as_ref(), &compact_map(p.clone())),
        None => base.alignment.clone(),
    };
    let protection = match &patch.protection {
        Some(p) => merge_protection(base.protection.as_ref(), &compact_map(p.clone())),
        None => base.protection.clone(),
    };
    let cell_style_name = match &patch.cell_style_name {
        Some(name) => name.clone(),
        None => base.cell_style_name.clone(),
    };
    CellStyle {
        font,
        fill,
        border,
        num_fmt,
        alignment,
        protection,
        cell_style_name,
        quote_prefix: base.quote_prefix,
        pivot_button: base.pivot_button,
    }
}

/// Intern `style` into `styles`, returning its id (deduped by [`style_key`]).
pub fn intern_style(
    styles: &mut Vec<CellStyle>,
    cache: &mut HashMap<String, usize>,
    style: CellStyle,
) -> u32 {
    let key = style_key(&style);
    if let Some(&known) = cache.get(&key) {
        if styles.get(known).is_some_and(|c| style_key(c) == key) {
            return known as u32;
        }
    }
    if let Some(index) = styles.iter().position(|c| style_key(c) == key) {
        cache.insert(key, index);
        return index as u32;
    }
    styles.push(style);
    let index = styles.len() - 1;
    cache.insert(key, index);
    index as u32
}

/// Applies a patch to the style at `style_id` and returns the id of the result.
pub fn apply_style_patch(
    styles: &mut Vec<CellStyle>,
    cache: &mut HashMap<String, usize>,
    style_id: Option<u32>,
    patch: &StylePatch,
) -> u32 {
    let base = style_at(styles, style_id).clone();
    intern_style(styles, cache, patch_style(&base, patch))
}

pub fn color_from_json(v: &Value) -> Result<Color, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "color must be an object".to_owned())?;
    Ok(Color {
        rgb: obj.get("rgb").and_then(|x| x.as_str()).map(str::to_owned),
        theme: obj.get("theme").and_then(|x| x.as_f64()),
        tint: obj.get("tint").and_then(|x| x.as_f64()),
        indexed: obj.get("indexed").and_then(|x| x.as_f64()),
        auto: obj.get("auto").and_then(|x| x.as_bool()),
    })
}

fn border_edge_from_json(v: &Value) -> Result<BorderEdge, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "border edge must be an object".to_owned())?;
    let style = obj
        .get("style")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "border edge missing style".to_owned())?
        .to_owned();
    let color = match obj.get("color") {
        Some(c) => Some(color_from_json(c)?),
        None => None,
    };
    Ok(BorderEdge { style, color })
}

pub fn fill_from_json(v: &Value) -> Result<Fill, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "fill must be an object".to_owned())?;
    let ty = obj
        .get("type")
        .and_then(|x| x.as_str())
        .unwrap_or("pattern");
    match ty {
        "gradient" => {
            let gradient = obj
                .get("gradient")
                .and_then(|x| x.as_str())
                .unwrap_or("linear")
                .to_owned();
            let degree = obj.get("degree").and_then(|x| x.as_f64());
            let stops = obj
                .get("stops")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|s| {
                    let so = s
                        .as_object()
                        .ok_or_else(|| "gradient stop must be an object".to_owned())?;
                    let position = so
                        .get("position")
                        .and_then(|x| x.as_f64())
                        .ok_or_else(|| "gradient stop missing position".to_owned())?;
                    let color = color_from_json(
                        so.get("color")
                            .ok_or_else(|| "gradient stop missing color".to_owned())?,
                    )?;
                    Ok(GradientStop { position, color })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(Fill::Gradient {
                gradient,
                degree,
                stops,
            })
        }
        _ => {
            let pattern = obj
                .get("pattern")
                .and_then(|x| x.as_str())
                .unwrap_or("none")
                .to_owned();
            let fg_color = match obj.get("fgColor") {
                Some(c) => Some(color_from_json(c)?),
                None => None,
            };
            let bg_color = match obj.get("bgColor") {
                Some(c) => Some(color_from_json(c)?),
                None => None,
            };
            Ok(Fill::Pattern {
                pattern,
                fg_color,
                bg_color,
            })
        }
    }
}

/// Rebuild a [`CellStyle`] from its [`CellStyle::to_json`] shape (snapshot restore).
pub fn cell_style_from_json(v: &Value) -> Result<CellStyle, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "cell style must be an object".to_owned())?;
    let font = font_from_json(obj.get("font").unwrap_or(&Value::Object(Map::new())))?;
    let fill = fill_from_json(obj.get("fill").unwrap_or(&Value::Object(Map::new())))?;
    let border = border_from_json(obj.get("border").unwrap_or(&Value::Object(Map::new())))?;
    let num_fmt = obj
        .get("numFmt")
        .and_then(|x| x.as_str())
        .unwrap_or("General")
        .to_owned();
    let alignment = match obj.get("alignment") {
        Some(a) => Some(alignment_from_json(a)?),
        None => None,
    };
    let protection = match obj.get("protection") {
        Some(p) => Some(protection_from_json(p)?),
        None => None,
    };
    Ok(CellStyle {
        font,
        fill,
        border,
        num_fmt,
        alignment,
        protection,
        cell_style_name: obj
            .get("cellStyleName")
            .and_then(|x| x.as_str())
            .map(str::to_owned),
        quote_prefix: obj.get("quotePrefix").and_then(|x| x.as_bool()),
        pivot_button: obj.get("pivotButton").and_then(|x| x.as_bool()),
    })
}

fn font_from_json(v: &Value) -> Result<Font, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "font must be an object".to_owned())?;
    Ok(Font {
        name: obj.get("name").and_then(|x| x.as_str()).map(str::to_owned),
        size: obj.get("size").and_then(|x| x.as_f64()),
        bold: obj.get("bold").and_then(|x| x.as_bool()),
        italic: obj.get("italic").and_then(|x| x.as_bool()),
        underline: obj
            .get("underline")
            .and_then(|x| x.as_str())
            .map(str::to_owned),
        strike: obj.get("strike").and_then(|x| x.as_bool()),
        color: match obj.get("color") {
            Some(c) => Some(color_from_json(c)?),
            None => None,
        },
        vert_align: obj
            .get("vertAlign")
            .and_then(|x| x.as_str())
            .map(str::to_owned),
        family: obj.get("family").and_then(|x| x.as_f64()),
        scheme: obj
            .get("scheme")
            .and_then(|x| x.as_str())
            .map(str::to_owned),
    })
}

fn border_from_json(v: &Value) -> Result<Border, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "border must be an object".to_owned())?;
    Ok(Border {
        left: match obj.get("left") {
            Some(e) => Some(border_edge_from_json(e)?),
            None => None,
        },
        right: match obj.get("right") {
            Some(e) => Some(border_edge_from_json(e)?),
            None => None,
        },
        top: match obj.get("top") {
            Some(e) => Some(border_edge_from_json(e)?),
            None => None,
        },
        bottom: match obj.get("bottom") {
            Some(e) => Some(border_edge_from_json(e)?),
            None => None,
        },
        diagonal: match obj.get("diagonal") {
            Some(e) => Some(border_edge_from_json(e)?),
            None => None,
        },
        diagonal_up: obj.get("diagonalUp").and_then(|x| x.as_bool()),
        diagonal_down: obj.get("diagonalDown").and_then(|x| x.as_bool()),
    })
}

fn alignment_from_json(v: &Value) -> Result<Alignment, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "alignment must be an object".to_owned())?;
    Ok(Alignment {
        horizontal: obj
            .get("horizontal")
            .and_then(|x| x.as_str())
            .map(str::to_owned),
        vertical: obj
            .get("vertical")
            .and_then(|x| x.as_str())
            .map(str::to_owned),
        wrap_text: obj.get("wrapText").and_then(|x| x.as_bool()),
        shrink_to_fit: obj.get("shrinkToFit").and_then(|x| x.as_bool()),
        indent: obj.get("indent").and_then(|x| x.as_f64()),
        text_rotation: obj.get("textRotation").and_then(|x| x.as_f64()),
        reading_order: obj.get("readingOrder").and_then(|x| x.as_f64()),
    })
}

fn protection_from_json(v: &Value) -> Result<Protection, String> {
    let obj = v
        .as_object()
        .ok_or_else(|| "protection must be an object".to_owned())?;
    Ok(Protection {
        locked: obj.get("locked").and_then(|x| x.as_bool()),
        hidden: obj.get("hidden").and_then(|x| x.as_bool()),
    })
}
