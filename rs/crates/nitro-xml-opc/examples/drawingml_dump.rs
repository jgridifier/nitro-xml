// SPDX-License-Identifier: MIT
//! JSON CLI for the #25 DrawingML differential gate.

use std::collections::HashMap;
use std::process;

use nitro_xml_opc::drawingml::{
    evaluate_preset_shape, list_preset_names, parse_drawing_fill, parse_drawing_line,
    parse_theme_xml, resolve_drawing_color, theme_drawing_color_theme, DrawingColor,
    DrawingColorTheme, HslRounding, ResolveOptions, TransformOrder, THEME_COLOR_SLOTS,
};
use nitro_xml_opc::xml::parse_xml_str;
use serde::Deserialize;
use serde_json::json;

fn fail_json(value: serde_json::Value) -> ! {
    println!(
        "{}",
        serde_json::to_string(&value).unwrap_or_else(|_| "{}".into())
    );
    process::exit(2);
}

#[derive(Deserialize)]
struct ColorInput {
    color: DrawingColor,
    #[serde(default)]
    theme: HashMap<String, String>,
    #[serde(default)]
    transform_order: Option<String>,
    #[serde(default)]
    hsl_rounding: Option<String>,
}

struct MapTheme(HashMap<String, String>);

impl DrawingColorTheme for MapTheme {
    fn scheme(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        fail_json(json!({"error": "usage"}));
    }
    let mode = args.remove(0);
    match mode.as_str() {
        "color" => cmd_color(args),
        "preset" => cmd_preset(args),
        "fill" => cmd_fill(args),
        "theme-colors" => cmd_theme_colors(args),
        "list-presets" => {
            println!("{}", serde_json::to_string(&list_preset_names()).unwrap());
        }
        _ => fail_json(json!({"error": "usage"})),
    }
}

fn cmd_color(args: Vec<String>) {
    let raw = args.first().map(|s| s.as_str()).unwrap_or("");
    let input: ColorInput = serde_json::from_str(raw).unwrap_or_else(|_| {
        fail_json(json!({"error": "invalid_json"}));
    });
    let theme = MapTheme(input.theme);
    let transform_order = match input.transform_order.as_deref() {
        Some("document") => TransformOrder::Document,
        _ => TransformOrder::Legacy,
    };
    let hsl_rounding = match input.hsl_rounding.as_deref() {
        Some("halfDown") => HslRounding::HalfDown,
        _ => HslRounding::Nearest,
    };
    let resolved = resolve_drawing_color(
        &input.color,
        Some(&theme),
        Some(ResolveOptions {
            hsl_rounding,
            transform_order,
        }),
    );
    match resolved {
        Some(r) => println!("{}", serde_json::to_string(&r).unwrap()),
        None => fail_json(json!({"error": "unresolved"})),
    }
}

fn cmd_preset(args: Vec<String>) {
    let name = args.first().map(|s| s.as_str()).unwrap_or("");
    let w = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let h = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let adj: Option<HashMap<String, f64>> = args.get(3).and_then(|s| serde_json::from_str(s).ok());
    match evaluate_preset_shape(name, w, h, adj.as_ref()) {
        Some(r) => println!("{}", serde_json::to_string(&r).unwrap()),
        None => fail_json(json!({"error": "unknown_preset"})),
    }
}

fn cmd_fill(args: Vec<String>) {
    let path = args.first().map(|s| s.as_str()).unwrap_or("");
    let xml = std::fs::read_to_string(path).unwrap_or_else(|_| {
        fail_json(json!({"error": "io"}));
    });
    let doc = parse_xml_str(&xml).unwrap_or_else(|_| fail_json(json!({"error": "xml"})));
    let fill = parse_drawing_fill(Some(&doc.root));
    let line = parse_drawing_line(Some(&doc.root));
    println!(
        "{}",
        serde_json::to_string(&json!({ "fill": fill, "line": line })).unwrap()
    );
}

fn cmd_theme_colors(args: Vec<String>) {
    let path = args.first().map(|s| s.as_str()).unwrap_or("");
    let xml = std::fs::read_to_string(path).unwrap_or_else(|_| {
        fail_json(json!({"error": "io"}));
    });
    let theme = parse_theme_xml(&xml, "theme");
    let color_theme = theme_drawing_color_theme(Some(&theme.color_scheme), &Default::default());
    let mut slots = serde_json::Map::new();
    for slot in THEME_COLOR_SLOTS {
        let name = slot.as_str();
        if let Some(color) = theme.color_scheme.colors.get(name) {
            let base = resolve_drawing_color(color, Some(&color_theme), None);
            let common = resolve_drawing_color(
                color,
                Some(&color_theme),
                Some(ResolveOptions {
                    transform_order: TransformOrder::Legacy,
                    hsl_rounding: HslRounding::Nearest,
                }),
            );
            let document = resolve_drawing_color(
                color,
                Some(&color_theme),
                Some(ResolveOptions {
                    transform_order: TransformOrder::Document,
                    hsl_rounding: HslRounding::Nearest,
                }),
            );
            slots.insert(
                name.into(),
                json!({
                    "color": color,
                    "legacy": common,
                    "document": document,
                    "base": base,
                }),
            );
        }
    }
    println!("{}", serde_json::to_string(&slots).unwrap());
}
