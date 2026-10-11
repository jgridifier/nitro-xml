// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/watermark.ts (parseWatermarkShape);
// changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::Element;

use crate::xml_util::descendant_ns;

const VML_NS: &str = "urn:schemas-microsoft-com:vml";
const WATERMARK_PREFIX: &str = "PowerPlusWaterMarkObject";
const POINT_TO_PX: f64 = 96.0 / 72.0;

fn color_of(value: Option<&str>) -> &'static str {
    let text = value.unwrap_or("").trim().to_lowercase();
    let word = text.split_whitespace().next().unwrap_or("");
    match word {
        "silver" => "#c0c0c0",
        "gray" | "grey" => "#808080",
        "black" => "#000000",
        "white" => "#ffffff",
        "red" => "#ff0000",
        "blue" => "#0000ff",
        "green" => "#008000",
        "yellow" => "#ffff00",
        _ => "#c0c0c0",
    }
}

pub fn parse_watermark_shape(shape: Option<&Element>) -> Option<Value> {
    let shape = shape?;
    let id = shape
        .attrs
        .iter()
        .find(|a| a.local == "id")
        .map(|a| a.value.as_str())
        .unwrap_or("");
    if !id.starts_with(WATERMARK_PREFIX) {
        return None;
    }
    let path = descendant_ns(Some(shape), VML_NS, "textpath")?;
    let style = shape
        .attrs
        .iter()
        .find(|a| a.local == "style")
        .map(|a| a.value.as_str())
        .unwrap_or("");
    let fill = descendant_ns(Some(shape), VML_NS, "fill");
    let opacity = fill.and_then(|f| {
        f.attrs
            .iter()
            .find(|a| a.local == "opacity")
            .map(|a| a.value.as_str())
    });
    let path_style = path
        .attrs
        .iter()
        .find(|a| a.local == "style")
        .map(|a| a.value.as_str())
        .unwrap_or("");
    let family = parse_font_family(path_style);
    let width = parse_pt(style, "width");
    let height = parse_pt(style, "height");
    let rotation = parse_rotation(style);
    let text = path
        .attrs
        .iter()
        .find(|a| a.local == "string")
        .map(|a| a.value.clone())
        .unwrap_or_default();
    let fillcolor = shape
        .attrs
        .iter()
        .find(|a| a.local == "fillcolor")
        .map(|a| a.value.as_str());
    let layout = if let Some(r) = rotation.and_then(|s| s.parse::<f64>().ok()) {
        if (r % 360.0).abs() > 1.0 {
            "diagonal"
        } else {
            "horizontal"
        }
    } else {
        "horizontal"
    };
    let mut watermark = serde_json::Map::new();
    watermark.insert("text".into(), json!(text));
    watermark.insert("color".into(), json!(color_of(fillcolor)));
    watermark.insert(
        "semitransparent".into(),
        json!(opacity.is_some() && opacity != Some("1") && opacity != Some("")),
    );
    watermark.insert("layout".into(), json!(layout));
    if let Some(f) = family {
        watermark.insert("fontFamily".into(), json!(f.trim()));
    }
    let width_px = width
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v * POINT_TO_PX).round() as i64)
        .unwrap_or(0);
    let height_px = height
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| (v * POINT_TO_PX).round() as i64)
        .unwrap_or(0);
    Some(json!({
        "relId": "",
        "partName": "",
        "contentType": "application/octet-stream",
        "widthPx": width_px,
        "heightPx": height_px,
        "unsupported": "Watermark",
        "watermark": Value::Object(watermark),
    }))
}

fn parse_font_family(style: &str) -> Option<String> {
    let lower = style.to_lowercase();
    let idx = lower.find("font-family:")?;
    let rest = &style[idx + 12..];
    let mut out = String::new();
    for c in rest.chars() {
        if c == ';' || c == '"' {
            break;
        }
        out.push(c);
    }
    let t = out.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn parse_rotation(style: &str) -> Option<String> {
    let lower = style.to_lowercase();
    let idx = lower.find("rotation:")?;
    let rest = &style[idx + 9..];
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    num.parse::<f64>().ok().map(|n| n.to_string())
}

fn parse_pt(style: &str, prop: &str) -> Option<String> {
    let lower = style.to_lowercase();
    let key = format!("{}:", prop);
    let pos = lower.find(&key)?;
    let rest = &style[pos + key.len()..];
    let num: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if num.is_empty() {
        None
    } else {
        Some(num)
    }
}
