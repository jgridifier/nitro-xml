// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/layout/units.ts; changes listed in PROVENANCE.md.

pub const DEFAULT_MAX_DIGIT_WIDTH: f64 = 7.0;
pub const DEFAULT_BASE_COL_WIDTH: f64 = 8.0;
pub const COLUMN_PADDING_PX: f64 = 5.0;
pub const INDENT_PX_PER_LEVEL: f64 = 9.0;

#[inline]
fn trunc(n: f64) -> i64 {
    n.trunc() as i64
}

pub fn column_width_to_pixels(width: f64, mdw: f64) -> i32 {
    if width <= 0.0 {
        return 0;
    }
    let mdw = if mdw > 0.0 {
        mdw
    } else {
        DEFAULT_MAX_DIGIT_WIDTH
    };
    trunc(((256.0 * width + trunc(128.0 / mdw) as f64) / 256.0) * mdw) as i32
}

pub fn characters_to_column_width(chars: f64, mdw: f64) -> f64 {
    if chars <= 0.0 {
        return 0.0;
    }
    let mdw = if mdw > 0.0 {
        mdw
    } else {
        DEFAULT_MAX_DIGIT_WIDTH
    };
    trunc(((chars * mdw + COLUMN_PADDING_PX) / mdw) * 256.0) as f64 / 256.0
}

pub fn default_column_pixels(default_col_width: Option<f64>, mdw: f64, base_col_width: f64) -> i32 {
    if let Some(w) = default_col_width {
        if w > 0.0 {
            return column_width_to_pixels(w, mdw);
        }
    }
    let mdw = if mdw > 0.0 {
        mdw
    } else {
        DEFAULT_MAX_DIGIT_WIDTH
    };
    let px = base_col_width * mdw + COLUMN_PADDING_PX;
    ((px / 8.0).ceil() * 8.0) as i32
}

pub fn points_to_pixels(pt: f64) -> i32 {
    if pt > 0.0 {
        ((pt * 96.0) / 72.0).round() as i32
    } else {
        0
    }
}
