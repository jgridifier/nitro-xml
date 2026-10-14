// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/ui/grid/icon-sets.ts;
// changes listed in PROVENANCE.md.
//! Small SVGs for conditional-format icon sets (index 0 is the lowest bucket).

const RED: &str = "#d9372b";
const YELLOW: &str = "#f2b600";
const GREEN: &str = "#3f9b3f";
const GRAY: &str = "#7f7f7f";
const BLACK: &str = "#404040";

fn svg(body: &str, size: u32) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{size}\" height=\"{size}\" viewBox=\"0 0 16 16\" aria-hidden=\"true\">{body}</svg>"
    )
}

fn arrow(color: &str, deg: i32) -> String {
    format!(
        "<g transform=\"rotate({deg} 8 8)\"><path d=\"M8 1.5 14 8h-3.5v6.5h-5V8H2z\" fill=\"{color}\"/></g>"
    )
}

fn circle(color: &str, rim: bool) -> String {
    if rim {
        format!(
            "<circle cx=\"8\" cy=\"8\" r=\"5.5\" fill=\"{color}\" stroke=\"#333\" stroke-width=\"2.4\"/>"
        )
    } else {
        format!("<circle cx=\"8\" cy=\"8\" r=\"6.5\" fill=\"{color}\"/>")
    }
}

fn flag(color: &str) -> String {
    format!(
        "<path d=\"M3 1.5h1.3v13H3z\" fill=\"#555\"/><path d=\"M4.3 2h9l-2.6 3.5 2.6 3.5h-9z\" fill=\"{color}\"/>"
    )
}

fn symbol(kind: char, color: &str, circled: bool) -> String {
    let mark = match kind {
        'x' => {
            "<path d=\"M5.2 5.2l5.6 5.6M10.8 5.2l-5.6 5.6\" stroke=\"#fff\" stroke-width=\"2\" stroke-linecap=\"round\"/>"
                .to_string()
        }
        '!' => {
            "<path d=\"M8 4v5\" stroke=\"#fff\" stroke-width=\"2.2\" stroke-linecap=\"round\"/><circle cx=\"8\" cy=\"11.8\" r=\"1.2\" fill=\"#fff\"/>"
                .to_string()
        }
        _ => {
            "<path d=\"M4.6 8.3l2.3 2.3 4.6-4.9\" stroke=\"#fff\" stroke-width=\"2\" fill=\"none\" stroke-linecap=\"round\"/>"
                .to_string()
        }
    };
    if circled {
        format!("<circle cx=\"8\" cy=\"8\" r=\"7\" fill=\"{color}\"/>{mark}")
    } else {
        mark.replace("#fff", color)
    }
}

fn sign(index: usize) -> String {
    match index {
        0 => format!("<path d=\"M8 1.5 14.5 8 8 14.5 1.5 8z\" fill=\"{RED}\"/>"),
        1 => format!("<path d=\"M8 2 15 14H1z\" fill=\"{YELLOW}\"/>"),
        _ => circle(GREEN, false),
    }
}

fn star(fill: f32) -> String {
    let d = "M8 1.2l2 4.3 4.7.6-3.4 3.2.9 4.7L8 11.7 3.8 14l.9-4.7L1.3 6.1 6 5.5z";
    if (fill - 0.5).abs() < f32::EPSILON {
        format!(
            "<defs><linearGradient id=\"sh\"><stop offset=\".5\" stop-color=\"{YELLOW}\"/><stop offset=\".5\" stop-color=\"#fff\"/></linearGradient></defs><path d=\"{d}\" fill=\"url(#sh)\" stroke=\"{YELLOW}\"/>"
        )
    } else if fill > 0.0 {
        format!("<path d=\"{d}\" fill=\"{YELLOW}\" stroke=\"{YELLOW}\"/>")
    } else {
        format!("<path d=\"{d}\" fill=\"#fff\" stroke=\"{YELLOW}\"/>")
    }
}

fn bars(filled: usize, total: usize) -> String {
    let mut out = String::new();
    let total = total.max(1);
    for i in 0..total {
        let h = 3.0 + (i as f64 * 10.0) / (total.saturating_sub(1).max(1) as f64);
        let fill = if i < filled { "#2f6db5" } else { "#c8c8c8" };
        out.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{fill}\"/>",
            1.5 + i as f64 * (13.0 / total as f64),
            14.5 - h,
            13.0 / total as f64 - 1.0,
            h
        ));
    }
    out
}

