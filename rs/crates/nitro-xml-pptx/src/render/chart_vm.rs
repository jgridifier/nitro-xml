// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-cartesian.ts,
// chart-cartesian-bars.ts, chart-view-model-layout.ts, chart-view-model-chrome.ts,
// chart-axis-render.ts, chart-category-axis.ts, chart-legend-build.ts, chart-legend-layout.ts,
// chart-style-palette.ts and chart-view-model-dom.ts (clustered column subset);
// changes listed in PROVENANCE.md.
//! pptx chart view-model engine (flat clustered columns, radar; pie / doughnut in
//! `chart_vm_pie.rs`). `render` returns `None` for any chart feature not yet ported so the
//! caller falls back to the S1 SVG painter.

use std::collections::HashMap;
use std::fmt::Write as _;

use nitro_xml_opc::chart::{ChartAxis, ChartGroupKind, ChartSpace, ChartTextBody};
use nitro_xml_opc::drawingml::{resolve_drawing_color, DrawingFill};
use nitro_xml_render::chart::THEME_SLOTS;

use super::font_substitution::substitute_font_family_stack;

#[path = "chart_vm_line.rs"]
mod line;
#[path = "chart_vm_pie.rs"]
mod pie;

const PX_PER_PT: f64 = 4.0 / 3.0;
const DEFAULT_TEXT_PX: f64 = 10.0 * PX_PER_PT;
const AXIS_LABEL_COLOR: &str = "#64748b";
const GRIDLINE_COLOR: &str = "#e2e8f0";
const OFFICE_ACCENTS: [&str; 6] = [
    "#4472C4", "#ED7D31", "#A5A5A5", "#FFC000", "#5B9BD5", "#70AD47",
];

#[derive(Clone, Default)]
struct TextStyle {
    size_pt: Option<f64>,
    color: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    family: Option<String>,
}

fn merge(base: &TextStyle, over: &TextStyle) -> TextStyle {
    TextStyle {
        size_pt: over.size_pt.or(base.size_pt),
        color: over.color.clone().or_else(|| base.color.clone()),
        bold: over.bold.or(base.bold),
        italic: over.italic.or(base.italic),
        family: over.family.clone().or_else(|| base.family.clone()),
    }
}

/// Theme inputs the chart needs beyond the chart part.
pub struct VmTheme {
    /// Unique prefix for SVG pattern ids (the slide element id).
    pub picture_id_prefix: String,
    pub scheme: HashMap<String, String>,
    pub minor_latin: Option<String>,
}

pub fn theme_accents(scheme: &HashMap<String, String>) -> Option<Vec<String>> {
    let v: Vec<String> = (1..=6)
        .filter_map(|i| scheme.get(&format!("accent{i}")).cloned())
        .collect();
    (v.len() == 6).then_some(v)
}

fn def_rpr(tx: Option<&ChartTextBody>, scheme: &HashMap<String, String>) -> Option<TextStyle> {
    let p = tx?.body.paragraphs.first()?.default_properties.as_ref()?;
    let color = p.color.as_ref().and_then(|c| {
        resolve_drawing_color(c, Some(scheme), None).and_then(|r| (r.alpha == 1.0).then_some(r.hex))
    });
    Some(TextStyle {
        size_pt: p.size_pt,
        color,
        bold: p.bold,
        italic: p.italic,
        family: p
            .typeface
            .clone()
            .filter(|f| !f.starts_with('+') && !f.is_empty()),
    })
}

fn family_css(face: &Option<String>) -> Option<String> {
    face.as_ref().map(|f| substitute_font_family_stack(Some(f)))
}

fn hex_rgb(h: &str) -> Option<[f64; 3]> {
    let h = h.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(f64::from);
    Some([p(0)?, p(2)?, p(4)?])
}
fn rgb_hex(c: [f64; 3]) -> String {
    let f = |v: f64| (v + 0.5).floor().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", f(c[0]), f(c[1]), f(c[2]))
}
fn tint(h: &str, a: f64) -> String {
    let c = hex_rgb(h).unwrap_or([0.0; 3]);
    rgb_hex(c.map(|v| v + (255.0 - v) * a))
}
fn shade(h: &str, a: f64) -> String {
    let c = hex_rgb(h).unwrap_or([0.0; 3]);
    rgb_hex(c.map(|v| v * (1.0 - a)))
}
fn ramp(base: &str) -> Vec<String> {
    vec![
        shade(base, 0.5),
        shade(base, 0.35),
        shade(base, 0.15),
        base.to_string(),
        tint(base, 0.2),
        tint(base, 0.4),
        tint(base, 0.6),
        tint(base, 0.8),
    ]
}

fn style_palette(style_id: Option<f64>, accents: Option<Vec<String>>) -> Vec<String> {
    let acc: Vec<String> = match accents {
        Some(a) if a.iter().all(|c| hex_rgb(c).is_some() && c.starts_with('#')) => a,
        _ => OFFICE_ACCENTS.iter().map(|s| s.to_string()).collect(),
    };
    let extras = ["#FF0000", "#00B0F0"];
    match style_id {
        Some(id) if (1.0..=48.0).contains(&id) => {
            let col = ((id.trunc() as i64) - 1).rem_euclid(8) as usize;
            match col {
                0 => ramp("#7F7F7F"),
                1 => acc
                    .iter()
                    .cloned()
                    .chain(acc.iter().map(|a| shade(a, 0.4)))
                    .collect(),
                n => ramp(&acc[n - 2]),
            }
        }
        _ => acc.into_iter().chain(extras.map(String::from)).collect(),
    }
}

