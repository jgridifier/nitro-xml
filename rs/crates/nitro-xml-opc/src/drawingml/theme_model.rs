// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/theme-model.ts; changes listed in PROVENANCE.md.

use serde::{Deserialize, Serialize};

use super::types::{DrawingColor, DrawingFill, DrawingLine};

pub const THEME_COLOR_SLOTS: &[ThemeColorSlot] = &[
    ThemeColorSlot::Dk1,
    ThemeColorSlot::Lt1,
    ThemeColorSlot::Dk2,
    ThemeColorSlot::Lt2,
    ThemeColorSlot::Accent1,
    ThemeColorSlot::Accent2,
    ThemeColorSlot::Accent3,
    ThemeColorSlot::Accent4,
    ThemeColorSlot::Accent5,
    ThemeColorSlot::Accent6,
    ThemeColorSlot::Hlink,
    ThemeColorSlot::FolHlink,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeColorSlot {
    Dk1,
    Lt1,
    Dk2,
    Lt2,
    Accent1,
    Accent2,
    Accent3,
    Accent4,
    Accent5,
    Accent6,
    Hlink,
    FolHlink,
}

impl ThemeColorSlot {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dk1 => "dk1",
            Self::Lt1 => "lt1",
            Self::Dk2 => "dk2",
            Self::Lt2 => "lt2",
            Self::Accent1 => "accent1",
            Self::Accent2 => "accent2",
            Self::Accent3 => "accent3",
            Self::Accent4 => "accent4",
            Self::Accent5 => "accent5",
            Self::Accent6 => "accent6",
            Self::Hlink => "hlink",
            Self::FolHlink => "folHlink",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        THEME_COLOR_SLOTS
            .iter()
            .find(|s| s.as_str() == value)
            .copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeLogicalColor {
    Bg1,
    Tx1,
    Bg2,
    Tx2,
}

pub type ThemeColorMapKey = String;

pub type ThemeColorMap = std::collections::HashMap<String, ThemeColorSlot>;

pub const DEFAULT_THEME_COLOR_MAP: &[(&str, ThemeColorSlot)] = &[
    ("bg1", ThemeColorSlot::Lt1),
    ("tx1", ThemeColorSlot::Dk1),
    ("bg2", ThemeColorSlot::Lt2),
    ("tx2", ThemeColorSlot::Dk2),
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeColorScheme {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub colors: std::collections::HashMap<String, DrawingColor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeFontCollection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub east_asia: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complex_script: Option<String>,
    #[serde(default)]
    pub scripts: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeFontScheme {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub major: ThemeFontCollection,
    pub minor: ThemeFontCollection,
}

/// `a:fmtScheme` style matrix. `fillRef`/`lnRef` idx 1–3 index the fill and
/// line lists; idx 1001–1003 index [`Self::background_fill_styles`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeFormatScheme {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fill_styles: Vec<DrawingFill>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_styles: Vec<DrawingLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub background_fill_styles: Vec<DrawingFill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawingTheme {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub color_scheme: ThemeColorScheme,
    pub font_scheme: ThemeFontScheme,
    /// Paint-time style matrix. Omitted from dumps (`inspect_json` themes).
    #[serde(skip)]
    pub format_scheme: Option<ThemeFormatScheme>,
}
