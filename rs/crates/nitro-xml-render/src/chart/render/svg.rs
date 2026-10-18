// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/chart/render/chart-svg.ts

use super::chart_auto_title::resolve_p9_chart_title_text;
use super::chart_plot_layout::{
    chart_title_baseline_y, chart_title_reserve_px, compute_plot_layout, DEFAULT_CHART_TEXT_PX,
};
use super::chart_user_shape_overlay::build_chart_user_shape_overlay_svg;
use super::chart_view::ChartViewModel;
use super::summary::ChartSummaryType;
use super::svg_appearance::{chart_area_rect, chart_gradient_paint, chart_text_attributes};
use super::svg_cartesian::cartesian_svg;
use super::svg_legend::chart_legend_svg;
use super::svg_radial::{pie_svg, radar_svg};
use super::svg_text_metrics::{
    chart_font_metrics, chart_text_width, fit_chart_text, ChartSvgMeasureOptions, CHART_FONT_FAMILY,
};
use super::svg_title_text::chart_rich_title_svg;
use super::svg_util::{esc, n, text, Rect, TextAttrs};
use nitro_xml_opc::chart::ChartUserShape;

/// Upstream `DEFAULT_TITLE_TEXT_PX` / `DEFAULT_TITLE_FILL` for auto titles
/// (`chart-title-style.ts`) — not built-in style 102's 18pt.
const AUTO_TITLE_TEXT_PX: f64 = 12.0;
const AUTO_TITLE_FILL: &str = "#1e293b";
const AUTO_TITLE_WEIGHT: &str = "600";

fn p9_title_paint_attrs(model: &ChartViewModel) -> TextAttrs {
    let mut attrs = chart_text_attributes(model, "title", AUTO_TITLE_TEXT_PX);
    if model.auto_title {
        attrs.size = Some(AUTO_TITLE_TEXT_PX);
        attrs.fill = Some(AUTO_TITLE_FILL.to_string());
        attrs.weight = Some(AUTO_TITLE_WEIGHT);
    }
    attrs
}