struct Series {
    name: String,
    values: Vec<f64>,
    color: Option<String>,
    /// `a:blipFill` series fill: (image URL, `a:alphaModFix` opacity).
    picture: Option<(String, Option<f64>)>,
    invert: bool,
}

struct Layout {
    w: f64,
    h: f64,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}
impl Layout {
    fn pw(&self) -> f64 {
        (self.right - self.left).max(1.0)
    }
}

struct Range {
    min: f64,
    max: f64,
    span: f64,
    unit: Option<f64>,
    reverse: bool,
}
fn value_y(v: f64, r: &Range, top: f64, bottom: f64) -> f64 {
    let ratio = (v - r.min) / r.span;
    if r.reverse {
        top + ratio * (bottom - top)
    } else {
        bottom - ratio * (bottom - top)
    }
}

/// Raw cache point text by index (`ptCount` slots, missing points empty).
fn cache_texts(src: Option<&nitro_xml_opc::chart::ChartDataSource>) -> Vec<String> {
    let Some(cache) = src.and_then(|s| s.cache.as_ref().or_else(|| s.levels.as_ref()?.first()))
    else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for (i, p) in cache.points.iter().enumerate() {
        let idx = p.index.unwrap_or(i as i64).max(0) as usize;
        if out.len() <= idx {
            out.resize(idx + 1, String::new());
        }
        out[idx] = p.value.clone();
    }
    out
}

/// Plain-title subset: no authored run/title formatting, layout or overlay.
fn title_supported(space: &ChartSpace) -> bool {
    let Some(t) = &space.title else { return true };
    let plain = |p: &nitro_xml_opc::drawingml::DrawingTextRunProperties| {
        p.size_pt.is_none()
            && p.bold.is_none()
            && p.italic.is_none()
            && p.color.is_none()
            && p.typeface.is_none()
    };
    let rich_plain = t.tx.as_ref().and_then(|x| x.rich.as_ref()).is_none_or(|r| {
        r.body.paragraphs.iter().all(|p| {
            p.default_properties.as_ref().is_none_or(plain)
                && p.runs.iter().all(|r| {
                    r.size_pt.is_none()
                        && r.bold.is_none()
                        && r.italic.is_none()
                        && r.color.is_none()
                        && r.typeface.is_none()
                })
        })
    });
    rich_plain
        && t.tx_pr.is_none()
        && t.sp_pr.is_none()
        && t.overlay != Some(true)
        && t.layout
            .as_ref()
            .is_none_or(|l| l.manual.x.is_none() && l.manual.y.is_none())
        && title_text(space).is_some()
}

fn title_text(space: &ChartSpace) -> Option<String> {
    let t = space.title.as_ref()?;
    t.text
        .clone()
        .or_else(|| {
            t.tx.as_ref().and_then(|x| {
                x.text
                    .clone()
                    .or_else(|| x.rich.as_ref().map(|r| r.body.text.clone()))
            })
        })
        .filter(|s| !s.is_empty())
}

/// `buildRadarViewModel` primitives and labels (flat radar, no data labels).
fn radar_body(
    o: &mut String,
    g: &nitro_xml_opc::chart::ChartPlotGroup,
    series: &[Series],
    categories: &[String],
    l: &Layout,
    palette: &[String],
) {
    let cx = l.left + l.pw() / 2.0;
    let ph = (l.bottom - l.top).max(1.0);
    let cy = l.top + ph / 2.0;
    let radius = (l.pw().min(ph) / 2.0 - 4.0).max(1.0);
    let cat_count = categories.len().max(1);
    let max_val = series
        .iter()
        .flat_map(|s| s.values.iter().map(|v| v.abs()))
        .fold(1.0, f64::max);
    let angle = |i: usize| {
        std::f64::consts::PI * 2.0 * i as f64 / cat_count as f64 - std::f64::consts::PI / 2.0
    };
    let none: Option<String> = None;
    // Perimeter labels come before the primitives in the DOM projector.
    for i in 0..cat_count {
        let a = angle(i);
        push_text(
            o,
            &Text {
                x: cx + (radius + 10.0) * a.cos(),
                y: cy + (radius + 10.0) * a.sin(),
                text: categories.get(i).map(String::as_str).unwrap_or(""),
                anchor: "middle",
                size: DEFAULT_TEXT_PX,
                fill: "#64748b",
                bold: None,
                family: &none,
                baseline: Some("central"),
            },
        );
    }
    for r in 1..=4 {
        let rr = radius * r as f64 / 4.0;
        let pts: Vec<String> = (0..cat_count)
            .map(|i| {
                format!(
                    "{:.2},{:.2}",
                    cx + rr * angle(i).cos(),
                    cy + rr * angle(i).sin()
                )
            })
            .collect();
        let _ = write!(
            o,
            "<polygon points=\"{}\" fill=\"none\" stroke=\"#cbd5e1\" stroke-width=\"0.5\" opacity=\"1\"{}></polygon>",
            pts.join(" "),
            if r < 4 { " stroke-dasharray=\"3 2\"" } else { "" }
        );
    }
    for i in 0..cat_count {
        let a = angle(i);
        let _ = write!(
            o,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#94a3b8\" stroke-width=\"0.5\" opacity=\"1\"></line>",
            n(cx),
            n(cy),
            n(cx + radius * a.cos()),
            n(cy + radius * a.sin())
        );
    }
    let style = g.radar_style.as_deref().unwrap_or("marker");
    for (si, s) in series.iter().enumerate() {
        let c = s
            .color
            .clone()
            .unwrap_or_else(|| palette[si % palette.len()].clone());
        let pts: Vec<(f64, f64)> = s
            .values
            .iter()
            .take(cat_count)
            .enumerate()
            .map(|(i, v)| {
                let r = v.abs() / max_val * radius;
                (cx + r * angle(i).cos(), cy + r * angle(i).sin())
            })
            .collect();
        if pts.is_empty() {
            continue;
        }
        let ps: Vec<String> = pts
            .iter()
            .map(|p| format!("{:.2},{:.2}", p.0, p.1))
            .collect();
        let (fill, opacity) = match style {
            "standard" => ("none".to_string(), 1.0),
            "filled" => (c.clone(), 0.6),
            _ => (c.clone(), 0.2),
        };
        let _ = write!(
            o,
            "<polygon points=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\" opacity=\"{}\" data-chart-part=\"series\" data-chart-series=\"{si}\">{}</polygon>",
            ps.join(" "),
            fill,
            c,
            n(opacity),
            if s.name.is_empty() { String::new() } else { format!("<title>{}</title>", esc(&s.name)) }
        );
        if style == "marker" {
            for (vi, p) in pts.iter().enumerate() {
                let cat = categories.get(vi).map(String::as_str).unwrap_or("");
                let tip = tooltip(&s.name, cat, s.values.get(vi).copied().unwrap_or(0.0));
                let _ = write!(
                    o,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"3\" fill=\"{}\" opacity=\"1\" data-chart-part=\"dataPoint\" data-chart-series=\"{si}\" data-chart-point=\"{vi}\"><title>{}</title></circle>",
                    n(p.0),
                    n(p.1),
                    c,
                    esc(&tip)
                );
            }
        }
    }
}

