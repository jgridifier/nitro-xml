// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-view-model-pie.ts,
// chart-view-model-points.ts (computePieLayout, computePieSlices, computePieSlicePath),
// chart-title-band.ts and chart-view-model-dom.ts (pie / doughnut subset);
// changes listed in PROVENANCE.md.
//! pptx pie / doughnut view-model (`buildPieViewModel` + the DOM projector). `render` returns
//! `None` for any chart feature not yet ported (data labels, per-point fills, explosion, manual
//! layouts) so the caller falls back to the S1 SVG painter.

use std::fmt::Write as _;

use nitro_xml_opc::chart::{ChartGroupKind, ChartSpace};

use super::{
    cache_texts, def_rpr, esc, family_css, fmt_axis, merge, push_legend, style_palette,
    theme_accents, title_supported, title_text, LegendStyle, TextStyle, VmTheme, PX_PER_PT,
};

/// `PIE_AUTO_INSET_PX`: constant padding PowerPoint leaves inside an automatic pie plot area.
const PIE_AUTO_INSET_PX: f64 = 6.7;
/// The title is drawn at the viewer default (12 px); authored title formatting is not ported.
const TITLE_PX: f64 = 12.0;

/// ECMAScript `Number::toString` for a finite number (what a JS template literal emits).
fn js_num(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if !v.is_finite() {
        return if v.is_nan() {
            "NaN"
        } else if v > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        }
        .into();
    }
    let sci = format!("{:e}", v.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp.parse::<i32>().unwrap_or(0) + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e < 0 { '-' } else { '+' };
        if k == 1 {
            format!("{digits}e{sign}{}", e.abs())
        } else {
            format!("{}.{}e{sign}{}", &digits[..1], &digits[1..], e.abs())
        }
    };
    if v < 0.0 {
        format!("-{body}")
    } else {
        body
    }
}

struct PieLayout {
    cx: f64,
    cy: f64,
    outer_r: f64,
    inner_r: f64,
    size: f64,
}

/// `chartTitleBand(12).bandHeight` (the reservation `chartTitleReservePx` makes for a title).
fn title_band() -> (f64, f64) {
    let baseline = 10.8 * PX_PER_PT + TITLE_PX * 0.95;
    (baseline, baseline + TITLE_PX * 0.25 + 3.6 * PX_PER_PT)
}

/// `computePieLayout` without a manual plot-area layout (those charts fall back to S1).
fn compute_pie_layout(
    w: f64,
    h: f64,
    has_title: bool,
    has_legend: bool,
    hole: Option<f64>,
    doughnut: bool,
) -> PieLayout {
    let size = w.max(1.0).min(h.max(1.0));
    let title_offset = if has_title { title_band().1 } else { 0.0 };
    let legend_offset = if has_legend { 20.0 } else { 0.0 };
    let cx = size / 2.0;
    let cy = title_offset + (size - title_offset - legend_offset) / 2.0;
    let outer_r = ((size - title_offset - legend_offset) * 0.42 - PIE_AUTO_INSET_PX).max(0.0);
    let hole_ratio = match hole {
        Some(hs) if doughnut => hs.clamp(10.0, 90.0) / 100.0,
        _ => 0.55,
    };
    PieLayout {
        cx,
        cy,
        outer_r,
        inner_r: if doughnut { outer_r * hole_ratio } else { 0.0 },
        size,
    }
}

