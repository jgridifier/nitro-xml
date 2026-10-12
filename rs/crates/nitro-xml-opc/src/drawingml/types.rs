// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/types.ts; changes listed in PROVENANCE.md.

use serde::{Deserialize, Serialize};

/// Reads one attribute by local name; `None` when absent.
pub type AttributeReader = fn(&str) -> Option<String>;

/// A theme-independent DrawingML colour as written in a part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingColor {
    pub kind: DrawingColorKind,
    /// Hex for `srgb`, the scheme/preset/system name, or serialized channels for `scrgb`/`hsl`.
    pub value: String,
    #[serde(default)]
    pub transforms: Vec<ColorTransform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DrawingColorKind {
    Srgb,
    Scheme,
    System,
    Preset,
    Scrgb,
    Hsl,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorTransform {
    pub name: String,
    pub value: String,
}

/// A DrawingML fill (`a:noFill`, `a:solidFill`, …).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DrawingFill {
    None,
    Solid {
        color: DrawingColor,
    },
    Gradient {
        #[serde(skip_serializing_if = "Option::is_none", rename = "sourceXml")]
        source_xml: Option<String>,
        stops: Vec<GradientStop>,
        #[serde(skip_serializing_if = "Option::is_none")]
        angle: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        scaled: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "fillToRect")]
        fill_to_rect: Option<RectFractions>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "tileRect")]
        tile_rect: Option<RectFractions>,
    },
    Pattern {
        preset: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        foreground: Option<DrawingColor>,
        #[serde(skip_serializing_if = "Option::is_none")]
        background: Option<DrawingColor>,
    },
    Picture {
        #[serde(skip_serializing_if = "Option::is_none", rename = "relId")]
        rel_id: Option<String>,
        #[serde(skip_serializing)]
        opacity: Option<f64>,
    },
    Unsupported {
        element: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub position: f64,
    pub color: DrawingColor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RectFractions {
    pub l: f64,
    pub t: f64,
    pub r: f64,
    pub b: f64,
}

/// A DrawingML outline (`a:ln`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingLine {
    #[serde(skip_serializing_if = "Option::is_none", rename = "widthEmu")]
    pub width_emu: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<DrawingFill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cap: Option<String>,
}

/// Run formatting without `text` (paragraph `defRPr` / `defaultProperties`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DrawingTextRunProperties {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_pt: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<DrawingColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typeface: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawingTextRun {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_pt: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<DrawingColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typeface: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawingTextSpacing {
    pub unit: String,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawingTextParagraph {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_properties: Option<DrawingTextRunProperties>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<DrawingTextSpacing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_before: Option<DrawingTextSpacing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_after: Option<DrawingTextSpacing>,
    pub runs: Vec<DrawingTextRun>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawingTextBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insets_emu: Option<FrameInsets>,
    pub paragraphs: Vec<DrawingTextParagraph>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameInsets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingFrame {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "UPPERCASE")]
pub enum DrawingPathCommand {
    M {
        x: f64,
        y: f64,
    },
    L {
        x: f64,
        y: f64,
    },
    C {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x: f64,
        y: f64,
    },
    Q {
        x1: f64,
        y1: f64,
        x: f64,
        y: f64,
    },
    A {
        w_r: f64,
        h_r: f64,
        start_angle: f64,
        swing_angle: f64,
    },
    Z,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawingPath {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<bool>,
    pub commands: Vec<DrawingPathCommand>,
}
