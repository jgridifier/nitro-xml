// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/read/styles.ts and styleKey from
// src/core/xlsx/styles.ts; changes listed in PROVENANCE.md.
//! Styles part: fonts/fills/borders, xf map, named styles, dxfs, palette.

use std::collections::HashMap;

use nitro_xml_opc::xml::{parse_xml_str, Element};
use serde_json::{Map, Value};

use crate::builtin_formats::builtin_format;
use crate::model::{CellStyle, DifferentialStyle, Fill, Font, NamedStyle};
use crate::style_parts::{
    parse_alignment, parse_border, parse_fill, parse_font, parse_indexed_palette, parse_protection,
};
use crate::workbook::default_cell_style;
use crate::xml_util::{att, bool_attr_or, num_attr, x_children, x_first};

/// Parsed `styles.xml` for the Stage-3 workbook slice.
#[derive(Debug, Clone)]
pub struct ParsedStyles {
    pub styles: Vec<CellStyle>,
    pub xf_map: Vec<u32>,
    pub named_styles: Vec<NamedStyle>,
    pub dxfs: Vec<DifferentialStyle>,
    pub palette: Option<Vec<String>>,
}

/// A stable key for a style so equal formats share one entry (matches upstream `styleKey`).
pub fn style_key(style: &CellStyle) -> String {
    sort_keys(&style.to_json()).to_string()
}

