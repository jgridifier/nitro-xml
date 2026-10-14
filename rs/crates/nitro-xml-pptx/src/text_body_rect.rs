// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/text-body-rect.ts; changes in PROVENANCE.md.

use std::collections::HashMap;
use std::sync::Mutex;

use nitro_xml_opc::drawingml::evaluate_preset_shape;

use crate::verified_text_rect_presets::is_verified_text_rect_preset;

/// Extra padding (px) from the geometry text rectangle (`a:rect`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextBodyRectPadding {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

const NO_PADDING: TextBodyRectPadding = TextBodyRectPadding {
    left: 0.0,
    top: 0.0,
    right: 0.0,
    bottom: 0.0,
};

const MIN_INSET_PX: f64 = 0.5;
const MIN_CONTENT_PX: f64 = 8.0;
const CACHE_LIMIT: usize = 512;

static RECT_CACHE: Mutex<Option<HashMap<String, TextBodyRectPadding>>> = Mutex::new(None);

fn cache_key(
    shape_type: &str,
    width: f64,
    height: f64,
    adjustments: Option<&HashMap<String, f64>>,
) -> String {
    let adj = match adjustments {
        None => String::new(),
        Some(map) => {
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            keys.iter()
                .map(|k| format!("{}={}", k, map[*k]))
                .collect::<Vec<_>>()
                .join(",")
        }
    };
    format!("{}|{}|{}|{}", shape_type, width, height, adj)
}

fn to_padding(
    l: f64,
    t: f64,
    r: f64,
    b: f64,
    width: f64,
    height: f64,
) -> Option<TextBodyRectPadding> {
    if ![l, t, r, b].iter().all(|n| n.is_finite()) {
        return None;
    }
    if r - l < MIN_CONTENT_PX || b - t < MIN_CONTENT_PX {
        return None;
    }
    let left = 0.0_f64.max(l.min(width));
    let top = 0.0_f64.max(t.min(height));
    let right = 0.0_f64.max(width - 0.0_f64.max(r.min(width)));
    let bottom = 0.0_f64.max(height - 0.0_f64.max(b.min(height)));
    if width - left - right < MIN_CONTENT_PX || height - top - bottom < MIN_CONTENT_PX {
        return None;
    }
    if left < MIN_INSET_PX && top < MIN_INSET_PX && right < MIN_INSET_PX && bottom < MIN_INSET_PX {
        return None;
    }
    Some(TextBodyRectPadding {
        left,
        top,
        right,
        bottom,
    })
}

/// Resolve geometry text-rectangle padding for a shape box (upstream `resolveTextBodyRectPadding`).
pub fn resolve_text_body_rect_padding(
    shape_type: Option<&str>,
    width: f64,
    height: f64,
    adjustments: Option<&HashMap<String, f64>>,
) -> TextBodyRectPadding {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return NO_PADDING;
    }
    let shape_type = shape_type.unwrap_or("").trim();
    if shape_type.is_empty() || !is_verified_text_rect_preset(shape_type) {
        return NO_PADDING;
    }

    let key = cache_key(shape_type, width, height, adjustments);
    if let Ok(mut guard) = RECT_CACHE.lock() {
        let cache = guard.get_or_insert_with(HashMap::new);
        if let Some(hit) = cache.get(&key) {
            return *hit;
        }
        let padding = compute_padding(shape_type, width, height, adjustments);
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, padding);
        return padding;
    }
    compute_padding(shape_type, width, height, adjustments)
}

fn compute_padding(
    shape_type: &str,
    width: f64,
    height: f64,
    adjustments: Option<&HashMap<String, f64>>,
) -> TextBodyRectPadding {
    let evaluated = evaluate_preset_shape(shape_type, width, height, adjustments);
    let rect = evaluated.and_then(|e| e.text_rect);
    match rect {
        Some(tr) => to_padding(tr.l, tr.t, tr.r, tr.b, width, height).unwrap_or(NO_PADDING),
        None => NO_PADDING,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Catches: roundRect text laid out on full bbox instead of COM-verified inscribed rect.
    #[test]
    fn round_rect_insets_text_body() {
        let pad = resolve_text_body_rect_padding(Some("roundRect"), 200.0, 100.0, None);
        assert!(pad.left > 2.0 && pad.left < 6.0);
        assert!(pad.top > 2.0 && pad.top < 6.0);
        assert!((pad.left - pad.top).abs() < 0.5);
        assert!((pad.left - pad.right).abs() < 0.5);
        assert!((pad.top - pad.bottom).abs() < 0.5);
    }

    // Catches: chevron text spills past arrow points when geometry rect padding is skipped.
    #[test]
    fn chevron_insets_horizontal() {
        let pad = resolve_text_body_rect_padding(Some("chevron"), 200.0, 100.0, None);
        assert_eq!(pad.left, 50.0);
        assert_eq!(pad.right, 50.0);
        assert_eq!(pad.top, 0.0);
        assert_eq!(pad.bottom, 0.0);
    }

    // Catches: homePlate preset insets only the right edge (ECMA `r=ir`, l/t/b on box).
    #[test]
    fn homeplate_insets_right_only() {
        let pad = resolve_text_body_rect_padding(Some("homePlate"), 260.0, 160.0, None);
        assert!(pad.left < 0.5 && pad.top < 0.5 && pad.bottom < 0.5);
        assert!(pad.right > 20.0);
    }
}
