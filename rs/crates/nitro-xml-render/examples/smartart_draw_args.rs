// SPDX-License-Identifier: MIT
//! Dump SmartArt draw arguments / SVG for draw-arg differentials (#27).

fn main() {
    let mut args = std::env::args().skip(1);
    let xml_path = args.next().expect("drawing.xml");
    let svg_path = args.next();
    let xml = std::fs::read_to_string(&xml_path).expect("read xml");
    let draw = nitro_xml_render::smartart::smartart_draw_args(Some(&xml), None);
    for (i, a) in draw.iter().enumerate() {
        println!(
            "{i}\tkind={}\tgeom={}\tx={:.0}\ty={:.0}\tw={:.0}\th={:.0}\tfill={}\tstroke={}\tsw={:.0}\ttext={:?}\tpath_len={}",
            a.kind,
            a.geom,
            a.x,
            a.y,
            a.w,
            a.h,
            a.fill,
            a.stroke,
            a.stroke_width,
            a.text,
            a.path_len
        );
    }
    if let Some(out) = svg_path {
        let svg = nitro_xml_render::smartart::smartart_svg(Some(&xml), 960.0, 540.0);
        std::fs::write(out, svg).expect("write svg");
    }
}