/// `withAxisBounds` (chart-axis-range.ts), linear axes only.
fn with_axis_bounds(auto: Range, axis: &ChartAxis) -> Range {
    let valid = |v: Option<f64>| v.filter(|x| x.is_finite());
    let (vmin, vmax) = (valid(axis.scaling.min), valid(axis.scaling.max));
    let min = vmin.unwrap_or(auto.min);
    let mut max = vmax.unwrap_or(auto.max);
    let unit = if vmin.is_none() && vmax.is_none() {
        auto.unit
    } else {
        None
    };
    if max <= min {
        max = min + 1.0;
    }
    Range {
        min,
        max,
        span: (max - min).max(f64::EPSILON),
        unit,
        reverse: axis.scaling.orientation.as_deref() == Some("maxMin"),
    }
}

fn n(v: f64) -> String {
    format!("{v}")
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn tooltip(series: &str, cat: &str, val: f64) -> String {
    let label = [series, cat]
        .iter()
        .filter(|p| !p.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if label.is_empty() {
        fmt_axis(val)
    } else {
        format!("{label}: {}", fmt_axis(val))
    }
}

/// `estimateTextWidth` (text-wrap-estimate.ts).
fn estimate_text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|c| {
            let r = if ('\u{300}'..='\u{36f}').contains(&c) {
                0.0
            } else if c.is_whitespace() {
                0.33
            } else if matches!(c as u32, 0x3400..=0x9fff | 0x3040..=0x30ff | 0xac00..=0xd7af | 0x1f300..=0x1faff) {
                1.0
            } else if c.is_ascii_uppercase() {
                0.65
            } else if c.is_ascii_lowercase() || c.is_ascii_digit() {
                0.55
            } else if ",.;:!?()[]{}'\"`-_/\\".contains(c) {
                0.35
            } else {
                0.6
            };
            r * size
        })
        .sum()
}

fn fmt_axis(v: f64) -> String {
    if v.abs() >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v.abs() >= 1000.0 {
        format!("{:.1}K", v / 1000.0)
    } else if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.1}")
    }
}

struct Text<'a> {
    x: f64,
    y: f64,
    text: &'a str,
    anchor: &'a str,
    size: f64,
    fill: &'a str,
    bold: Option<bool>,
    family: &'a Option<String>,
    baseline: Option<&'a str>,
}
fn push_text(o: &mut String, t: &Text) {
    let _ = write!(
        o,
        "<text x=\"{}\" y=\"{}\" text-anchor=\"{}\" font-size=\"{}\" fill=\"{}\" font-weight=\"{}\" font-style=\"normal\"",
        n(t.x), n(t.y), t.anchor, n(t.size), t.fill,
        if t.bold == Some(true) { "bold" } else { "normal" }
    );
    if let Some(f) = t.family {
        let _ = write!(o, " font-family=\"{}\"", esc(f));
    }
    if let Some(b) = t.baseline {
        let _ = write!(o, " dominant-baseline=\"{b}\"");
    }
    let _ = write!(o, " opacity=\"1\">{}</text>", esc(t.text));
}
fn push_line(o: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, stroke: &str) {
    let _ = write!(
        o,
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{stroke}\" stroke-width=\"1\" opacity=\"1\"></line>",
        n(x1), n(y1), n(x2), n(y2)
    );
}

/// Value-axis tick (`buildVerticalTickMark`, left side): offsets from the axis x.
fn val_tick(placement: Option<&str>, len: f64) -> Option<(f64, f64)> {
    match placement {
        None | Some("none") => None,
        Some("cross") => Some((-len, len)),
        Some("out") => Some((0.0, -len)),
        _ => Some((0.0, len)),
    }
}

/// Category-axis tick (`tickLine`, bottom axis): offsets from the axis y.
fn cat_tick(placement: Option<&str>, len: f64, top: bool) -> Option<(f64, f64)> {
    match placement {
        None | Some("none") => None,
        Some("cross") => Some((-len, len)),
        Some("in") => Some((0.0, if top { len } else { -len })),
        _ => Some((0.0, if top { -len } else { len })),
    }
}

/// Legend text style after the chart-wide / legend `txPr` cascade.
struct LegendStyle {
    text: TextStyle,
}

