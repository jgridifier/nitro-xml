// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-cartesian.ts,
// chart-cartesian-bars.ts, chart-view-model-layout.ts, chart-view-model-chrome.ts,
// chart-axis-render.ts, chart-category-axis.ts, chart-legend-build.ts, chart-legend-layout.ts,
// chart-style-palette.ts and chart-view-model-dom.ts (clustered column subset);
// changes listed in PROVENANCE.md.
//! pptx chart view-model engine (flat clustered columns). `render` returns `None` for any chart
//! feature not yet ported so the caller falls back to the S1 SVG painter.

use std::collections::HashMap;
use std::fmt::Write as _;

use nitro_xml_opc::chart::{ChartAxis, ChartGroupKind, ChartSpace, ChartTextBody};
use nitro_xml_opc::drawingml::{resolve_drawing_color, DrawingFill};
use nitro_xml_render::chart::THEME_SLOTS;

use super::font_substitution::substitute_font_family_stack;

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
}
fn value_y(v: f64, r: &Range, top: f64, bottom: f64) -> f64 {
    bottom - (v - r.min) / r.span * (bottom - top)
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
fn cat_tick(placement: Option<&str>, len: f64) -> Option<(f64, f64)> {
    match placement {
        None | Some("none") => None,
        Some("cross") => Some((-len, len)),
        Some("in") => Some((0.0, -len)),
        _ => Some((0.0, len)),
    }
}

/// Render the chart, or `None` when it uses a feature this port does not cover yet.
pub fn render(space: &ChartSpace, w: u32, h: u32, theme: &VmTheme) -> Option<String> {
    let pa = &space.plot_area;
    if pa.groups.len() != 1
        || pa
            .layout
            .as_ref()
            .is_some_and(|l| l.manual.x.is_some() || l.manual.width.is_some())
        || pa.data_table.is_some()
        || pa.sp_pr.is_some()
        || space.sp_pr.is_some()
        || space.title.is_some()
        || space.rounded_corners == Some(true)
    {
        return None;
    }
    let g = &pa.groups[0];
    if g.kind != ChartGroupKind::Bar
        || g.is3_d
        || g.bar_direction.as_deref() == Some("bar")
        || !matches!(g.grouping.as_deref(), None | Some("clustered"))
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
        || cat_ax.deleted == Some(true)
        || val_ax.deleted == Some(true)
        || cat_ax.position.as_deref() == Some("t")
        || val_ax.position.as_deref() == Some("r")
        || cat_ax.scaling.orientation.as_deref() == Some("maxMin")
        || val_ax.scaling.orientation.as_deref() == Some("maxMin")
        || val_ax.scaling.log_base.is_some()
        || val_ax.display_units.is_some()
        || val_ax
            .number_format
            .as_ref()
            .is_some_and(|f| !f.format_code.eq_ignore_ascii_case("general"))
        || val_ax.minor_gridlines
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
        || val_ax.major_unit.is_some()
        || val_ax.minor_unit.is_some()
        || val_ax.scaling.min.is_some()
        || val_ax.scaling.max.is_some()
        || val_ax.tick_label_position.as_deref() == Some("none")
        || cat_ax.tick_label_position.as_deref() == Some("none")
        || cat_ax.label_align.as_deref().is_some_and(|a| a != "ctr")
    {
        return None;
    }
    if space.legend.as_ref().is_some_and(|l| {
        !l.entries.is_empty() || l.layout.as_ref().is_some_and(|x| x.manual.x.is_some())
    }) {
        return None;
    }

    let mut series = Vec::new();
    let mut categories: Vec<String> = Vec::new();
    for (i, s) in g.series.iter().enumerate() {
        if !s.data_points.is_empty() || s.data_labels.is_some() || s.picture_options.is_some() {
            return None;
        }
        let color = match s.sp_pr.as_ref().and_then(|p| p.fill.as_ref()) {
            None => None,
            Some(DrawingFill::Solid { color }) => Some(
                resolve_drawing_color(color, Some(&theme.scheme), None)
                    .filter(|r| r.alpha == 1.0)?
                    .hex,
            ),
            Some(_) => return None,
        };
        if s.sp_pr
            .as_ref()
            .is_some_and(|p| p.line.is_some() || p.effects_xml.is_some())
        {
            return None;
        }
        let vals = nitro_xml_opc::chart::chart_source_values(s.values.as_ref());
        let values: Vec<f64> = vals.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
        let cats = nitro_xml_opc::chart::chart_source_values(s.categories.as_ref());
        if categories.is_empty() {
            categories = cats
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(t) => t.clone(),
                    serde_json::Value::Number(x) => x.to_string(),
                    _ => String::new(),
                })
                .collect();
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
            invert: s.invert_if_negative == Some(true),
        });
    }
    if series.is_empty() {
        return None;
    }
    // withAutoTitle: a lone named series gets an automatic title (not ported).
    if series.len() == 1 && space.auto_title_deleted != Some(true) {
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
    let mut l = Layout {
        w: wf,
        h: hf,
        left: 48.0,
        top: 8.0,
        right: wf - 8.0,
        bottom: hf - (24f64).max(offset + cat_px * 1.2),
    };
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
    let ph = auto_ph;

    // Value range
    let all: Vec<f64> = series
        .iter()
        .flat_map(|s| s.values.iter().copied())
        .collect();
    let range = if all.is_empty() {
        Range {
            min: 0.0,
            max: 1.0,
            span: 1.0,
            unit: None,
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
        }
    };

    let palette = style_palette(space.style, theme_accents(&theme.scheme));
    let mut o = String::new();
    let _ = write!(
        o,
        "<svg class=\"pptxv-chart-svg\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\" style=\"width: 100%; height: 100%; display: block;\">",
        n(l.w), n(l.h)
    );

    // Value axis (buildPrimaryAxis / buildGridlinesAndLabels)
    let show_grid = val_ax.major_gridlines;
    let ticks: Vec<f64> = match range.unit {
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
    let axis_y = if val_ax.crosses_at.is_some() || val_ax.crosses.as_deref() == Some("autoZero") {
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
        let x = l.left + group_w * (i as f64 + 0.5);
        push_text(
            &mut o,
            &Text {
                x,
                y: axis_y + offset + 0.8 * cat_px,
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

    // Bars (clustered)
    let gap = g.gap_width.unwrap_or(150.0) as i32;
    let overlap = g.overlap.unwrap_or(0.0) as i32;
    let geo =
        nitro_xml_render::chart::clustered_bar_geometry(group_w, series.len().max(1), gap, overlap);
    let offs = (group_w - geo.cluster_width) / 2.0;
    for ci in 0..cat_count {
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
            let _ = write!(
                o,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" rx=\"1\" opacity=\"1\" data-chart-part=\"dataPoint\" data-chart-series=\"{si}\" data-chart-point=\"{ci}\"><title>{}</title></rect>",
                n(x), n(zero_y.min(vy)), n(geo.single_bar_width), n((zero_y - vy).abs().max(1.0)), fill, esc(&tip)
            );
        }
    }
    // Category tick marks
    if let Some((a, b)) = cat_tick(cat_ax.major_tick_mark.as_deref(), 4.0) {
        for i in 0..cat_count {
            let x = l.left + group_w * (i as f64 + 0.5);
            push_line(&mut o, x, axis_y + a, x, axis_y + b, AXIS_LABEL_COLOR);
        }
    }

    // Legend (buildLegend + computeChartLegendLayout)
    if legend.is_some() {
        let (lx, ly, vertical) = match side {
            'r' => (l.w - 75.0, l.top, true),
            'l' => (4.0, l.top, true),
            't' => (l.w / 2.0, 28.0, false),
            _ => (l.w / 2.0, l.h - 8.0, false),
        };
        let leg_rpr = legend.and_then(|x| def_rpr(x.tx_pr.as_ref(), &theme.scheme));
        let leg = merge(&chart_style, &leg_rpr.or(space_rpr).unwrap_or_default());
        let have_style = leg.size_pt.is_some()
            || leg.color.is_some()
            || leg.family.is_some()
            || leg.bold.is_some();
        let size = if have_style {
            leg.size_pt.map(|p| p * PX_PER_PT).unwrap_or(9.0)
        } else {
            9.0
        };
        let fill = leg.color.clone().unwrap_or_else(|| "#475569".into());
        let fam = family_css(&leg.family);
        let cnt = series.len() as f64;
        for (i, s) in series.iter().enumerate() {
            let (x, y) = if vertical {
                (lx, ly + i as f64 * 14.0)
            } else {
                (lx - cnt * 80.0 / 2.0 + i as f64 * 80.0, ly)
            };
            let color = s
                .color
                .clone()
                .unwrap_or_else(|| palette[i % palette.len()].clone());
            let _ = write!(
                o,
                "<g class=\"pptxv-chart-legend-item\" transform=\"translate({x:.1},{y:.1})\"><rect x=\"0\" y=\"-7\" width=\"10\" height=\"10\" rx=\"2\" fill=\"{color}\"></rect>"
            );
            push_text(
                &mut o,
                &Text {
                    x: 13.0,
                    y: 3.0,
                    text: &s.name,
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
    o.push_str("</svg>");
    let _ = THEME_SLOTS;
    Some(o)
}
