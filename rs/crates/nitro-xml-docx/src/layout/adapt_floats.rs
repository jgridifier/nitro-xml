// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/adapt-floats.ts; changes listed in PROVENANCE.md.

use serde_json::Value;

use super::input::LayoutFloat;

pub fn paragraph_floats(paragraph: &Value) -> Vec<LayoutFloat> {
    let mut floats = Vec::new();
    if let Some(runs) = paragraph.get("runs").and_then(|v| v.as_array()) {
        for run in runs {
            let image = run.get("image");
            let Some(image) = image else { continue };
            if !image.get("anchored").and_then(|v| v.as_bool()).unwrap_or(false) {
                continue;
            }
            let part_name = image.get("partName").and_then(|v| v.as_str()).filter(|p| !p.is_empty());
            let Some(part_name) = part_name else { continue };
            floats.push(LayoutFloat {
                part_name: part_name.to_string(),
                content_type: image
                    .get("contentType")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                width_px: image.get("widthPx").and_then(|v| v.as_f64()).unwrap_or(0.0),
                height_px: image.get("heightPx").and_then(|v| v.as_f64()).unwrap_or(0.0),
                relative_from_h: image
                    .get("placement")
                    .and_then(|p| p.get("relativeFrom"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                align_h: image
                    .get("placement")
                    .and_then(|p| p.get("align"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                offset_x_px: image
                    .get("placement")
                    .and_then(|p| p.get("offsetXPx"))
                    .and_then(|v| v.as_f64()),
                relative_from_v: image
                    .get("placement")
                    .and_then(|p| p.get("relativeFromV"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                align_v: image
                    .get("placement")
                    .and_then(|p| p.get("alignV"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                offset_y_px: image
                    .get("placement")
                    .and_then(|p| p.get("offsetYPx"))
                    .and_then(|v| v.as_f64()),
                behind_text: image
                    .get("placement")
                    .and_then(|p| p.get("behindText"))
                    .and_then(|v| v.as_bool()),
                wrap: image
                    .get("placement")
                    .and_then(|p| p.get("wrap"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
        }
    }
    floats
}

pub fn floats_of(paragraph: &Value) -> Option<Vec<LayoutFloat>> {
    let floats = paragraph_floats(paragraph);
    if floats.is_empty() {
        None
    } else {
        Some(floats)
    }
}