/// `computePieSlices` + `computePieSlicePath` (no explosion): one SVG path `d` per slice.
fn compute_pie_slices(values: &[f64], l: &PieLayout, start_angle: f64) -> Vec<String> {
    let total = {
        let t: f64 = values.iter().map(|v| v.abs()).sum();
        if t == 0.0 {
            1.0
        } else {
            t
        }
    };
    let mut cum = start_angle;
    values
        .iter()
        .map(|val| {
            let slice = (val.abs() / total) * std::f64::consts::PI * 2.0;
            let start = cum;
            cum += slice;
            let end = cum;
            let large_arc = if end - start > std::f64::consts::PI {
                1
            } else {
                0
            };
            let (cx, cy, outer_r, inner_r) = (l.cx, l.cy, l.outer_r, l.inner_r);
            let x1 = cx + outer_r * start.cos();
            let y1 = cy + outer_r * start.sin();
            let x2 = cx + outer_r * end.cos();
            let y2 = cy + outer_r * end.sin();
            if inner_r > 0.0 {
                let ix1 = cx + inner_r * start.cos();
                let iy1 = cy + inner_r * start.sin();
                let ix2 = cx + inner_r * end.cos();
                let iy2 = cy + inner_r * end.sin();
                format!(
                    "M{},{} A{},{} 0 {large_arc} 1 {},{} L{},{} A{},{} 0 {large_arc} 0 {},{} Z",
                    js_num(x1),
                    js_num(y1),
                    js_num(outer_r),
                    js_num(outer_r),
                    js_num(x2),
                    js_num(y2),
                    js_num(ix2),
                    js_num(iy2),
                    js_num(inner_r),
                    js_num(inner_r),
                    js_num(ix1),
                    js_num(iy1),
                )
            } else {
                format!(
                    "M{},{} L{},{} A{},{} 0 {large_arc} 1 {},{} Z",
                    js_num(cx),
                    js_num(cy),
                    js_num(x1),
                    js_num(y1),
                    js_num(outer_r),
                    js_num(outer_r),
                    js_num(x2),
                    js_num(y2),
                )
            }
        })
        .collect()
}