/// `computeChartLegendLayout` + `appendChartLegendSvg`: swatch and label per `(color, label)` entry.
fn push_legend(
    o: &mut String,
    style: &LegendStyle,
    entries: &[(String, String, Option<String>)],
    (lx, ly, vertical): (f64, f64, bool),
) {
    let leg = &style.text;
    let have_style =
        leg.size_pt.is_some() || leg.color.is_some() || leg.family.is_some() || leg.bold.is_some();
    let size = if have_style {
        leg.size_pt.map(|p| p * PX_PER_PT).unwrap_or(9.0)
    } else {
        9.0
    };
    let fill = leg.color.clone().unwrap_or_else(|| "#475569".into());
    let fam = family_css(&leg.family);
    let cnt = entries.len() as f64;
    for (i, (color, label, swatch)) in entries.iter().enumerate() {
        let (x, y) = if vertical {
            (lx, ly + i as f64 * 14.0)
        } else {
            (lx - cnt * 80.0 / 2.0 + i as f64 * 80.0, ly)
        };
        let _ = write!(
            o,
            "<g class=\"pptxv-chart-legend-item\" transform=\"translate({x:.1},{y:.1})\">"
        );
        match swatch {
            Some(markup) => o.push_str(markup),
            None => {
                let _ = write!(
                    o,
                    "<rect x=\"0\" y=\"-7\" width=\"10\" height=\"10\" rx=\"2\" fill=\"{color}\"></rect>"
                );
            }
        }
        push_text(
            o,
            &Text {
                x: 13.0,
                y: 3.0,
                text: label,
                anchor: "start",
                size,
                fill: &fill,
                bold: leg.bold,
                family: &fam,
                baseline: None,
            },
        );
        o.push_str("</g>");
    }
}

