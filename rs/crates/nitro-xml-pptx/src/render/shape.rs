// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/text-shape.ts; changes in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub, set_style};
use super::prst_geom::{apply_preset_geometry, preset_stroke_outline_path};
use super::shape_autofit_resize::queue_sp_autofit_scroll_height;
use super::stroke_dash::{css_border_dash_style, svg_stroke_dasharray};
use super::text::{paint_text_body, TextBodyPaint};
use crate::slide::DumpElement;

pub fn paint<I: StringIntern>(ctx: &mut PaintContext<'_, I>, el: &DumpElement, parent: u32) {
    let painted_h = ctx.sp_autofit.painted_height(&el.id, el.height);
    let id = create_child(ctx, parent, "div");
    position_box(ctx, id, el.x, el.y, el.width, painted_h);
    set_attr(ctx, id, "data-pptx-element", "true");
    set_attr(ctx, id, "data-element-id", &el.id);
    set_attr(ctx, id, "data-pptx-kind", "shape");
    if let Some(name) = &el.name {
        set_attr(ctx, id, "data-pptx-name", name);
    }
    if let Some(src) = el.image_src.as_deref() {
        set_style(ctx, id, "background-image", &format!("url({src})"));
        set_style(ctx, id, "background-size", "100% 100%");
        set_style(ctx, id, "background-repeat", "no-repeat");
    } else {
        let fill_bg = match el.fill_mode.as_deref() {
            Some("none") => None,
            Some("gradient") => el.fill_gradient.clone(),
            _ => None,
        };
        if let Some(bg) =
            compose_background_image(el.material_overlay.as_deref(), fill_bg.as_deref())
        {
            set_style(ctx, id, "background-image", &bg);
        }
        if fill_bg.is_none() {
            if let Some(color) = &el.fill_color {
                if !matches!(el.fill_mode.as_deref(), Some("none")) {
                    set_style(ctx, id, "background-color", color);
                }
            }
        }
    }
    if let Some(st) = &el.shape_type {
        set_attr(ctx, id, "data-shape-type", st);
        apply_preset_geometry(ctx, id, st, el.width, painted_h);
    }
    if let Some(color) = &el.stroke_color {
        let width = el.stroke_width_px.unwrap_or(1.0).max(0.0);
        if width > 0.0 {
            if stroke_uses_css_border(el.stroke_line_align.as_deref()) {
                let style = css_border_dash_style(el.stroke_dash.as_deref());
                set_style(
                    ctx,
                    id,
                    "border",
                    &format!("{}px {} {color}", format_px(width), style),
                );
            } else {
                paint_centered_stroke_overlay(
                    ctx,
                    id,
                    el.width,
                    painted_h,
                    color,
                    width,
                    el.shape_type.as_deref(),
                    el.stroke_dash.as_deref(),
                );
            }
        }
    }
    if let Some(shadow) =
        compose_box_shadow(el.material_box_shadow.as_deref(), el.box_shadow.as_deref())
    {
        set_style(ctx, id, "box-shadow", &shadow);
    }
    if let Some(filter) = el.effect_filter.as_deref() {
        set_style(ctx, id, "filter", filter);
    }
    if let Some(rot) = el.rotation {
        if rot.abs() > 0.01 {
            set_style(ctx, id, "transform", &format!("rotate({}deg)", rot));
        }
    }
    let text = el.text.as_deref().unwrap_or("");
    if !text.trim().is_empty() {
        let block = paint_text_body(ctx, id, text, TextBodyPaint::from_element(el));
        queue_sp_autofit_scroll_height(
            ctx.layout_reads,
            &mut ctx.layout_read_records,
            &el.id,
            el.auto_fit_mode,
            block,
            painted_h,
        );
    }
    ctx.view_ir
        .push_painted(if el.type_ == "text" { "text" } else { "shape" }, el, None);
}

