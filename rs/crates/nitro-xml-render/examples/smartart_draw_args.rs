// SPDX-License-Identifier: MIT
//! Dump SmartArt draw arguments / SVG for draw-arg differentials (#27).
//!
//! Usage: `smartart_draw_args drawing.xml [out.svg] [scheme.txt]`
//!
//! `scheme.txt` lines are `accent1=#156082` (optional; without it scheme colours stay unresolved).

use std::collections::BTreeMap;
use std::fs;

fn load_scheme(path: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in fs::read_to_string(path).expect("scheme").lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        map.insert(k.trim().to_string(), v.trim().to_string());
    }
    map
}

fn main() {
    let mut args = std::env::args().skip(1);
    let xml_path = args.next().expect("drawing.xml");
    let mut svg_path = None;
    let mut scheme_path = None;
    for arg in args {
        if arg.ends_with(".svg") {
            svg_path = Some(arg);
        } else {
            scheme_path = Some(arg);
        }
    }
    let xml = fs::read_to_string(&xml_path).expect("read xml");
    let scheme = scheme_path.map(|p| load_scheme(&p));
    let scheme_ref = scheme.as_ref();
    let draw = nitro_xml_render::smartart::smartart_draw_args(Some(&xml), scheme_ref);
    for (i, a) in draw.iter().enumerate() {
        println!(
            "{i}\tkind={}\tgeom={}\tx={:.0}\ty={:.0}\tw={:.0}\th={:.0}\tfill={}\tstroke={}\tsw={:.2}\tfont={}\ttext={:?}\tpath_len={}",
            a.kind,
            a.geom,
            a.x,
            a.y,
            a.w,
            a.h,
            a.fill,
            a.stroke,
            a.stroke_width,
            a.font_color,
            a.text,
            a.path_len
        );
    }
    if let Some(out) = svg_path {
        let svg = nitro_xml_render::smartart::smartart_svg_with_scheme(
            Some(&xml),
            960.0,
            540.0,
            scheme_ref,
        );
        fs::write(out, svg).expect("write svg");
    }
}
