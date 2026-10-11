// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/theme-color.ts; changes listed in PROVENANCE.md.

use super::color::DrawingColorTheme;
use super::theme::is_theme_color_slot;
use super::theme_model::{
    ThemeColorMap, ThemeColorScheme, ThemeColorSlot, DEFAULT_THEME_COLOR_MAP,
};
use super::types::{DrawingColor, DrawingColorKind};

pub fn theme_color_slot_for(name: &str, color_map: &ThemeColorMap) -> Option<ThemeColorSlot> {
    for (logical, slot) in DEFAULT_THEME_COLOR_MAP {
        if *logical == name {
            return Some(color_map.get(*logical).copied().unwrap_or(*slot));
        }
    }
    if !is_theme_color_slot(name) {
        return None;
    }
    let slot = ThemeColorSlot::parse(name)?;
    match slot {
        ThemeColorSlot::Dk1 | ThemeColorSlot::Lt1 | ThemeColorSlot::Dk2 | ThemeColorSlot::Lt2 => {
            Some(slot)
        }
        _ => Some(color_map.get(name).copied().unwrap_or(slot)),
    }
}

pub fn resolve_scheme_color(
    scheme: &ThemeColorScheme,
    name: &str,
    color_map: &ThemeColorMap,
) -> Option<DrawingColor> {
    let slot = theme_color_slot_for(name, color_map)?;
    scheme.colors.get(slot.as_str()).cloned()
}

pub fn theme_slot_hex(color: Option<&DrawingColor>) -> Option<String> {
    let color = color?;
    let value = match color.kind {
        DrawingColorKind::Srgb => Some(color.value.as_str()),
        DrawingColorKind::System => color.fallback.as_deref(),
        _ => None,
    }?;
    Some(value.to_uppercase())
}

pub struct ThemeDrawingColorTheme {
    scheme: ThemeColorScheme,
    color_map: ThemeColorMap,
}

impl ThemeDrawingColorTheme {
    pub fn new(scheme: ThemeColorScheme, color_map: ThemeColorMap) -> Self {
        Self { scheme, color_map }
    }
}

impl DrawingColorTheme for ThemeDrawingColorTheme {
    fn scheme(&self, name: &str) -> Option<String> {
        let hex =
            theme_slot_hex(resolve_scheme_color(&self.scheme, name, &self.color_map).as_ref());
        hex.map(|h| format!("#{}", h.trim_start_matches('#')))
    }
}

pub fn theme_drawing_color_theme(
    scheme: Option<&ThemeColorScheme>,
    color_map: &ThemeColorMap,
) -> ThemeDrawingColorTheme {
    ThemeDrawingColorTheme::new(scheme.cloned().unwrap_or_default(), color_map.clone())
}
