// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/adapt-paragraph-box.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;

use serde_json::Value;

use super::adapt_table::css_hex;
use super::input::{LayoutBlock, LayoutParagraph, LayoutParagraphBorder, LayoutParagraphBorders};

fn points_to_px(points: f64) -> f64 {
    points * 96.0 / 72.0
}

pub struct ParagraphBoxResult {
    pub borders: Option<LayoutParagraphBorders>,
    pub shading: Option<String>,
    pub between_border: Option<LayoutParagraphBorder>,
}

pub fn paragraph_box(formatting: &Value, theme: Option<&Value>) -> ParagraphBoxResult {
    let borders_obj = formatting.get("borders");
    let mut borders = LayoutParagraphBorders {
        top: None,
        bottom: None,
        left: None,
        right: None,
    };
    let mut between_border = None;
    if let Some(b) = borders_obj {
        for side in ["top", "bottom", "left", "right", "between"] {
            let source = b.get(side);
            let line = border_side(source, theme);
            if line.is_none() {
                continue;
            }
            let line = line.unwrap();
            let space = source
                .and_then(|s| s.get("spacePoints"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let resolved = LayoutParagraphBorder {
                width_px: line.width_px,
                style: line.style,
                color: line.color,
                space_px: points_to_px(space),
            };
            match side {
                "between" => between_border = Some(resolved),
                "top" => borders.top = Some(resolved),
                "bottom" => borders.bottom = Some(resolved),
                "left" => borders.left = Some(resolved),
                "right" => borders.right = Some(resolved),
                _ => {}
            }
        }
    }
    let shading = css_hex(formatting.get("shadingFill").and_then(|v| v.as_str()));
    ParagraphBoxResult {
        borders: if borders.top.is_some()
            || borders.bottom.is_some()
            || borders.left.is_some()
            || borders.right.is_some()
        {
            Some(borders)
        } else {
            None
        },
        shading,
        between_border,
    }
}

fn border_side(side: Option<&Value>, theme: Option<&Value>) -> Option<LayoutParagraphBorder> {
    let side = side?;
    let style = side.get("style").and_then(|v| v.as_str())?;
    if style == "none" || style == "nil" {
        return None;
    }
    let resolved_style = if style.starts_with("double") {
        "double"
    } else if style.to_ascii_lowercase().contains("dot") {
        "dotted"
    } else if style.to_ascii_lowercase().contains("dash") {
        "dashed"
    } else {
        "solid"
    };
    let width_px = ((side.get("sizeEighthPoints").and_then(|v| v.as_f64()).unwrap_or(4.0) / 8.0)
        * 96.0
        / 72.0)
        .max(if resolved_style == "double" { 3.0 } else { 1.0 });
    let color = side
        .get("color")
        .and_then(|v| v.as_str())
        .and_then(|c| css_hex(Some(c)))
        .or_else(|| {
            side.get("themeColor").and_then(|tc| {
                theme.and_then(|t| crate::theme_color::resolve_theme_color_reference(tc, t))
            })
        })
        .unwrap_or_else(|| "#000000".into());
    Some(LayoutParagraphBorder {
        width_px,
        style: resolved_style.into(),
        color,
        space_px: 0.0,
    })
}

pub fn group_paragraph_borders(
    blocks: &mut [LayoutBlock],
    between: &HashMap<String, LayoutParagraphBorder>,
) {
    let keys: Vec<Option<String>> = blocks
        .iter()
        .map(|block| match block {
            LayoutBlock::Paragraph(p) => p
                .borders
                .as_ref()
                .map(|b| serde_json::to_string(b).unwrap_or_default()),
            _ => None,
        })
        .collect();
    for (index, block) in blocks.iter_mut().enumerate() {
        if let LayoutBlock::Paragraph(paragraph) = block {
            if paragraph.borders.is_none() {
                continue;
            }
            let same_prev = index > 0 && keys[index - 1] == keys[index];
            let same_next = index + 1 < keys.len() && keys[index + 1] == keys[index];
            if !same_prev && !same_next {
                continue;
            }
            let mut borders = paragraph.borders.clone().unwrap();
            if same_next {
                borders.bottom = None;
            }
            if same_prev {
                if let Some(line) = between.get(&paragraph.id) {
                    borders.top = Some(line.clone());
                } else {
                    borders.top = None;
                }
            }
            paragraph.borders = Some(borders);
        }
    }
}
