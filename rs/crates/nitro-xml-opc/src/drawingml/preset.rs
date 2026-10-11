// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/geometry/preset-shape-evaluator.ts and
// preset-text-rect-table.ts; changes listed in PROVENANCE.md.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

use super::guide_api::{evaluate_guides, GeometryContext, GeometryGuide};
use super::guide_eval::{evaluate_formula, parse_formula, resolve_operand};
use super::ooxml_arc::ooxml_arc_to_svg;

static PRESET_TABLE: LazyLock<HashMap<String, PresetShapeGeometryDefinition>> =
    LazyLock::new(|| {
        let raw = include_str!("data/preset_shape_geometry.json");
        serde_json::from_str(raw).expect("preset_shape_geometry.json")
    });

static TEXT_RECT_TABLE: LazyLock<HashMap<String, PresetTextRectDefinition>> = LazyLock::new(|| {
    let raw = include_str!("data/preset_text_rect.json");
    serde_json::from_str(raw).expect("preset_text_rect.json")
});

#[derive(Debug, Clone, Deserialize)]
pub struct PresetShapeGeometryDefinition {
    #[serde(default)]
    #[allow(dead_code)]
    pub name: String,
    #[serde(default, rename = "avLst")]
    pub av_lst: HashMap<String, f64>,
    #[serde(default, rename = "gdLst")]
    gd_lst: Vec<JsonGuide>,
    #[serde(default)]
    rect: Option<RectTokens>,
    #[serde(default, rename = "pathLst")]
    path_lst: Vec<PresetPath>,
}

