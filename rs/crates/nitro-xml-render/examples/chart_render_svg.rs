// SPDX-License-Identifier: MIT
//! SVG CLI for chart S1 differential (#28).

use nitro_xml_opc::chart::parse_chart_space_xml;
use nitro_xml_render::chart::{render_chart_space_svg, ChartSvgOptions, RenderChartSpaceOptions};
use std::io::{self, Read};

fn read_xml() -> String {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).expect("stdin");
        return buf;
    }
    if args[0] == "--file" {
        let path = args.get(1).expect("path");
        return std::fs::read_to_string(path).expect("read file");
    }
    args.remove(0)
}

fn main() {
    let xml = read_xml();
    let parsed = parse_chart_space_xml(&xml);
    let svg = render_chart_space_svg(
        &parsed.chart_space,
        &RenderChartSpaceOptions {
            width: 480,
            height: 320,
            svg: ChartSvgOptions {
                measure_widths: Vec::new(),
            },
        },
    );
    print!("{}", svg);
}