pub fn render_chart_svg(
    model: &ChartViewModel,
    width: u32,
    height: u32,
    measure: &mut ChartSvgMeasureOptions<'_>,
    user_shapes: Option<&[ChartUserShape]>,
) -> String {
    let p9_bar3d = model.is3_d
        && model.bar3d_extrusion
        && matches!(
            model.type_,
            ChartSummaryType::Bar | ChartSummaryType::Column
        );
    let paint_model = if p9_bar3d {
        let mut m = model.clone();
        if let Some(title) = resolve_p9_chart_title_text(model) {
            m.title = Some(title);
        }
        m
    } else {
        model.clone()
    };
    let (model, defs) = chart_gradient_paint(&paint_model, width.max(1), height.max(1));
    let mut picture_defs: Vec<String> = Vec::new();
    let w = width.max(1) as f64;
    let h = height.max(1) as f64;
    let mut out = Vec::new();
    let mut foreground_title = String::new();
    out.push(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\" font-family=\"{}\" role=\"img\">",
        n(w),
        n(h),
        n(w),
        n(h),
        esc(CHART_FONT_FAMILY)
    ));
    if let Some(title) = &model.title {
        out.push(format!("<title>{}</title>", esc(title)));
    }
    if !defs.is_empty() {
        out.push(defs);
    }
    out.push(chart_area_rect(
        &model,
        "chartArea",
        Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
    ));
    let mut area = Rect {
        x: 8.0,
        y: 8.0,
        w: w - 16.0,
        h: h - 16.0,
    };
    if let Some(rich) = chart_rich_title_svg(&model, width, measure, height) {
        if model.title_overlay {
            foreground_title = rich.markup;
        } else {
            out.push(rich.markup);
        }
        if !model.title_overlay && !p9_bar3d {
            area.y += rich.height;
            area.h -= rich.height;
        }
    } else if let Some(title) = &model.title {
        let attrs = if p9_bar3d {
            p9_title_paint_attrs(&model)
        } else {
            chart_text_attributes(&model, "title", 14.0)
        };
        let size = attrs
            .size
            .unwrap_or(if p9_bar3d { AUTO_TITLE_TEXT_PX } else { 14.0 });
        let label = fit_chart_text(title, w - 16.0, &attrs, measure);
        let title_width = (w - 16.0).min(chart_text_width(&label, &attrs, measure) + 8.0);
        let title_height = if let Some((a, d)) = chart_font_metrics(&attrs, measure) {
            a + d + 4.0
        } else {
            size * 1.4 + 4.0
        }
        .max(24.0);
        out.push(chart_area_rect(
            &model,
            "title",
            Rect {
                x: (w - title_width) / 2.0,
                y: 4.0,
                w: title_width.max(0.0),
                h: if chart_font_metrics(&attrs, measure).is_some() {
                    chart_font_metrics(&attrs, measure)
                        .map(|(a, d)| a + d + 4.0)
                        .unwrap_or(title_height)
                } else {
                    size * 1.4 + 4.0
                },
            },
        ));
        let mut title_attrs = attrs.clone();
        title_attrs.anchor = Some("middle");
        if title_attrs.fill.is_none() {
            title_attrs.fill = Some("#404040".to_string());
        }
        let title_baseline_y = if p9_bar3d {
            chart_title_baseline_y(size)
        } else {
            8.0 + size
        };
        out.push(text(w / 2.0, title_baseline_y, &label, &title_attrs));
        if !model.title_overlay && !p9_bar3d {
            area.y += title_height;
            area.h -= title_height;
        }
    }
    if !model.supported {
        out.push(text(
            w / 2.0,
            area.y + area.h / 2.0,
            &format!("{} charts are not drawn", model.type_.as_str()),
            &TextAttrs {
                anchor: Some("middle"),
                ..Default::default()
            },
        ));
        if !foreground_title.is_empty() {
            out.push(foreground_title);
        }
        out.push("</svg>".to_string());
        return out.join("");
    }
    let plot_layout = if p9_bar3d {
        let title_attrs = p9_title_paint_attrs(&model);
        let title_px = title_attrs.size.unwrap_or(AUTO_TITLE_TEXT_PX);
        let cat_attrs = chart_text_attributes(&model, "categoryAxis", 10.0);
        let cat_px = cat_attrs.size.unwrap_or(DEFAULT_CHART_TEXT_PX);
        Some(compute_plot_layout(
            w,
            h,
            chart_title_reserve_px(model.title.is_some() || model.title_placeholder, title_px),
            model.show_legend,
            &model.legend_position,
            model.category_label_offset,
            cat_px,
        ))
    } else {
        None
    };
    let legend_area = if p9_bar3d {
        Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        }
    } else {
        area
    };
    let mut legend_model = model.clone();
    if p9_bar3d {
        legend_model.legend_overlay = true;
    }
    let legend = chart_legend_svg(&legend_model, legend_area, w, h, measure);
    area = if p9_bar3d { legend_area } else { legend.plot };
    let legend_markup = legend.markup;
    if !legend.foreground {
        out.push(legend_markup.clone());
    }
    if area.w > 4.0 && area.h > 4.0 {
        let body = match model.type_ {
            ChartSummaryType::Pie | ChartSummaryType::Doughnut => pie_svg(&model, area),
            ChartSummaryType::Radar => radar_svg(&model, area),
            _ => cartesian_svg(&model, area, &mut picture_defs, plot_layout),
        };
        out.push(body);
        if let Some(shapes) = user_shapes {
            out.push(build_chart_user_shape_overlay_svg(
                Some(shapes),
                width,
                height,
            ));
        }
    }
    if legend.foreground {
        out.push(legend_markup);
    }
    if !foreground_title.is_empty() {
        out.push(foreground_title);
    }
    if !picture_defs.is_empty() {
        out.push(format!("<defs>{}</defs>", picture_defs.join("")));
    }
    out.push("</svg>".to_string());
    out.join("")
}