/// Material inset shadows first, then outer/drop shadows (matches upstream layering).
fn compose_box_shadow(material: Option<&str>, outer: Option<&str>) -> Option<String> {
    match (material, outer) {
        (Some(m), Some(o)) => Some(format!("{m}, {o}")),
        (Some(m), None) => Some(m.to_owned()),
        (None, Some(o)) => Some(o.to_owned()),
        (None, None) => None,
    }
}

/// Material gradient on top of fill gradient (TS `visual-3d-materials` order).
fn compose_background_image(overlay: Option<&str>, fill: Option<&str>) -> Option<String> {
    match (overlay, fill) {
        (Some(o), Some(f)) => Some(format!("{o}, {f}")),
        (Some(o), None) => Some(o.to_owned()),
        (None, Some(f)) => Some(f.to_owned()),
        (None, None) => None,
    }
}

fn format_px(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_owned();
    }
    let rounded = v.round();
    if (v - rounded).abs() < 1e-6 {
        format!("{}", rounded as i64)
    } else {
        // Full float string so stroke/font lengths match upstream/Chromium (#27).
        format!("{}", v)
    }
}

/// CSS `border-box` borders match PowerPoint only when `a:ln/@algn="in"`.
fn stroke_uses_css_border(align: Option<&str>) -> bool {
    align == Some("in")
}

const STROKE_OUTLINE_SVG_STYLE: &str =
    "position:absolute;inset:0;width:100%;height:100%;min-width:1px;min-height:1px;overflow:visible;pointer-events:none";