#[derive(Debug, Clone, Deserialize)]
struct JsonGuide {
    name: String,
    formula: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RectTokens {
    l: String,
    t: String,
    r: String,
    b: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PresetPath {
    #[serde(default)]
    w: Option<f64>,
    #[serde(default)]
    h: Option<f64>,
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    stroke: Option<bool>,
    commands: Vec<PathCommand>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum PathCommand {
    MoveTo {
        x: String,
        y: String,
    },
    LnTo {
        x: String,
        y: String,
    },
    ArcTo {
        #[serde(rename = "wR")]
        w_r: String,
        #[serde(rename = "hR")]
        h_r: String,
        #[serde(rename = "stAng")]
        st_ang: String,
        #[serde(rename = "swAng")]
        sw_ang: String,
    },
    QuadBezTo {
        x1: String,
        y1: String,
        x2: String,
        y2: String,
    },
    CubicBezTo {
        x1: String,
        y1: String,
        x2: String,
        y2: String,
        x3: String,
        y3: String,
    },
    Close,
}

#[derive(Debug, Clone, Deserialize)]
struct PresetTextRectDefinition {
    #[serde(default, rename = "avLst")]
    av_lst: HashMap<String, f64>,
    #[serde(default, rename = "gdLst")]
    gd_lst: Vec<JsonGuide>,
    rect: RectTokens,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PresetSubpathResult {
    pub d: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PresetShapeEvaluationResult {
    #[serde(rename = "svgPath")]
    pub svg_path: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "textRect")]
    pub text_rect: Option<TextRect>,
    pub paths: Vec<PresetSubpathResult>,
    #[serde(rename = "fillNone")]
    pub fill_none: bool,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct TextRect {
    pub l: f64,
    pub t: f64,
    pub r: f64,
    pub b: f64,
}

pub fn list_preset_names() -> Vec<String> {
    let mut names: Vec<String> = PRESET_TABLE.keys().cloned().collect();
    names.sort();
    names
}

pub fn lookup_preset_shape(name: &str) -> Option<PresetShapeGeometryDefinition> {
    if name.is_empty() {
        return None;
    }
    if let Some(def) = PRESET_TABLE.get(name) {
        return Some(def.clone());
    }
    let lower = name.to_lowercase();
    for key in PRESET_TABLE.keys() {
        if key.to_lowercase() == lower {
            return PRESET_TABLE.get(key).cloned();
        }
    }
    None
}

fn lookup_text_rect_override(name: &str) -> Option<PresetTextRectDefinition> {
    if name.is_empty() {
        return None;
    }
    if let Some(def) = TEXT_RECT_TABLE.get(name) {
        return Some(def.clone());
    }
    let lower = name.to_lowercase();
    for key in TEXT_RECT_TABLE.keys() {
        if key.to_lowercase() == lower {
            return TEXT_RECT_TABLE.get(key).cloned();
        }
    }
    None
}

fn build_adjustment_map(
    def: &PresetShapeGeometryDefinition,
    adjustments: Option<&HashMap<String, f64>>,
) -> HashMap<String, f64> {
    let mut adj = HashMap::new();
    for (name, value) in &def.av_lst {
        adj.insert(name.clone(), *value);
    }
    if let Some(overrides) = adjustments {
        for (name, value) in overrides {
            if value.is_finite() {
                adj.insert(name.clone(), *value);
            }
        }
    }
    adj
}

fn resolve_token(token: &str, vars: &HashMap<String, f64>) -> f64 {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    if let Ok(num) = trimmed.parse::<f64>() {
        if num.is_finite() {
            return num;
        }
    }
    resolve_operand(trimmed, vars)
}

fn guides_from_json(guides: &[JsonGuide]) -> Vec<GeometryGuide> {
    guides
        .iter()
        .map(|g| GeometryGuide {
            name: g.name.clone(),
            formula: g.formula.clone(),
        })
        .collect()
}

fn get_preset_text_rect(
    prst: &str,
    width: f64,
    height: f64,
    adjustments: Option<&HashMap<String, f64>>,
) -> Option<TextRect> {
    let def = lookup_text_rect_override(prst)?;
    let w = if width.is_finite() && width > 0.0 {
        width
    } else {
        0.0
    };
    let h = if height.is_finite() && height > 0.0 {
        height
    } else {
        0.0
    };
    let mut adj = HashMap::new();
    for (name, value) in &def.av_lst {
        adj.insert(name.clone(), *value);
    }
    if let Some(overrides) = adjustments {
        for (name, value) in overrides {
            if value.is_finite() {
                adj.insert(name.clone(), *value);
            }
        }
    }
    let vars = evaluate_guides(
        &guides_from_json(&def.gd_lst),
        GeometryContext { w, h },
        Some(&adj),
    );
    Some(TextRect {
        l: resolve_rect_token(&def.rect.l, &vars),
        t: resolve_rect_token(&def.rect.t, &vars),
        r: resolve_rect_token(&def.rect.r, &vars),
        b: resolve_rect_token(&def.rect.b, &vars),
    })
}

fn resolve_rect_token(token: &str, vars: &HashMap<String, f64>) -> f64 {
    if let Ok(num) = token.parse::<f64>() {
        if num.is_finite() {
            return num;
        }
    }
    resolve_operand(token, vars)
}

pub fn evaluate_preset_shape(
    name: &str,
    width: f64,
    height: f64,
    adjustments: Option<&HashMap<String, f64>>,
) -> Option<PresetShapeEvaluationResult> {
    let def = lookup_preset_shape(name)?;
    let w = if width.is_finite() && width > 0.0 {
        width
    } else {
        0.0
    };
    let h = if height.is_finite() && height > 0.0 {
        height
    } else {
        0.0
    };
    let adj = build_adjustment_map(&def, adjustments);
    let guides = guides_from_json(&def.gd_lst);
    let vars = evaluate_guides(&guides, GeometryContext { w, h }, Some(&adj));

    let mut paths = Vec::new();
    for path in &def.path_lst {
        let mut parts: Vec<String> = Vec::new();
        let space_w = path.w.filter(|v| *v > 0.0);
        let space_h = path.h.filter(|v| *v > 0.0);
        let in_path_space = space_w.is_some() || space_h.is_some();
        let scale_x = space_w.map(|sw| w / sw).unwrap_or(1.0);
        let scale_y = space_h.map(|sh| h / sh).unwrap_or(1.0);
        let sx = |value: f64| {
            if in_path_space {
                value * scale_x
            } else {
                value
            }
        };
        let sy = |value: f64| {
            if in_path_space {
                value * scale_y
            } else {
                value
            }
        };

        let mut pen_x = 0.0;
        let mut pen_y = 0.0;
        let mut move_x = 0.0;
        let mut move_y = 0.0;

        for cmd in &path.commands {
            match cmd {
                PathCommand::MoveTo { x, y } => {
                    let x = sx(resolve_token(x, &vars));
                    let y = sy(resolve_token(y, &vars));
                    parts.push(format!("M {x} {y}"));
                    pen_x = x;
                    pen_y = y;
                    move_x = x;
                    move_y = y;
                }
                PathCommand::LnTo { x, y } => {
                    let x = sx(resolve_token(x, &vars));
                    let y = sy(resolve_token(y, &vars));
                    parts.push(format!("L {x} {y}"));
                    pen_x = x;
                    pen_y = y;
                }
                PathCommand::QuadBezTo { x1, y1, x2, y2 } => {
                    let x1 = sx(resolve_token(x1, &vars));
                    let y1 = sy(resolve_token(y1, &vars));
                    let x2 = sx(resolve_token(x2, &vars));
                    let y2 = sy(resolve_token(y2, &vars));
                    parts.push(format!("Q {x1} {y1} {x2} {y2}"));
                    pen_x = x2;
                    pen_y = y2;
                }
                PathCommand::CubicBezTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x3,
                    y3,
                } => {
                    let x1 = sx(resolve_token(x1, &vars));
                    let y1 = sy(resolve_token(y1, &vars));
                    let x2 = sx(resolve_token(x2, &vars));
                    let y2 = sy(resolve_token(y2, &vars));
                    let x3 = sx(resolve_token(x3, &vars));
                    let y3 = sy(resolve_token(y3, &vars));
                    parts.push(format!("C {x1} {y1} {x2} {y2} {x3} {y3}"));
                    pen_x = x3;
                    pen_y = y3;
                }
                PathCommand::ArcTo {
                    w_r,
                    h_r,
                    st_ang,
                    sw_ang,
                } => {
                    let w_r = sx(resolve_token(w_r, &vars));
                    let h_r = sy(resolve_token(h_r, &vars));
                    let st_ang = resolve_token(st_ang, &vars);
                    let sw_ang = resolve_token(sw_ang, &vars);
                    if let Some(result) = ooxml_arc_to_svg(w_r, h_r, st_ang, sw_ang, pen_x, pen_y) {
                        parts.push(result.svg);
                        pen_x = result.end_x;
                        pen_y = result.end_y;
                    }
                }
                PathCommand::Close => {
                    parts.push("Z".into());
                    pen_x = move_x;
                    pen_y = move_y;
                }
            }
        }
        paths.push(PresetSubpathResult {
            d: parts.join(" ").trim().to_string(),
            fill: path.fill.clone(),
            stroke: path.stroke,
        });
    }

    let svg_path = paths
        .iter()
        .map(|p| p.d.as_str())
        .filter(|d| !d.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    let fill_none = !paths.is_empty() && paths.iter().all(|p| p.fill.as_deref() == Some("none"));

    let mut text_rect = get_preset_text_rect(name, w, h, adjustments);
    if text_rect.is_none() {
        if let Some(rect) = &def.rect {
            let resolve_rect_token_formula = |token: &str| -> f64 {
                if token.contains(' ') {
                    evaluate_formula(&parse_formula(token), &vars)
                } else {
                    resolve_token(token, &vars)
                }
            };
            text_rect = Some(TextRect {
                l: resolve_rect_token_formula(&rect.l),
                t: resolve_rect_token_formula(&rect.t),
                r: resolve_rect_token_formula(&rect.r),
                b: resolve_rect_token_formula(&rect.b),
            });
        }
    }

    Some(PresetShapeEvaluationResult {
        svg_path,
        text_rect,
        paths,
        fill_none,
    })
}
