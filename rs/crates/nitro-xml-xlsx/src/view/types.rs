// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/types.ts; changes listed in PROVENANCE.md.

use std::rc::Rc;

use serde_json::{json, Map, Value};

use crate::model::js_num;
use crate::view::sparkline_view::SparklineView;

#[derive(Debug, Clone, PartialEq)]
pub struct FontView {
    pub family: String,
    pub size_px: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: Option<String>,
    pub strike: bool,
    pub color: String,
    pub vert_align: Option<String>,
}

impl FontView {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("family".into(), Value::String(self.family.clone()));
        m.insert("sizePx".into(), js_num(self.size_px));
        m.insert("bold".into(), Value::Bool(self.bold));
        m.insert("italic".into(), Value::Bool(self.italic));
        m.insert("strike".into(), Value::Bool(self.strike));
        m.insert("color".into(), Value::String(self.color.clone()));
        if let Some(u) = &self.underline {
            m.insert("underline".into(), Value::String(u.clone()));
        }
        if let Some(v) = &self.vert_align {
            m.insert("vertAlign".into(), Value::String(v.clone()));
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeView {
    pub width_px: f64,
    pub style: String,
    pub color: String,
}

impl EdgeView {
    pub fn to_json(&self) -> Value {
        json!({
            "widthPx": js_num(self.width_px),
            "style": self.style,
            "color": self.color,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FillView {
    Background {
        background: String,
    },
    Gradient {
        gradient: String,
    },
    Pattern {
        pattern: String,
        fg: String,
        bg: String,
    },
}

impl FillView {
    pub fn to_json(&self) -> Value {
        match self {
            FillView::Background { background } => json!({ "background": background }),
            FillView::Gradient { gradient } => json!({ "gradient": gradient }),
            FillView::Pattern { pattern, fg, bg } => json!({
                "pattern": pattern,
                "fg": fg,
                "bg": bg,
            }),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BordersView {
    pub top: Option<EdgeView>,
    pub right: Option<EdgeView>,
    pub bottom: Option<EdgeView>,
    pub left: Option<EdgeView>,
    pub diagonal_up: Option<EdgeView>,
    pub diagonal_down: Option<EdgeView>,
}

impl BordersView {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(e) = &self.top {
            m.insert("top".into(), e.to_json());
        }
        if let Some(e) = &self.right {
            m.insert("right".into(), e.to_json());
        }
        if let Some(e) = &self.bottom {
            m.insert("bottom".into(), e.to_json());
        }
        if let Some(e) = &self.left {
            m.insert("left".into(), e.to_json());
        }
        if let Some(e) = &self.diagonal_up {
            m.insert("diagonalUp".into(), e.to_json());
        }
        if let Some(e) = &self.diagonal_down {
            m.insert("diagonalDown".into(), e.to_json());
        }
        Value::Object(m)
    }

    pub fn has_borders(&self) -> bool {
        self.top.is_some()
            || self.right.is_some()
            || self.bottom.is_some()
            || self.left.is_some()
            || self.diagonal_up.is_some()
            || self.diagonal_down.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataBarView {
    pub fraction: f64,
    pub start: Option<f64>,
    pub axis: Option<(f64, String)>,
    pub color: String,
    pub negative: Option<bool>,
    pub gradient: Option<bool>,
    pub direction: Option<String>,
    pub border_color: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IconView {
    pub set: String,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichRun {
    pub text: String,
    pub font: FontView,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CellView {
    pub text: String,
    pub title: Option<String>,
    /// Shared across same-style cells via StylePaintCache (#30).
    pub font: Rc<FontView>,
    pub fill: Option<FillView>,
    pub borders: BordersView,
    pub h_align: String,
    pub v_align: String,
    pub wrap: bool,
    pub shrink: bool,
    pub indent_px: f64,
    pub rotation: f64,
    pub vertical_text: bool,
    pub is_number: bool,
    pub overflow: bool,
    pub data_bar: Option<DataBarView>,
    pub icon: Option<IconView>,
    pub has_comment: bool,
    pub has_hyperlink: bool,
    pub validation_list: bool,
    pub is_error: bool,
    pub rich: Option<Vec<RichRun>>,
    pub sparkline: Option<SparklineView>,
}

impl CellView {
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("text".into(), Value::String(self.text.clone()));
        if let Some(t) = &self.title {
            m.insert("title".into(), Value::String(t.clone()));
        }
        m.insert("font".into(), self.font.to_json());
        if let Some(f) = &self.fill {
            m.insert("fill".into(), f.to_json());
        }
        m.insert("borders".into(), self.borders.to_json());
        m.insert("hAlign".into(), Value::String(self.h_align.clone()));
        m.insert("vAlign".into(), Value::String(self.v_align.clone()));
        m.insert("wrap".into(), Value::Bool(self.wrap));
        m.insert("shrink".into(), Value::Bool(self.shrink));
        m.insert("indentPx".into(), js_num(self.indent_px));
        m.insert("rotation".into(), js_num(self.rotation));
        if self.vertical_text {
            m.insert("verticalText".into(), Value::Bool(true));
        }
        m.insert("isNumber".into(), Value::Bool(self.is_number));
        m.insert("overflow".into(), Value::Bool(self.overflow));
        if let Some(db) = &self.data_bar {
            let mut dm = Map::new();
            dm.insert("fraction".into(), js_num(db.fraction));
            if let Some(s) = db.start {
                dm.insert("start".into(), js_num(s));
            }
            if let Some((f, c)) = &db.axis {
                dm.insert("axis".into(), json!({ "fraction": js_num(*f), "color": c }));
            }
            dm.insert("color".into(), Value::String(db.color.clone()));
            if let Some(n) = db.negative {
                dm.insert("negative".into(), Value::Bool(n));
            }
            if let Some(g) = db.gradient {
                dm.insert("gradient".into(), Value::Bool(g));
            }
            if let Some(d) = &db.direction {
                dm.insert("direction".into(), Value::String(d.clone()));
            }
            if let Some(bc) = &db.border_color {
                dm.insert("borderColor".into(), Value::String(bc.clone()));
            }
            m.insert("dataBar".into(), Value::Object(dm));
        }
        if let Some(icon) = &self.icon {
            m.insert(
                "icon".into(),
                json!({ "set": icon.set, "index": icon.index }),
            );
        }
        m.insert("hasComment".into(), Value::Bool(self.has_comment));
        m.insert("hasHyperlink".into(), Value::Bool(self.has_hyperlink));
        m.insert("validationList".into(), Value::Bool(self.validation_list));
        m.insert("isError".into(), Value::Bool(self.is_error));
        if let Some(rich) = &self.rich {
            m.insert(
                "rich".into(),
                Value::Array(
                    rich.iter()
                        .map(|r| {
                            json!({
                                "text": r.text,
                                "font": r.font.to_json(),
                            })
                        })
                        .collect(),
                ),
            );
        }
        if let Some(sp) = &self.sparkline {
            m.insert("sparkline".into(), sp.to_json());
        }
        Value::Object(m)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CellItem {
    pub key: String,
    pub row: u32,
    pub col: u32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub view: CellView,
    pub text: String,
    pub text_left: i32,
    pub text_width: i32,
    pub overflowing: bool,
    pub font_scale: f64,
    pub merged: bool,
    pub error_mark: bool,
}

impl CellItem {
    pub fn to_json(&self) -> Value {
        json!({
            "key": self.key,
            "row": self.row,
            "col": self.col,
            "x": self.x,
            "y": self.y,
            "w": self.w,
            "h": self.h,
            "view": self.view.to_json(),
            "text": self.text,
            "textLeft": self.text_left,
            "textWidth": self.text_width,
            "overflowing": self.overflowing,
            "fontScale": js_num(self.font_scale),
            "merged": self.merged,
            "errorMark": self.error_mark,
        })
    }
}
