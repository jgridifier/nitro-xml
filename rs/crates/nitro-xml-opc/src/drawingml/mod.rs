// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/drawingml/ and related geometry/color modules;
// changes listed in PROVENANCE.md.

mod color;
mod color_linear;
mod color_primitives;
mod dom;
mod fill;
mod geometry_parse;
mod guide_api;
mod guide_eval;
mod ooxml_arc;
mod ordered_color_transforms;
mod preset;
mod serialize;
mod theme;
mod theme_color;
mod theme_model;
mod types;
mod units;

pub use color::{
    parse_drawing_color, parse_drawing_color_in, parse_drawing_color_list, resolve_drawing_color,
    DrawingColorTheme, HslRounding, ResolveOptions, ResolvedDrawingColor, TransformOrder,
};
pub use fill::{fill_element_of, parse_drawing_fill, parse_drawing_line};
pub use geometry_parse::{
    parse_drawing_geometry, parse_frame, parse_transform, ParsedGeometry, ParsedTransform,
};
pub use guide_api::{create_builtin_variables, evaluate_guides, GeometryContext, GeometryGuide};
pub use guide_eval::{
    angle_to_radians, evaluate_formula, parse_formula, resolve_operand, ANGLE_SCALE,
};
pub use ooxml_arc::{ooxml_arc_to_svg, ArcToResult};
pub use ordered_color_transforms::ordered_color_transforms;
pub use preset::{
    evaluate_preset_shape, list_preset_names, lookup_preset_shape, PresetShapeEvaluationResult,
    PresetSubpathResult,
};
pub use theme::{is_theme_color_slot, parse_theme, parse_theme_color_map, parse_theme_xml};
pub use theme_color::{
    resolve_scheme_color, theme_color_slot_for, theme_drawing_color_theme, theme_slot_hex,
};
pub use theme_model::{
    DrawingTheme, ThemeColorMap, ThemeColorMapKey, ThemeColorScheme, ThemeColorSlot,
    ThemeFontCollection, ThemeFontScheme, ThemeLogicalColor, DEFAULT_THEME_COLOR_MAP,
    THEME_COLOR_SLOTS,
};
pub use types::{
    AttributeReader, DrawingColor, DrawingFill, DrawingFrame, DrawingLine, DrawingPath,
    DrawingPathCommand, DrawingTextBody, DrawingTextParagraph, DrawingTextRun, DrawingTextSpacing,
};
pub use units::{emu_to_pixels, pixels_to_emu, EMU_PER_PIXEL};

pub use color_linear::{linear_to_srgb255, srgb255_to_linear};
pub use color_primitives::{
    clamp_unit_interval, hex_to_rgb_channels, hsl_to_rgb, parse_drawing_fraction,
    parse_drawing_hue_degrees, parse_drawing_percent, rgb_to_hsl, scrgb_linear_to_srgb8, to_hex,
};