/// Render a pie or doughnut, or `None` when it uses a feature this port does not cover yet.
pub(super) fn render(space: &ChartSpace, w: u32, h: u32, theme: &VmTheme) -> Option<String> {
    let pa = &space.plot_area;
    let g = pa.groups.first()?;
    let doughnut = g.kind == ChartGroupKind::Doughnut;
    if pa.groups.len() != 1
        || !matches!(g.kind, ChartGroupKind::Pie | ChartGroupKind::Doughnut)
        || g.is3_d
        || pa.layout.as_ref().is_some_and(|l| {
            l.manual.x.is_some()
                || l.manual.y.is_some()
                || l.manual.width.is_some()
                || l.manual.height.is_some()
        })
        || pa.data_table.is_some()
        || pa.sp_pr.as_ref().is_some_and(|p| p.fill.is_some())
        || space
            .sp_pr
            .as_ref()
            .is_some_and(|p| p.fill.is_some() || p.effects_xml.is_some())
        || !title_supported(space)
        || space.rounded_corners == Some(true)
        || g.series.len() != 1
        || g.data_labels.as_ref().is_some_and(|d| {
            d.options.show_value == Some(true)
                || d.options.show_category_name == Some(true)
                || d.options.show_series_name == Some(true)
                || d.options.show_percent == Some(true)
        })
        || space.legend.as_ref().is_some_and(|l| {
            !l.entries.is_empty() || l.layout.as_ref().is_some_and(|x| x.manual.x.is_some())
        })
    {
        return None;
    }
    // withAutoTitle: a title-less lone series is auto-titled with its name (not ported).
    if space.title.is_none() && space.auto_title_deleted != Some(true) {
        return None;
    }
    let s = &g.series[0];
    if !s.data_points.is_empty()
        || s.data_labels.is_some()
        || s.picture_options.is_some()
        || s.explosion.is_some_and(|e| e != 0.0)
        || s.sp_pr.as_ref().is_some_and(|p| p.fill.is_some())
    {
        return None;
    }
    let vals_src = s.values.as_ref();
    if vals_src.is_some_and(|v| {
        v.cache
            .as_ref()
            .and_then(|c| c.format_code.as_deref())
            .is_some_and(|f| !f.eq_ignore_ascii_case("general"))
    }) {
        return None;
    }
    let values: Vec<f64> = nitro_xml_opc::chart::chart_source_values(vals_src)
        .iter()
        .map(|v| v.as_f64().unwrap_or(0.0))
        .collect();
    let series_name =
        s.tx.as_ref()
            .and_then(|t| {
                t.value.clone().or_else(|| t.text.clone()).or_else(|| {
                    nitro_xml_opc::chart::chart_source_values(t.reference.as_ref())
                        .first()
                        .and_then(|v| v.as_str().map(String::from))
                })
            })
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Series 1".to_string());
    let mut categories = cache_texts(s.categories.as_ref());
    if categories.is_empty() {
        categories = (1..=values.len()).map(|i| i.to_string()).collect();
    }

    let has_title = space.title.is_some();
    let has_legend = space.legend.is_some();
    let layout = compute_pie_layout(
        w as f64,
        h as f64,
        has_title,
        has_legend,
        g.hole_size,
        doughnut,
    );
    let svg_w = layout.size.max(100.0);
    let svg_h = layout.size.max(60.0);
    let start_angle = -std::f64::consts::FRAC_PI_2
        + g.first_slice_angle.unwrap_or(0.0) * std::f64::consts::PI / 180.0;
    let slices = compute_pie_slices(&values, &layout, start_angle);
    let palette = style_palette(space.style, theme_accents(&theme.scheme));

    let mut chart_style = TextStyle {
        family: theme.minor_latin.clone(),
        ..Default::default()
    };
    let space_rpr = def_rpr(space.tx_pr.as_ref(), &theme.scheme);
    if let Some(r) = &space_rpr {
        chart_style = merge(&chart_style, r);
    }

    let mut o = String::new();
    let _ = write!(
        o,
        "<svg class=\"pptxv-chart-svg\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"xMidYMid meet\" style=\"width: 100%; height: 100%; display: block;\">",
        js_num(svg_w),
        js_num(svg_h)
    );
    let title = title_text(space);
    if let Some(t) = &title {
        let fam = family_css(&chart_style.family);
        let _ = write!(
            o,
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" font-weight=\"600\"",
            js_num(svg_w / 2.0),
            js_num(title_band().0),
            js_num(TITLE_PX)
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
    for (i, d) in slices.iter().enumerate() {
        let tip = {
            let label = [
                series_name.as_str(),
                categories.get(i).map_or("", String::as_str),
            ]
            .iter()
            .filter(|p| !p.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
            let val = fmt_axis(values.get(i).copied().unwrap_or(0.0));
            if label.is_empty() {
                val
            } else {
                format!("{label}: {val}")
            }
        };
        let _ = write!(
            o,
            "<path d=\"{d}\" fill=\"{}\" stroke=\"#ffffff\" stroke-width=\"1.5\" fill-opacity=\"1\" data-chart-part=\"dataPoint\" data-chart-series=\"0\" data-chart-point=\"{i}\"><title>{}</title></path>",
            palette[i % palette.len()],
            esc(&tip)
        );
    }

    if let Some(legend) = &space.legend {
        // `legendPos` r / l / b all use the bottom-centred row; `t` and `tr` move it.
        let pos = legend.position.as_deref().unwrap_or("b");
        let (lx, ly, vertical) = match pos {
            "t" => (
                svg_w / 2.0,
                if has_title {
                    24.0 + (title_band().1 - 20.0)
                } else {
                    8.0
                },
                false,
            ),
            "tr" => (
                svg_w - 75.0,
                if has_title { title_band().1 + 4.0 } else { 8.0 },
                true,
            ),
            _ => (svg_w / 2.0, svg_h - 8.0, false),
        };
        let entries: Vec<(String, String)> = categories
            .iter()
            .enumerate()
            .map(|(i, c)| (palette[i % palette.len()].clone(), c.clone()))
            .collect();
        let leg_rpr = def_rpr(legend.tx_pr.as_ref(), &theme.scheme);
        push_legend(
            &mut o,
            &LegendStyle {
                text: merge(&chart_style, &leg_rpr.or(space_rpr).unwrap_or_default()),
            },
            &entries,
            (lx, ly, vertical),
        );
    }
    o.push_str("</svg>");
    Some(o)
}