fn quarters(n: usize) -> String {
    if n == 0 {
        return "<circle cx=\"8\" cy=\"8\" r=\"6.5\" fill=\"#fff\" stroke=\"#555\"/>".into();
    }
    if n == 4 {
        return "<circle cx=\"8\" cy=\"8\" r=\"6.5\" fill=\"#555\"/>".into();
    }
    let angle = (n as f64 / 4.0) * 2.0 * std::f64::consts::PI;
    let x = 8.0 + 6.5 * angle.sin();
    let y = 8.0 - 6.5 * angle.cos();
    format!(
        "<circle cx=\"8\" cy=\"8\" r=\"6.5\" fill=\"#fff\" stroke=\"#555\"/><path d=\"M8 8V1.5A6.5 6.5 0 0 1 {x:.2} {y:.2}z\" fill=\"#555\"/>"
    )
}

fn boxes(n: usize) -> String {
    let mut out = String::new();
    for i in 0..4 {
        let x = if i % 2 == 1 { 8.5 } else { 1.5 };
        let y = if i < 2 { 1.5 } else { 8.5 };
        let fill = if i < n { "#2f6db5" } else { "#d6d6d6" };
        out.push_str(&format!(
            "<rect x=\"{x}\" y=\"{y}\" width=\"6\" height=\"6\" fill=\"{fill}\"/>"
        ));
    }
    out
}

fn body(set: &str, index: usize) -> String {
    let arrows3 = [arrow(RED, 180), arrow(YELLOW, 90), arrow(GREEN, 0)];
    match set {
        "3Arrows" => arrows3.get(index).cloned().unwrap_or_default(),
        "3ArrowsGray" => arrow(GRAY, [180, 90, 0].get(index).copied().unwrap_or(0)),
        "3Flags" => flag([RED, YELLOW, GREEN].get(index).copied().unwrap_or(GREEN)),
        "3TrafficLights2" => circle(
            [RED, YELLOW, GREEN].get(index).copied().unwrap_or(GREEN),
            true,
        ),
        "3Signs" => sign(index),
        "3Symbols" => symbol(
            ['x', '!', 'v'].get(index).copied().unwrap_or('v'),
            [RED, YELLOW, GREEN].get(index).copied().unwrap_or(GREEN),
            true,
        ),
        "3Symbols2" => symbol(
            ['x', '!', 'v'].get(index).copied().unwrap_or('v'),
            [RED, YELLOW, GREEN].get(index).copied().unwrap_or(GREEN),
            false,
        ),
        "3Stars" => star([0.0, 0.5, 1.0].get(index).copied().unwrap_or(1.0)),
        "3Triangles" => {
            if index == 1 {
                format!("<rect x=\"3\" y=\"7\" width=\"10\" height=\"2.5\" fill=\"{YELLOW}\"/>")
            } else if index == 0 {
                format!("<path d=\"M2 5h12L8 13z\" fill=\"{RED}\"/>")
            } else {
                format!("<path d=\"M2 12h12L8 4z\" fill=\"{GREEN}\"/>")
            }
        }
        "4Arrows" => arrow(
            [RED, YELLOW, YELLOW, GREEN]
                .get(index)
                .copied()
                .unwrap_or(GREEN),
            [180, 135, 45, 0].get(index).copied().unwrap_or(0),
        ),
        "4ArrowsGray" => arrow(GRAY, [180, 135, 45, 0].get(index).copied().unwrap_or(0)),
        "4RedToBlack" => circle(
            [BLACK, GRAY, "#f4a6a6", RED]
                .get(index)
                .copied()
                .unwrap_or(RED),
            false,
        ),
        "4Rating" => bars(index + 1, 4),
        "4TrafficLights" => circle(
            [BLACK, RED, YELLOW, GREEN]
                .get(index)
                .copied()
                .unwrap_or(GREEN),
            true,
        ),
        "5Arrows" => arrow(
            [RED, YELLOW, YELLOW, YELLOW, GREEN]
                .get(index)
                .copied()
                .unwrap_or(GREEN),
            [180, 135, 90, 45, 0].get(index).copied().unwrap_or(0),
        ),
        "5ArrowsGray" => arrow(GRAY, [180, 135, 90, 45, 0].get(index).copied().unwrap_or(0)),
        "5Rating" => bars(index, 4),
        "5Quarters" => quarters(index),
        "5Boxes" => boxes(index),
        // 3TrafficLights1 and unknown sets.
        _ => circle(
            [RED, YELLOW, GREEN]
                .get(index.min(2))
                .copied()
                .unwrap_or(GREEN),
            false,
        ),
    }
}

/// Inner SVG body (no outer `<svg>` wrapper) for one icon of a set.
pub fn icon_body(set: &str, index: u32) -> String {
    body(set, index as usize)
}

/// SVG markup for one icon of a set.
pub fn icon_svg(set: &str, index: u32, size: u32) -> String {
    svg(&icon_body(set, index), size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traffic_lights_default_emits_svg() {
        // Catches: missing default icon-set body so CF samples paint empty xg-ic nodes.
        let s = icon_svg("3TrafficLights1", 1, 16);
        assert!(s.contains("<svg"));
        assert!(s.contains(YELLOW));
    }
}