#[allow(clippy::too_many_arguments)]
fn paint_centered_stroke_overlay<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    parent: u32,
    width: f64,
    height: f64,
    color: &str,
    stroke_width: f64,
    shape_type: Option<&str>,
    stroke_dash: Option<&str>,
) {
    if width < 1.0 || height < 1.0 {
        return;
    }
    let w = format_px(width);
    let h = format_px(height);
    let sw = format_px(stroke_width);
    let d = shape_type
        .and_then(|st| preset_stroke_outline_path(st, width, height))
        .unwrap_or_else(|| format!("M 0 0 L {w} 0 L {w} {h} L 0 {h} Z"));
    let dash_attr = svg_stroke_dasharray(stroke_dash, stroke_width)
        .map(|da| format!(" stroke-dasharray=\"{da}\""))
        .unwrap_or_default();
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"pptx-vanilla-gradient-outline\" aria-hidden=\"true\" viewBox=\"0 0 {w} {h}\" preserveAspectRatio=\"none\" style=\"{STROKE_OUTLINE_SVG_STYLE}\"><path d=\"{d}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{sw}\"{dash_attr}/></svg>"
    );
    let overlay = create_child(ctx, parent, "div");
    set_attr(ctx, overlay, "aria-hidden", "true");
    set_style(ctx, overlay, "position", "absolute");
    set_style(ctx, overlay, "inset", "0");
    set_style(ctx, overlay, "width", "100%");
    set_style(ctx, overlay, "height", "100%");
    set_style(ctx, overlay, "min-width", "1px");
    set_style(ctx, overlay, "min-height", "1px");
    set_style(ctx, overlay, "overflow", "visible");
    set_style(ctx, overlay, "pointer-events", "none");
    set_inner_markup_stub(ctx, overlay, &svg);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use nitro_xml_render::measure::{LayoutReadBatch, MeasureBatch};
    use nitro_xml_render::CommandBuffer;

    use super::super::context::{PaintContext, StringIntern};
    use super::super::view_ir::ViewIrCollector;
    use crate::slide::DumpElement;

    use super::paint;

    struct Intern(Vec<String>);

    impl StringIntern for Intern {
        fn intern(&mut self, s: &str) -> u32 {
            if let Some(i) = self.0.iter().position(|existing| existing == s) {
                return i as u32;
            }
            self.0.push(s.to_owned());
            (self.0.len() - 1) as u32
        }
    }

    fn shape_el(stroke_line_align: Option<String>) -> DumpElement {
        DumpElement {
            type_: "shape".to_owned(),
            id: "shp-1".to_owned(),
            shape_id: None,
            name: None,
            placeholder_type: None,
            placeholder_sz: None,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
            x_emu: None,
            y_emu: None,
            width_emu: None,
            height_emu: None,
            rotation: None,
            flip_horizontal: false,
            flip_vertical: false,
            inherited_transform: None,
            text: None,
            shape_type: None,
            table_row_count: None,
            table_data: None,
            children: None,
            fill_color: Some("#ffffff".to_owned()),
            fill_gradient: None,
            fill_mode: Some("solid".to_owned()),
            stroke_color: Some("#4472C4".to_owned()),
            stroke_width_px: Some(2.0),
            stroke_line_align,
            stroke_dash: None,
            connector_start_arrow: None,
            connector_end_arrow: None,
            text_color: None,
            font_size_px: None,
            font_family: None,
            font_bold: false,
            font_italic: false,
            text_stroke_width_px: None,
            text_stroke_color: None,
            text_body_layout: None,
            text_paint_paragraphs: None,
            presentation_kinsoku: false,
            auto_fit_mode: None,
            chart_r_id: None,
            chart_xml: None,
            chart_part_path: None,
            chart_image_urls: None,
            smartart_dm_r_id: None,
            smartart_lo_r_id: None,
            smartart_qs_r_id: None,
            smartart_cs_r_id: None,
            smartart_drawing_path: None,
            smartart_drawing_xml: None,
            smartart_data_xml: None,
            image_rel_id: None,
            image_rel_owner: None,
            ole_spid: None,
            is_media: false,
            media_rel_id: None,
            media_src: None,
            media_type: None,
            image_src: None,
            seq: 0,
            box_shadow: None,
            material_box_shadow: None,
            material_overlay: None,
            text_shadow: None,
            effect_filter: None,
            ink_data: None,
            content_part_ink_rel_id: None,
        }
    }

    fn paint_shape(el: &DumpElement) -> Intern {
        let mut buffer = CommandBuffer::new();
        let mut intern = Intern(Vec::new());
        let mut measure = MeasureBatch::default();
        let mut layout_reads = LayoutReadBatch::default();
        let mut view_ir = ViewIrCollector::default();
        let scheme = BTreeMap::new();
        let mut sp_autofit = crate::render::SpAutofitPaintState::default();
        let slide_autofit = sp_autofit.slide_mut(0);
        let mut ctx = PaintContext::new(
            &mut buffer,
            &mut intern,
            &mut measure,
            &mut layout_reads,
            &mut view_ir,
            1.0,
            960.0,
            540.0,
            &scheme,
            0,
            crate::render::FieldSubstitutionContext::default(),
            slide_autofit,
        );
        paint(&mut ctx, el, 1);
        intern
    }

    #[test]
    fn centered_stroke_uses_svg_overlay_not_css_border() {
        // Catches: default/centred `a:ln/@algn` still paints inset CSS `border` instead of SVG stroke overlay.
        let intern = paint_shape(&shape_el(None));
        assert!(
            !intern.0.iter().any(|s| s == "2px solid #4472C4"),
            "must not use CSS border for centred stroke, got {:?}",
            intern.0
        );
        assert!(
            intern
                .0
                .iter()
                .any(|s| s.contains("pptx-vanilla-gradient-outline")
                    && s.contains("stroke=\"#4472C4\"")),
            "expected SVG stroke overlay, got {:?}",
            intern.0
        );
    }

    #[test]
    fn inset_stroke_keeps_css_border() {
        let intern = paint_shape(&shape_el(Some("in".to_owned())));
        assert!(
            intern.0.iter().any(|s| s == "2px solid #4472C4"),
            "inset stroke should use CSS border, got {:?}",
            intern.0
        );
        assert!(
            !intern
                .0
                .iter()
                .any(|s| s.contains("pptx-vanilla-gradient-outline")),
            "inset stroke must not emit SVG overlay, got {:?}",
            intern.0
        );
    }
}
