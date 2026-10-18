// SPDX-License-Identifier: MIT
//! SVG `stroke-dasharray` from OOXML `a:prstDash` (upstream `stroke-utils.ts`).

/// CSS `border-style` for inset (`a:ln/@algn="in"`) strokes.
pub fn css_border_dash_style(dash: Option<&str>) -> &'static str {
    match dash {
        None | Some("solid") => "solid",
        Some("dot") | Some("sysDot") => "dotted",
        _ => "dashed",
    }
}

/// CSS `border-style` for compound lines (uses `double` for non-sng compounds).
pub fn css_border_compound_style(compound: Option<&str>, dash: Option<&str>) -> &'static str {
    if compound.is_some() && compound != Some("sng") {
        "double"
    } else {
        css_border_dash_style(dash)
    }
}

/// SVG `stroke-dasharray` attribute value, or `None` for a solid line.
pub fn svg_stroke_dasharray(dash: Option<&str>, stroke_width: f64) -> Option<String> {
    let dash = dash?;
    if dash == "solid" {
        return None;
    }
    let stroke = stroke_width.max(1.0);
    let s = match dash {
        "dot" | "sysDot" => format!("{} {}", stroke, stroke * 2.0),
        "dash" | "sysDash" => format!("{} {}", stroke * 4.0, stroke * 2.0),
        "lgDash" => format!("{} {}", stroke * 7.0, stroke * 2.5),
        "dashDot" | "sysDashDot" => {
            format!(
                "{} {} {} {}",
                stroke * 4.0,
                stroke * 2.0,
                stroke,
                stroke * 2.0
            )
        }
        "lgDashDot" => {
            format!(
                "{} {} {} {}",
                stroke * 7.0,
                stroke * 2.5,
                stroke,
                stroke * 2.5
            )
        }
        "lgDashDotDot" | "sysDashDotDot" => {
            format!(
                "{} {} {} {} {} {}",
                stroke * 7.0,
                stroke * 2.5,
                stroke,
                stroke * 2.0,
                stroke,
                stroke * 2.0
            )
        }
        "custom" => format!("{} {}", stroke * 3.0, stroke * 2.0),
        _ => return None,
    };
    Some(s)
}