/// Render the chart, or `None` when it uses a feature this port does not cover yet.
pub fn render(
    space: &ChartSpace,
    w: u32,
    h: u32,
    theme: &VmTheme,
    images: &HashMap<String, String>,
    user_shapes_xml: Option<&str>,
) -> Option<String> {
    let pa = &space.plot_area;
    if pa
        .groups
        .first()
        .is_some_and(|g| matches!(g.kind, ChartGroupKind::Pie | ChartGroupKind::Doughnut))
    {
        return pie::render(space, w, h, theme);
    }
    if space.user_shapes_rel_id.is_some() && user_shapes_xml.is_none() {
        return None;
    }
    if pa.groups.len() != 1
        || pa.data_table.is_some()
        // The DOM projector paints no plot-area border or fill rect, only the chart area.
        || pa.sp_pr
            .as_ref()
            .is_some_and(|p| !matches!(p.fill, None | Some(DrawingFill::None)))
        || space
            .sp_pr
            .as_ref()
            .is_some_and(|p| {
                !matches!(p.fill, None | Some(DrawingFill::None)) || p.effects_xml.is_some()
            })
        || !title_supported(space)
        || space.rounded_corners == Some(true)
    {
        return None;
    }
    let g = &pa.groups[0];
    let horizontal = g.bar_direction.as_deref() == Some("bar");
    let is_radar = g.kind == ChartGroupKind::Radar;
    let is_line = g.kind == ChartGroupKind::Line;
    let stacked = is_line && g.grouping.as_deref() == Some("stacked");
    if !(is_radar || is_line || g.kind == ChartGroupKind::Bar)
        || g.is3_d
        || !(if is_line {
            matches!(
                g.grouping.as_deref(),
                None | Some("standard") | Some("stacked")
            )
        } else {
            matches!(g.grouping.as_deref(), None | Some("clustered"))
        })
        || g.vary_colors == Some(true)
        || g.data_labels.as_ref().is_some_and(|d| {
            d.options.show_value == Some(true)
                || d.options.show_category_name == Some(true)
                || d.options.show_series_name == Some(true)
                || d.options.show_percent == Some(true)
        })
    {
        return None;
    }
    let cat_ax: &ChartAxis = pa.axes.iter().find(|a| a.kind == "cat")?;
    let val_ax: &ChartAxis = pa.axes.iter().find(|a| a.kind == "val")?;
    if pa.axes.len() != 2
        || (!is_radar
            && (false
                || cat_ax.deleted == Some(true)
                || val_ax.deleted == Some(true)
                || val_ax.position.as_deref() == Some("r")
                || cat_ax.scaling.orientation.as_deref() == Some("maxMin")
                || val_ax.scaling.log_base.is_some()
                || val_ax.display_units.is_some()
                || val_ax
                    .number_format
                    .as_ref()
                    .is_some_and(|f| !f.format_code.eq_ignore_ascii_case("general"))
                || val_ax
                    .minor_tick_mark
                    .as_deref()
                    .is_some_and(|m| m != "none")
                || cat_ax
                    .minor_tick_mark
                    .as_deref()
                    .is_some_and(|m| m != "none")
                || [cat_ax, val_ax].iter().any(|a| {
                    a.sp_pr.is_some()
                        || a.major_gridlines_sp_pr.is_some()
                        || a.title.is_some()
                        || a.crosses_at.is_some()
                        || a.tick_label_skip.is_some()
                        || a.tick_mark_skip.is_some()
                        || matches!(a.tick_label_position.as_deref(), Some("high") | Some("low"))
                        || matches!(a.crosses.as_deref(), Some("max") | Some("min"))
                })
                || (horizontal && val_ax.major_unit.is_some())
                || val_ax.minor_unit.is_some()
                || val_ax.tick_label_position.as_deref() == Some("none")
                || cat_ax.tick_label_position.as_deref() == Some("none")
                || cat_ax.label_align.as_deref().is_some_and(|a| a != "ctr")))
    {
        return None;
    }
    if is_line
        && (space.style.is_some()
            || [cat_ax, val_ax].iter().any(|a| {
                a.major_tick_mark.as_deref().is_some_and(|m| m != "none")
                    || a.scaling.min.is_some()
                    || a.scaling.max.is_some()
            }))
    {
        return None;
    }
    if space.legend.as_ref().is_some_and(|l| {
        !l.entries.is_empty() || l.layout.as_ref().is_some_and(|x| x.manual.x.is_some())
    }) {
        return None;
    }

    let mut series = Vec::new();
    let mut line_series: Vec<line::LineSeries> = Vec::new();
    let mut categories: Vec<String> = Vec::new();
    for (i, s) in g.series.iter().enumerate() {
        let ls = if is_line {
            Some(line::parse_series(s, i, &theme.scheme)?)
        } else {
            None
        };
        if !is_line
            && (!s.data_points.is_empty() || s.data_labels.is_some() || s.picture_options.is_some())
        {
            return None;
        }
        let mut picture = None;
        let color = match s.sp_pr.as_ref().and_then(|p| p.fill.as_ref()) {
            _ if ls.is_some() => ls.as_ref().and_then(|l| l.color.clone()),
            None => None,
            Some(DrawingFill::Solid { color }) => Some(
                resolve_drawing_color(color, Some(&theme.scheme), None)
                    .filter(|r| r.alpha == 1.0)?
                    .hex,
            ),
            Some(DrawingFill::Picture { rel_id, opacity }) => {
                picture = Some((images.get(rel_id.as_deref()?)?.clone(), *opacity));
                None
            }
            Some(_) => return None,
        };
        // `a:ln/a:noFill` draws nothing, as an absent line does.
        if !is_line
            && s.sp_pr.as_ref().is_some_and(|p| {
                p.effects_xml.is_some()
                    || p.line.as_ref().is_some_and(|ln| {
                        ln.width_emu.is_some() || !matches!(ln.fill, None | Some(DrawingFill::None))
                    })
            })
        {
            return None;
        }
        let vals = nitro_xml_opc::chart::chart_source_values(s.values.as_ref());
        if is_line && vals.iter().any(|v| v.as_f64().is_none()) {
            return None;
        }
        let values: Vec<f64> = vals.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
        let cats = nitro_xml_opc::chart::chart_source_values(s.categories.as_ref());
        let _ = cats;
        if categories.is_empty() {
            categories = cache_texts(s.categories.as_ref());
        }
        let name =
            s.tx.as_ref()
                .and_then(|t| {
                    t.value.clone().or_else(|| t.text.clone()).or_else(|| {
                        nitro_xml_opc::chart::chart_source_values(t.reference.as_ref())
                            .first()
                            .and_then(|v| v.as_str().map(String::from))
                    })
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("Series {}", i + 1));
        series.push(Series {
            name,
            values,
            color,
            picture,
            invert: s.invert_if_negative == Some(true),
        });
        if let Some(l) = ls {
            line_series.push(l);
        }
    }
    if series.is_empty() {
        return None;
    }
    // withAutoTitle: a lone named series gets an automatic title (not ported).
    if series.len() == 1 && space.title.is_none() && space.auto_title_deleted != Some(true) {
        return None;
    }
    let longest = series.iter().map(|s| s.values.len()).max().unwrap_or(0);
    if categories.is_empty() {
        categories = (1..=longest).map(|i| i.to_string()).collect();
    }

    // Text styles (chart-wide txPr over theme minor font, then each level's own).
    let mut chart_style = TextStyle {
        family: theme.minor_latin.clone(),
        ..Default::default()
    };
    let space_rpr = def_rpr(space.tx_pr.as_ref(), &theme.scheme);
    if let Some(s) = &space_rpr {
        chart_style = merge(&chart_style, s);
    }
    let axis_style = |a: &ChartAxis| {
        let own = def_rpr(a.tx_pr.as_ref(), &theme.scheme).unwrap_or_default();
        merge(&chart_style, &own)
    };
    let cat_style = axis_style(cat_ax);
    let val_style = axis_style(val_ax);
    let px = |s: &TextStyle| s.size_pt.map(|p| p * PX_PER_PT).unwrap_or(DEFAULT_TEXT_PX);
    let (cat_px, val_px) = (px(&cat_style), px(&val_style));

    // computePlotLayout
    let (wf, hf) = (w.max(1) as f64, h.max(1) as f64);
    let offset = 4.0 + 8.0 * (cat_ax.label_offset.unwrap_or(100.0) / 100.0);
    let cat_top = cat_ax.position.as_deref() == Some("t");
    let widest_label = categories
        .iter()
        .map(|c| estimate_text_width(c, DEFAULT_TEXT_PX))
        .fold(0.0, f64::max);
    let mut l = Layout {
        w: wf,
        h: hf,
        left: if is_radar {
            8.0
        } else {
            8.0 + (if horizontal { widest_label + 4.0 } else { 0.0 }).max(40.0)
        },
        top: 8.0,
        right: wf - 8.0,
        bottom: if is_radar {
            hf - 8.0
        } else {
            hf - (24f64).max(offset + cat_px * 1.2)
        },
    };
    if cat_top {
        l.bottom = hf - 8.0;
        l.top += (24f64).max(offset + cat_px * 1.2);
    }
    // Title band (chartTitleBand at the 12px default title size).
    const TITLE_PX: f64 = 12.0;
    let title_baseline = 10.8 * PX_PER_PT + TITLE_PX * 0.95;
    let title = title_text(space);
    if space.title.is_some() {
        l.top += title_baseline + TITLE_PX * 0.25 + 3.6 * PX_PER_PT;
    }
    let legend = space.legend.as_ref();
    let legend_pos = legend
        .and_then(|x| x.position.clone())
        .unwrap_or_else(|| "b".into());
    let side = match legend_pos.as_str() {
        "tr" | "r" => 'r',
        "t" => 't',
        "l" => 'l',
        _ => 'b',
    };
    if legend.is_some() && legend_pos != "tr" {
        match side {
            'b' => l.bottom -= 20.0,
            't' => l.top += 20.0,
            'r' => l.right -= 80.0,
            _ => l.left += 80.0,
        }
    }
    let auto_ph = (l.bottom - l.top).max(1.0);
    let ph = if horizontal { l.pw() } else { auto_ph };
    // `c:plotArea/c:layout/c:manualLayout`: an `outer` target keeps the axis bands inside it.
    if let (false, Some(pl)) = (is_radar, pa.layout.as_ref()) {
        if let Some(r) = nitro_xml_render::chart::resolve_manual_layout_rect(
            Some(&pl.manual),
            wf,
            hf,
            nitro_xml_render::chart::ChartLayoutRect {
                x: l.left,
                y: l.top,
                width: l.right - l.left,
                height: l.bottom - l.top,
            },
        ) {
            l.left = r.x;
            l.top = r.y;
            l.right = r.x + r.width;
            l.bottom = r.y + r.height;
            if pl.manual.layout_target.as_deref() != Some("inner") {
                l.left += (if horizontal { widest_label + 4.0 } else { 0.0 }).max(40.0);
                l.bottom -= (24f64).max(offset + cat_px * 1.2) - 8.0;
            }
        }
    }

    // Value range
    let line_cat_count = categories.len().max(1);
    let stacked_cum = stacked.then(|| {
        let vals: Vec<Vec<f64>> = series.iter().map(|s| s.values.clone()).collect();
        line::stacked_cumulative(&vals, line_cat_count)
    });
    let all: Vec<f64> = if stacked {
        // computeStackedValueRange: per-category positive / negative totals, anchored at zero.
        let (mut hi, mut lo) = (0.0f64, 0.0f64);
        for ci in 0..line_cat_count {
            let (mut p, mut ng) = (0.0, 0.0);
            for s in &series {
                let v = s.values.get(ci).copied().unwrap_or(0.0);
                if v >= 0.0 {
                    p += v;
                } else {
                    ng += v;
                }
            }
            hi = hi.max(p);
            lo = lo.min(ng);
        }
        vec![lo, hi]
    } else {
        series
            .iter()
            .flat_map(|s| s.values.iter().copied())
            .collect()
    };
    let range = if all.is_empty() {
        Range {
            min: 0.0,
            max: 1.0,
            span: 1.0,
            unit: None,
            reverse: false,
        }
    } else {
        let lo = all.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = all.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let b = nitro_xml_render::chart::nice_value_axis_bounds(
            lo,
            hi,
            nitro_xml_render::chart::axis_target_intervals(ph),
            nitro_xml_render::chart::VALUE_AXIS_HEADROOM,
        );
        Range {
            min: b.min,
            max: b.max,
            span: (b.max - b.min).max(f64::EPSILON),
            unit: Some(b.major_unit),
            reverse: false,
        }
    };
    let range = with_axis_bounds(range, val_ax);

    let palette = style_palette(space.style, theme_accents(&theme.scheme));
    let mut o = String::new();
    let mut defs = String::new();
    let _ = write!(
        o,
        "<svg class=\"pptxv-chart-svg\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\" style=\"width: 100%; height: 100%; display: block;\">",
        n(l.w), n(l.h)
    );
    let defs_at = o.len();

    if let Some(t) = &title {
        let fam = family_css(&chart_style.family);
        let _ = write!(
            o,
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" font-weight=\"600\"",
            n(l.w / 2.0),
            n(title_baseline),
            n(TITLE_PX)
        );
        if let Some(f) = &fam {
            let _ = write!(o, " font-family=\"{}\"", esc(f));
        }
        let _ = write!(
            o,
            " fill=\"#1e293b\" data-chart-part=\"title\">{}</text>",
            esc(t)
        );
    }

    if is_radar {
        radar_body(&mut o, g, &series, &categories, &l, &palette);
    } else if !horizontal {
        // Value axis (buildPrimaryAxis / buildGridlinesAndLabels)
        let show_grid = val_ax.major_gridlines;
        // An authored `c:majorUnit` steps the ticks (generateAxisTicks); the range stays automatic.
        let tick_unit = val_ax
            .major_unit
            .filter(|u| u.is_finite() && *u > 0.0)
            .or(range.unit);
        let ticks: Vec<f64> = match tick_unit {
            Some(u) if u > 0.0 => {
                let tol = 1f64.max(range.min.abs()).max(range.max.abs()) * 1e-12;
                let first = ((range.min - tol) / u).ceil() as i64;
                let last = ((range.max + tol) / u).floor() as i64;
                (first..=last).map(|i| i as f64 * u).collect()
            }
            _ => (0..=5)
                .map(|i| range.min + range.span / 5.0 * i as f64)
                .collect(),
        };
        let mut labels = String::new();
        let val_fam = family_css(&val_style.family);
        let val_fill = val_style
            .color
            .clone()
            .unwrap_or_else(|| AXIS_LABEL_COLOR.into());
        let major_tick = val_ax.major_tick_mark.as_deref();
        for v in &ticks {
            let y = value_y(*v, &range, l.top, l.bottom);
            if show_grid {
                push_line(&mut o, l.left, y, l.right, y, GRIDLINE_COLOR);
            }
            if let Some((a, b)) = val_tick(major_tick, 4.0) {
                push_line(&mut o, l.left + a, y, l.left + b, y, AXIS_LABEL_COLOR);
            }
            push_text(
                &mut labels,
                &Text {
                    x: l.left - 4.0,
                    y,
                    text: &fmt_axis(*v),
                    anchor: "end",
                    size: val_px,
                    fill: &val_fill,
                    bold: val_style.bold,
                    family: &val_fam,
                    baseline: Some("central"),
                },
            );
        }
        o.push_str(&labels);
        if range.min < 0.0 && range.max > 0.0 {
            let y = value_y(0.0, &range, l.top, l.bottom);
            push_line(&mut o, l.left, y, l.right, y, "#94a3b8");
        }

        // Category labels (buildMultiLevelCategoryLabels) at the zero crossing
        let axis_y = if val_ax.crosses_at.is_some() || val_ax.crosses.as_deref() == Some("autoZero")
        {
            value_y(0f64.clamp(range.min, range.max), &range, l.top, l.bottom)
        } else {
            l.bottom
        };
        let cat_count = categories.len().max(1);
        let group_w = l.pw() / cat_count as f64;
        let cat_fam = family_css(&cat_style.family);
        let cat_fill = cat_style
            .color
            .clone()
            .unwrap_or_else(|| AXIS_LABEL_COLOR.into());
        for (i, c) in categories.iter().enumerate() {
            if c.is_empty() {
                continue;
            }
            let x = if is_line {
                l.left + l.pw() * i as f64 / (cat_count.max(2) - 1) as f64
            } else {
                l.left + group_w * (i as f64 + 0.5)
            };
            push_text(
                &mut o,
                &Text {
                    x,
                    y: if cat_top {
                        axis_y - offset - 0.2 * cat_px
                    } else {
                        axis_y + offset + 0.8 * cat_px
                    },
                    text: c,
                    anchor: "middle",
                    size: cat_px,
                    fill: &cat_fill,
                    bold: cat_style.bold,
                    family: &cat_fam,
                    baseline: None,
                },
            );
        }

        // Lines (buildLines): each series at its own (or running-sum) heights.
        if is_line {
            let esc_fn = |t: &str| esc(t);
            for (si, (s, ls)) in series.iter().zip(&line_series).enumerate() {
                let plot_vals = stacked_cum
                    .as_ref()
                    .map(|c| c[si].clone())
                    .unwrap_or_else(|| s.values.clone());
                let n_pts = line_cat_count;
                let xs: Vec<f64> = (0..plot_vals.len())
                    .map(|i| l.left + l.pw() * i as f64 / (n_pts.max(2) - 1) as f64)
                    .collect();
                let ys: Vec<f64> = plot_vals
                    .iter()
                    .map(|v| value_y(*v, &range, l.top, l.bottom))
                    .collect();
                let color = s
                    .color
                    .clone()
                    .unwrap_or_else(|| palette[si % palette.len()].clone());
                line::series_markup(
                    &mut o,
                    ls,
                    si,
                    &color,
                    &s.name,
                    &categories,
                    &s.values,
                    &xs,
                    &ys,
                    &esc_fn,
                    &fmt_axis,
                );
            }
        }
        // Bars (clustered)
        let gap = g.gap_width.unwrap_or(150.0) as i32;
        let overlap = g.overlap.unwrap_or(0.0) as i32;
        let geo = nitro_xml_render::chart::clustered_bar_geometry(
            group_w,
            series.len().max(1),
            gap,
            overlap,
        );
        let offs = (group_w - geo.cluster_width) / 2.0;
        for ci in 0..(if is_line { 0 } else { cat_count }) {
            for (si, s) in series.iter().enumerate() {
                let val = s.values.get(ci).copied().unwrap_or(0.0);
                let x = l.left + group_w * ci as f64 + offs + geo.step * si as f64;
                let zero_y = value_y(0.0, &range, l.top, l.bottom);
                let vy = value_y(val, &range, l.top, l.bottom);
                let base = s
                    .color
                    .clone()
                    .unwrap_or_else(|| palette[si % palette.len()].clone());
                let fill = if val < 0.0 && s.invert {
                    tint(&base, 0.5)
                } else {
                    base
                };
                let cat = categories.get(ci).map(String::as_str).unwrap_or("");
                let tip = [s.name.as_str(), cat]
                    .iter()
                    .filter(|p| !p.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ");
                let tip = if tip.is_empty() {
                    fmt_axis(val)
                } else {
                    format!("{tip}: {}", fmt_axis(val))
                };
                let (ry, rh) = (zero_y.min(vy), (zero_y - vy).abs().max(1.0));
                let (fill, opacity) = match &s.picture {
                    Some((href, alpha)) => {
                        // buildPictureFillPatternDef: a stretch pattern over the mark's own box.
                        let id = format!("{}-chart-dpt-pic-{si}-{ci}", theme.picture_id_prefix);
                        let _ = write!(
                            defs,
                            "<pattern id=\"{id}\" patternUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><image href=\"{}\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"></image></pattern>",
                            n(x), n(ry), n(geo.single_bar_width), n(rh), esc(href), n(geo.single_bar_width), n(rh)
                        );
                        (format!("url(#{id})"), alpha.unwrap_or(1.0))
                    }
                    None => (fill, 1.0),
                };
                let _ = write!(
                o,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" rx=\"1\" opacity=\"{}\" data-chart-part=\"dataPoint\" data-chart-series=\"{si}\" data-chart-point=\"{ci}\"><title>{}</title></rect>",
                n(x), n(ry), n(geo.single_bar_width), n(rh), fill, n(opacity), esc(&tip)
            );
            }
        }
        // Category tick marks
        if let Some((a, b)) = cat_tick(cat_ax.major_tick_mark.as_deref(), 4.0, cat_top) {
            for i in 0..cat_count {
                let x = l.left + group_w * (i as f64 + 0.5);
                push_line(&mut o, x, axis_y + a, x, axis_y + b, AXIS_LABEL_COLOR);
            }
        }
    } else {
        // buildHorizontalBarViewModel: categories down the left, values along the bottom.
        let x_of = |v: f64| {
            let ratio = (v - range.min) / range.span;
            if range.reverse {
                l.right - ratio * (l.right - l.left)
            } else {
                l.left + ratio * (l.right - l.left)
            }
        };
        let unit = range.unit.filter(|u| u.is_finite() && *u > 0.0);
        let ticks: Vec<f64> = match unit {
            Some(u)
                if ((range.max - range.min) / u).round() >= 1.0
                    && ((range.max - range.min) / u).round() <= 100.0 =>
            {
                let steps = ((range.max - range.min) / u).round() as usize;
                (0..=steps).map(|i| range.min + u * i as f64).collect()
            }
            _ => (0..=5)
                .map(|i| range.min + range.span / 5.0 * i as f64)
                .collect(),
        };
        let none: Option<String> = None;
        if val_ax.major_gridlines {
            for v in &ticks {
                let x = x_of(*v);
                push_line(&mut o, x, l.top, x, l.bottom, GRIDLINE_COLOR);
            }
        }
        for v in &ticks {
            push_text(
                &mut o,
                &Text {
                    x: x_of(*v),
                    y: l.bottom + 12.0,
                    text: &fmt_axis(*v),
                    anchor: "middle",
                    size: DEFAULT_TEXT_PX,
                    fill: AXIS_LABEL_COLOR,
                    bold: None,
                    family: &none,
                    baseline: None,
                },
            );
        }
        if range.min < 0.0 && range.max > 0.0 {
            let x = x_of(0.0);
            push_line(&mut o, x, l.top, x, l.bottom, "#94a3b8");
        }
        let cat_count = categories.len().max(1);
        let band = (l.bottom - l.top).max(1.0) / cat_count as f64;
        for (i, c) in categories.iter().enumerate() {
            push_text(
                &mut o,
                &Text {
                    x: l.left - 4.0,
                    y: l.top + band * (i as f64 + 0.5),
                    text: c,
                    anchor: "end",
                    size: DEFAULT_TEXT_PX,
                    fill: AXIS_LABEL_COLOR,
                    bold: None,
                    family: &none,
                    baseline: Some("central"),
                },
            );
        }
        let gap = g.gap_width.unwrap_or(150.0) as i32;
        let overlap = g.overlap.unwrap_or(0.0) as i32;
        let geo = nitro_xml_render::chart::clustered_bar_geometry(
            band,
            series.len().max(1),
            gap,
            overlap,
        );
        let offs = (band - geo.cluster_width) / 2.0;
        let zero_x = x_of(0.0);
        for ci in 0..cat_count {
            for (si, s) in series.iter().enumerate() {
                let val = s.values.get(ci).copied().unwrap_or(0.0);
                let vx = x_of(val);
                let base = s
                    .color
                    .clone()
                    .unwrap_or_else(|| palette[si % palette.len()].clone());
                let fill = if val < 0.0 && s.invert {
                    tint(&base, 0.5)
                } else {
                    base
                };
                let cat = categories.get(ci).map(String::as_str).unwrap_or("");
                let tip = tooltip(&s.name, cat, val);
                let _ = write!(
                    o,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" rx=\"1\" opacity=\"1\" data-chart-part=\"dataPoint\" data-chart-series=\"{si}\" data-chart-point=\"{ci}\"><title>{}</title></rect>",
                    n(zero_x.min(vx)),
                    n(l.top + band * ci as f64 + offs + geo.step * si as f64),
                    n((vx - zero_x).abs().max(1.0)),
                    n(geo.single_bar_width),
                    fill,
                    esc(&tip)
                );
            }
        }
    }

    // `c:userShapes` drawing overlay (layered after the marks, before the legend).
    if let Some(xml) = user_shapes_xml {
        o.push_str(&super::chart_user_shapes::overlay_svg(
            xml,
            l.w,
            l.h,
            &theme.scheme,
        ));
    }
    // Legend (buildLegend + computeChartLegendLayout)
    if legend.is_some() {
        let (lx, ly, vertical) = match side {
            'r' => (l.w - 75.0, l.top, true),
            'l' => (4.0, l.top, true),
            't' => (l.w / 2.0, 28.0, false),
            _ => (l.w / 2.0, l.h - 8.0, false),
        };
        let entries: Vec<(String, String, Option<String>)> = series
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let color = s
                    .color
                    .clone()
                    .unwrap_or_else(|| palette[i % palette.len()].clone());
                let swatch = line_series.get(i).map(|ls| line::legend_swatch(ls, &color));
                (color, s.name.clone(), swatch)
            })
            .collect();
        let leg_rpr = legend.and_then(|x| def_rpr(x.tx_pr.as_ref(), &theme.scheme));
        push_legend(
            &mut o,
            &LegendStyle {
                text: merge(&chart_style, &leg_rpr.or(space_rpr).unwrap_or_default()),
            },
            &entries,
            (lx, ly, vertical),
        );
    }
    if !defs.is_empty() {
        o.insert_str(defs_at, &format!("<defs>{defs}</defs>"));
    }
    o.push_str("</svg>");
    let _ = THEME_SLOTS;
    Some(o)
}
