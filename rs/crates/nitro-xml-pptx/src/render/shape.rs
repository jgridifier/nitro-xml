// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/dom/elements/text-shape.ts; changes in PROVENANCE.md.

use super::context::{PaintContext, StringIntern};
use super::dom::{create_child, position_box, set_attr, set_inner_markup_stub, set_style};
use super::pattern_tile::get_pattern_tile;
use super::prst_geom::{apply_preset_geometry, preset_stroke_outline_path};
use super::shape_autofit_resize::queue_sp_autofit_scroll_height;
use super::stroke_dash::{css_border_compound_style, svg_stroke_dasharray};
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
    // Upstream only paints an outline when `strokeWidth > 0`; an `a:ln` with
    // solidFill but no `@w` must not invent a 1px hairline (be4aaa).
    let stroke_width = el.stroke_width_px.unwrap_or(0.0).max(0.0);
    let wants_paint_server = matches!(
        el.stroke_fill_mode.as_deref(),
        Some("pattern") | Some("gradient")
    );
    if stroke_width > 0.0
        && (el.stroke_color.is_some() || wants_paint_server)
        && !matches!(el.stroke_fill_mode.as_deref(), Some("none"))
    {
        let color = el.stroke_color.as_deref().unwrap_or("#000000");
        // CSS border cannot express pattern/gradient lines — always SVG overlay
        // (matches upstream `stroke-outline.ts`).
        if !wants_paint_server && stroke_uses_css_border(el.stroke_line_align.as_deref()) {
            let style = css_border_compound_style(
                el.stroke_compound_line.as_deref(),
                el.stroke_dash.as_deref(),
            );
            set_style(
                ctx,
                id,
                "border",
                &format!("{}px {} {color}", format_px(stroke_width), style),
            );
        } else {
            paint_centered_stroke_overlay(ctx, id, el, painted_h, color, stroke_width);
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
    // Apply 3D camera transform if present with explicit rotation override
    if el.scene_3d_camera_rot_x.is_some()
        || el.scene_3d_camera_rot_y.is_some()
        || el.scene_3d_camera_rot_z.is_some()
    {
        let matrix3d = build_3d_camera_transform(el);
        if let Some(transform) = matrix3d {
            set_style(ctx, id, "transform", &transform);
            set_style(ctx, id, "transform-origin", "0 0");
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

fn paint_centered_stroke_overlay<I: StringIntern>(
    ctx: &mut PaintContext<'_, I>,
    parent: u32,
    el: &DumpElement,
    height: f64,
    color: &str,
    stroke_width: f64,
) {
    let width = el.width;
    if width < 1.0 || height < 1.0 {
        return;
    }
    let w = format_px(width);
    let h = format_px(height);
    let d = el
        .shape_type
        .as_deref()
        .and_then(|st| preset_stroke_outline_path(st, width, height))
        .unwrap_or_else(|| format!("M 0 0 L {w} 0 L {w} {h} L 0 {h} Z"));

    // Compound line support: get offsets and widths for each strand
    let offsets = super::compound_line::get_compound_line_offsets(
        el.stroke_compound_line.as_deref(),
        stroke_width,
    );
    let widths = super::compound_line::get_compound_line_widths(
        el.stroke_compound_line.as_deref(),
        stroke_width,
    );

    let dash_attr = svg_stroke_dasharray(el.stroke_dash.as_deref(), stroke_width)
        .map(|da| format!(" stroke-dasharray=\"{da}\""))
        .unwrap_or_default();
    let paint_id = svg_paint_id(&el.id);
    let (defs, stroke_attr) = stroke_paint_server(el, color, &paint_id);
    let defs_block = if defs.is_empty() {
        String::new()
    } else {
        format!("<defs>{defs}</defs>")
    };

    // Emit one <path> per compound strand
    let mut paths = String::new();
    for (i, (&offset, &width)) in offsets.iter().zip(widths.iter()).enumerate() {
        let sw = format_px(width);
        let transform = if offset.abs() > 0.01 {
            format!(" style=\"transform: translate(0, {offset}px)\"")
        } else {
            String::new()
        };
        let dash_for_strand = if i > 0 && !dash_attr.is_empty() {
            // Only apply dasharray to first strand to avoid double-dashing compound lines
            String::new()
        } else {
            dash_attr.clone()
        };
        paths.push_str(&format!(
            "<path d=\"{d}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{sw}\"{dash}{transform}/>",
            d = d,
            stroke = stroke_attr,
            sw = sw,
            dash = dash_for_strand,
            transform = transform,
        ));
    }

    let svg = format!(
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"pptx-vanilla-gradient-outline\" ",
            "aria-hidden=\"true\" viewBox=\"0 0 {w} {h}\" preserveAspectRatio=\"none\" ",
            "style=\"{style}\">{defs_block}{paths}</svg>"
        ),
        w = w,
        h = h,
        style = STROKE_OUTLINE_SVG_STYLE,
        defs_block = defs_block,
        paths = paths,
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

fn svg_paint_id(element_id: &str) -> String {
    let seed: String = element_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("pptx-strokepat-{seed}")
}

/// `(defs_inner_markup, stroke_attribute_value)`.
fn stroke_paint_server(el: &DumpElement, color: &str, paint_id: &str) -> (String, String) {
    match el.stroke_fill_mode.as_deref() {
        Some("pattern") => {
            let preset = el.stroke_pattern_preset.as_deref().unwrap_or("");
            let bg = el.stroke_pattern_bg.as_deref().unwrap_or("#FFFFFF");
            if let Some(tile) = get_pattern_tile(preset, color, bg) {
                let markup = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">{}</svg>"#,
                    tile.w, tile.h, tile.inner
                );
                // Keep URI encoding minimal and stable (TS uses encodeURIComponent).
                let href = format!("data:image/svg+xml,{}", urlencoding_minimal(&markup));
                let defs = format!(
                    r#"<pattern id="{paint_id}" patternUnits="userSpaceOnUse" width="{tw}" height="{th}"><image href="{href}" width="{tw}" height="{th}"/></pattern>"#,
                    tw = tile.w,
                    th = tile.h,
                );
                return (defs, format!("url(#{paint_id})"));
            }
            (String::new(), color.to_owned())
        }
        Some("gradient") => {
            if let Some(stops) = el.stroke_gradient_stops.as_ref().filter(|s| s.len() >= 2) {
                let angle = el.stroke_gradient_angle.unwrap_or(0.0);
                let (x1, y1, x2, y2) = linear_gradient_endpoints(angle);
                let mut stop_markup = String::new();
                for (pos, stop_color) in stops {
                    let offset = (*pos / 100.0).clamp(0.0, 1.0);
                    stop_markup.push_str(&format!(
                        r#"<stop offset="{offset}" stop-color="{stop_color}"/>"#
                    ));
                }
                let defs = format!(
                    r#"<linearGradient id="{paint_id}" gradientUnits="objectBoundingBox" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}">{stop_markup}</linearGradient>"#
                );
                return (defs, format!("url(#{paint_id})"));
            }
            (String::new(), color.to_owned())
        }
        _ => (String::new(), color.to_owned()),
    }
}

/// OOXML `a:lin/@ang` → objectBoundingBox endpoints (upstream `linearEndpoints`).
fn linear_gradient_endpoints(angle_degrees: f64) -> (f64, f64, f64, f64) {
    let radians = angle_degrees.to_radians();
    let dx = radians.cos();
    let dy = radians.sin();
    let span = (dx.abs() + dy.abs()) / 2.0;
    let round4 = |v: f64| (v * 10000.0).round() / 10000.0;
    (
        round4(0.5 - dx * span),
        round4(0.5 - dy * span),
        round4(0.5 + dx * span),
        round4(0.5 + dy * span),
    )
}

fn urlencoding_minimal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Build a matrix3d transform string for 3D camera with explicit rotation override.
fn build_3d_camera_transform(el: &DumpElement) -> Option<String> {
    use crate::visual_3d_camera_homography::homography_to_matrix3d;
    use crate::visual_3d_camera_parametric::{
        compute_parametric_camera_homography, sixty_thousandths_deg_to_rad, ParametricCameraParams,
    };

    // Only apply if we have explicit rotation overrides
    if el.scene_3d_camera_rot_x.is_none()
        && el.scene_3d_camera_rot_y.is_none()
        && el.scene_3d_camera_rot_z.is_none()
    {
        return None;
    }

    let lat_rad = sixty_thousandths_deg_to_rad(el.scene_3d_camera_rot_x);
    let lon_rad = sixty_thousandths_deg_to_rad(el.scene_3d_camera_rot_y);
    let rev_rad = sixty_thousandths_deg_to_rad(el.scene_3d_camera_rot_z);

    let params = ParametricCameraParams {
        lat_rad,
        lon_rad,
        rev_rad,
        fov_rad: 0.0, // Not used in orthographic parametric model
    };

    let h_unit = compute_parametric_camera_homography(params);
    let matrix3d = homography_to_matrix3d(&h_unit, el.width, el.height);
    Some(matrix3d)
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
            stroke_compound_line: None,
            stroke_dash: None,
            stroke_fill_mode: None,
            stroke_pattern_preset: None,
            stroke_pattern_bg: None,
            stroke_gradient_angle: None,
            stroke_gradient_stops: None,
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
            editor_text: None,
            chart_r_id: None,
            chart_xml: None,
            chart_part_path: None,
            chart_image_urls: None,
            chart_minor_font: None,
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
            ole_prog_id: None,
            ole_name: None,
            ole_cls_id: None,
            is_media: false,
            media_rel_id: None,
            media_src: None,
            media_type: None,
            image_src: None,
            image_fit: None,
            seq: 0,
            box_shadow: None,
            material_box_shadow: None,
            material_overlay: None,
            text_shadow: None,
            effect_filter: None,
            scene_3d_camera_preset: None,
            scene_3d_camera_rot_x: None,
            scene_3d_camera_rot_y: None,
            scene_3d_camera_rot_z: None,
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

    #[test]
    fn catches_pattern_stroke_overlay_uses_svg_pattern_paint_server() {
        // Catches: a:ln/a:pattFill flattened to solid strokeColor so hatching disappears.
        let mut el = shape_el(None);
        el.id = "pattern-dkDnDiag".to_owned();
        el.shape_type = Some("rect".to_owned());
        el.stroke_fill_mode = Some("pattern".to_owned());
        el.stroke_pattern_preset = Some("dkDnDiag".to_owned());
        el.stroke_pattern_bg = Some("#FFF2CC".to_owned());
        el.stroke_color = Some("#1F4E79".to_owned());
        el.stroke_width_px = Some(8.0);
        let intern = paint_shape(&el);
        let joined = intern.0.join("\n");
        assert!(
            joined.contains("<pattern") && joined.contains("url(#pptx-strokepat-"),
            "expected SVG pattern paint server, got {joined}"
        );
        assert!(
            joined.contains("#FFF2CC") || joined.contains("%23FFF2CC"),
            "pattern bg must appear in tile/data URI, got {joined}"
        );
    }

    #[test]
    fn catches_gradient_stroke_overlay_uses_svg_linear_gradient() {
        // Catches: a:ln/a:gradFill flattened to first-stop solid so two-tone outlines disappear.
        let mut el = shape_el(None);
        el.id = "gradient-outline".to_owned();
        el.shape_type = Some("rect".to_owned());
        el.stroke_fill_mode = Some("gradient".to_owned());
        el.stroke_gradient_angle = Some(0.0);
        el.stroke_gradient_stops = Some(vec![
            (0.0, "#FF0000".to_owned()),
            (100.0, "#0000FF".to_owned()),
        ]);
        el.stroke_color = Some("#FF0000".to_owned());
        el.stroke_width_px = Some(8.0);
        let intern = paint_shape(&el);
        let joined = intern.0.join("\n");
        assert!(
            joined.contains("<linearGradient") && joined.contains("#0000FF"),
            "expected SVG linearGradient with both stops, got {joined}"
        );
    }
}