fn sort_keys(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(sort_keys).collect()),
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for key in keys {
                let entry = &map[key];
                if !entry.is_null() {
                    out.insert(key.clone(), sort_keys(entry));
                }
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Reads a `dxf` into a differential style.
pub fn parse_dxf(
    dxf: &Element,
    num_fmts: &HashMap<u32, String>,
    palette: Option<&[String]>,
) -> DifferentialStyle {
    let mut style = DifferentialStyle {
        font: None,
        fill: None,
        border: None,
        num_fmt: None,
    };
    if let Some(font) = x_first(Some(dxf), "font") {
        style.font = Some(parse_font(Some(font), palette, true));
    }
    if let Some(fill) = x_first(Some(dxf), "fill") {
        style.fill = Some(parse_fill(Some(fill), palette, true));
    }
    if let Some(border) = x_first(Some(dxf), "border") {
        style.border = Some(parse_border(Some(border), palette));
    }
    if let Some(num_fmt) = x_first(Some(dxf), "numFmt") {
        let code = att(Some(num_fmt), "formatCode")
            .map(str::to_owned)
            .or_else(|| {
                num_attr(Some(num_fmt), "numFmtId").and_then(|id| {
                    if id < 0.0 {
                        None
                    } else {
                        num_fmts.get(&(id as u32)).cloned()
                    }
                })
            });
        if let Some(code) = code {
            style.num_fmt = Some(code);
        }
    }
    style
}

/// Reads `styles.xml`; an absent part gives the default style table.
pub fn parse_styles(xml: Option<&str>, warnings: &mut Vec<String>) -> ParsedStyles {
    let fallback = default_cell_style();
    let Some(xml) = xml else {
        return ParsedStyles {
            styles: vec![fallback.clone()],
            xf_map: vec![0],
            named_styles: vec![NamedStyle {
                name: "Normal".into(),
                style: default_cell_style(),
                builtin_id: Some(0.0),
            }],
            dxfs: vec![],
            palette: None,
        };
    };
    let root = match parse_xml_str(xml) {
        Ok(doc) => doc.root,
        Err(_) => {
            return ParsedStyles {
                styles: vec![fallback.clone()],
                xf_map: vec![0],
                named_styles: vec![NamedStyle {
                    name: "Normal".into(),
                    style: default_cell_style(),
                    builtin_id: Some(0.0),
                }],
                dxfs: vec![],
                palette: None,
            };
        }
    };
    let palette = parse_indexed_palette(&root);
    let palette_ref = palette.as_deref();
    let mut num_fmts: HashMap<u32, String> = HashMap::new();
    let num_fmts_parent = x_first(Some(&root), "numFmts").unwrap_or(&root);
    for node in x_children(num_fmts_parent, "numFmt") {
        if let (Some(id), Some(code)) = (
            num_attr(Some(node), "numFmtId"),
            att(Some(node), "formatCode"),
        ) {
            if id >= 0.0 {
                num_fmts.insert(id as u32, code.to_owned());
            }
        }
    }
    let format_of = |id: Option<f64>, warnings: &mut Vec<String>| -> String {
        let Some(id) = id else {
            return "General".into();
        };
        if id < 0.0 {
            return "General".into();
        }
        let id_u = id as u32;
        if let Some(code) = num_fmts.get(&id_u) {
            return code.clone();
        }
        if let Some(code) = builtin_format(id_u) {
            return code.to_owned();
        }
        warnings.push(format!("Unknown number format id {id_u}; using General."));
        "General".into()
    };

    let fonts_parent = x_first(Some(&root), "fonts").unwrap_or(&root);
    let fonts: Vec<Font> = x_children(fonts_parent, "font")
        .into_iter()
        .map(|node| parse_font(Some(node), palette_ref, false))
        .collect();
    let fills_parent = x_first(Some(&root), "fills").unwrap_or(&root);
    let fills: Vec<Fill> = x_children(fills_parent, "fill")
        .into_iter()
        .map(|node| parse_fill(Some(node), palette_ref, false))
        .collect();
    let borders_parent = x_first(Some(&root), "borders").unwrap_or(&root);
    let borders: Vec<_> = x_children(borders_parent, "border")
        .into_iter()
        .map(|node| parse_border(Some(node), palette_ref))
        .collect();

    let resolve = |xf: &Element, warnings: &mut Vec<String>| -> CellStyle {
        let font_id = num_attr(Some(xf), "fontId").unwrap_or(0.0) as usize;
        let fill_id = num_attr(Some(xf), "fillId").unwrap_or(0.0) as usize;
        let border_id = num_attr(Some(xf), "borderId").unwrap_or(0.0) as usize;
        let mut style = CellStyle {
            font: fonts
                .get(font_id)
                .cloned()
                .or_else(|| fonts.first().cloned())
                .unwrap_or_else(|| fallback.font.clone()),
            fill: fills.get(fill_id).cloned().unwrap_or(Fill::Pattern {
                pattern: "none".into(),
                fg_color: None,
                bg_color: None,
            }),
            border: borders.get(border_id).cloned().unwrap_or_default(),
            num_fmt: format_of(num_attr(Some(xf), "numFmtId"), warnings),
            alignment: None,
            protection: None,
            cell_style_name: None,
            quote_prefix: None,
            pivot_button: None,
        };
        if let Some(alignment) = parse_alignment(x_first(Some(xf), "alignment")) {
            style.alignment = Some(alignment);
        }
        if let Some(protection) = parse_protection(x_first(Some(xf), "protection")) {
            style.protection = Some(protection);
        }
        if bool_attr_or(Some(xf), "quotePrefix", false) {
            style.quote_prefix = Some(true);
        }
        if bool_attr_or(Some(xf), "pivotButton", false) {
            style.pivot_button = Some(true);
        }
        style
    };

    let style_xfs_parent = x_first(Some(&root), "cellStyleXfs").unwrap_or(&root);
    let style_xfs: Vec<CellStyle> = x_children(style_xfs_parent, "xf")
        .into_iter()
        .map(|xf| resolve(xf, warnings))
        .collect();

    let mut names_by_xf: HashMap<u32, String> = HashMap::new();
    let mut named_styles: Vec<NamedStyle> = Vec::new();
    let cell_styles_parent = x_first(Some(&root), "cellStyles").unwrap_or(&root);
    for node in x_children(cell_styles_parent, "cellStyle") {
        let xf_id = num_attr(Some(node), "xfId").unwrap_or(0.0) as u32;
        let name = att(Some(node), "name")
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Style {xf_id}"));
        let style = style_xfs
            .get(xf_id as usize)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        let mut entry = NamedStyle {
            name: name.clone(),
            style,
            builtin_id: None,
        };
        if let Some(builtin_id) = num_attr(Some(node), "builtinId") {
            entry.builtin_id = Some(builtin_id);
        }
        names_by_xf.entry(xf_id).or_insert(name);
        named_styles.push(entry);
    }
    if let Some(normal_at) = named_styles.iter().position(|e| e.builtin_id == Some(0.0)) {
        if normal_at > 0 {
            let normal = named_styles.remove(normal_at);
            named_styles.insert(0, normal);
        }
    }
    if !named_styles.iter().any(|e| e.name == "Normal") {
        named_styles.insert(
            0,
            NamedStyle {
                name: "Normal".into(),
                style: style_xfs
                    .first()
                    .cloned()
                    .unwrap_or_else(default_cell_style),
                builtin_id: Some(0.0),
            },
        );
    }

    let mut styles: Vec<CellStyle> = Vec::new();
    let mut keys: HashMap<String, u32> = HashMap::new();
    let mut xf_map: Vec<u32> = Vec::new();
    let cell_xfs_parent = x_first(Some(&root), "cellXfs").unwrap_or(&root);
    for xf in x_children(cell_xfs_parent, "xf") {
        let mut style = resolve(xf, warnings);
        let xf_id = num_attr(Some(xf), "xfId").unwrap_or(0.0) as u32;
        if let Some(named) = names_by_xf.get(&xf_id) {
            if xf_id != 0 && named != "Normal" {
                style.cell_style_name = Some(named.clone());
            }
        }
        let key = style_key(&style);
        let index = if let Some(&index) = keys.get(&key) {
            index
        } else {
            let index = styles.len() as u32;
            styles.push(style);
            keys.insert(key, index);
            index
        };
        xf_map.push(index);
    }
    if styles.is_empty() {
        styles.push(fallback);
        xf_map.push(0);
    }
    let dxfs_parent = x_first(Some(&root), "dxfs").unwrap_or(&root);
    let dxfs = x_children(dxfs_parent, "dxf")
        .into_iter()
        .map(|node| parse_dxf(node, &num_fmts, palette_ref))
        .collect();
    ParsedStyles {
        styles,
        xf_map,
        named_styles,
        dxfs,
        palette,
    }
}
