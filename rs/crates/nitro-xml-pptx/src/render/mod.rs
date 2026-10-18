// SPDX-License-Identifier: MIT
//! pptx slide painters (P1–P23, issue #27). One command buffer per frame.

mod activex;
mod chart;
mod chart3d;
mod chart_vm;
mod comments;
mod compound_line;
mod connector;
mod connector_geometry;
mod connector_markers;
mod content_part;
mod context;
mod dispatch;
mod dom;
mod equation;
mod field_substitution;
mod font_substitution;
mod group;
mod image_fill_counter_transform;
mod ink;
mod media;
mod model3d;
mod ole;
mod pattern_tile;
mod picture;
mod placeholder;
mod print;
mod prst_geom;
mod raster_export;
mod selection;
mod shape;
mod shape_autofit_resize;
mod slideshow;
mod smartart;
mod smartart3d;
mod stage;
mod stroke_dash;
mod svg_export;
mod table;
mod text;
mod three_host;
mod view_ir;
mod wordart;
mod wordart_envelope_affine;
mod wordart_envelope_curves;
mod wordart_envelope_map;
mod wordart_envelope_outline_warp;
mod wordart_preset_definitions;
mod wordart_preset_sampler;
mod zoom;

pub use chart3d::paint as paint_chart3d;
pub use context::StringIntern;
pub use field_substitution::FieldSubstitutionContext;
pub use print::print_slide_html;
pub use raster_export::export_slide_png;
pub use selection::{contains_id, copy_text, hit_top_level, top_level_ids};
pub use smartart3d::paint as paint_smartart3d;
pub use svg_export::export_slide_svg;
pub use three_host::is_available as webgl_host_available;
pub use view_ir::{dump_pptx_view_ir, dump_pptx_view_ir_string, ViewIrCollector, ViewIrNode};

use nitro_xml_render::measure::{MeasureBatch, OutlineBatch};
use nitro_xml_render::{CommandBuffer, LayoutReadBatch};

use crate::presentation::Presentation;

pub use shape_autofit_resize::SpAutofitPaintState;
use shape_autofit_resize::SpAutofitPaintState as SpAutofitState;

pub struct SlidePaintViewport {
    pub slide_index: usize,
    pub scale: f64,
    pub viewport_w: i32,
    pub viewport_h: i32,
    /// Top-level element ids to outline (`.pptx-sel`).
    pub selected_ids: Vec<String>,
}

impl Default for SlidePaintViewport {
    fn default() -> Self {
        Self {
            slide_index: 0,
            scale: 1.0,
            viewport_w: 800,
            viewport_h: 600,
            selected_ids: Vec::new(),
        }
    }
}

/// Scale that fits the slide into the viewport (same rule as paint when `scale` is 0).
pub fn fit_slide_scale(canvas_w: f64, canvas_h: f64, viewport_w: i32, viewport_h: i32) -> f64 {
    if viewport_w <= 0 || viewport_h <= 0 {
        return 1.0;
    }
    let sx = viewport_w as f64 / canvas_w.max(1.0);
    let sy = viewport_h as f64 / canvas_h.max(1.0);
    sx.min(sy).min(1.0)
}

/// Paint one slide into `buffer` and return view-IR nodes for the active slide.
#[allow(clippy::too_many_arguments)]
pub fn paint_slide_frame<I: StringIntern>(
    buffer: &mut CommandBuffer,
    intern: &mut I,
    measure: &mut MeasureBatch,
    outlines: &mut OutlineBatch,
    layout_reads: &mut LayoutReadBatch,
    pres: &mut Presentation,
    sp_autofit: &mut SpAutofitState,
    viewport: &SlidePaintViewport,
) -> Result<Vec<ViewIrNode>, crate::presentation::Error> {
    let (canvas_w, canvas_h) = pres.canvas_size();
    let scale = if viewport.scale > 0.0 {
        viewport.scale
    } else {
        fit_slide_scale(canvas_w, canvas_h, viewport.viewport_w, viewport.viewport_h)
    };
    let slide_index = viewport.slide_index;
    if layout_reads.awaiting() == 0 && !layout_reads.values.is_empty() {
        sp_autofit
            .slide_mut(slide_index)
            .apply_layout_results(layout_reads);
    }
    buffer.begin();
    // Fresh per-frame queues (awaiting replies from a prior take stay intact).
    measure.clear_pending();
    outlines.clear_pending();
    layout_reads.clear_pending();
    let mut view_ir = ViewIrCollector::default();
    let records = {
        let slide = pres.slide(viewport.slide_index)?;
        let scheme = slide.scheme_colors.clone();
        let field_ctx = field_substitution::build_field_context(slide_index, slide);
        let slide_autofit = sp_autofit.slide_mut(slide_index);
        let mut ctx = context::PaintContext::new(
            buffer,
            intern,
            measure,
            outlines,
            layout_reads,
            &mut view_ir,
            scale,
            canvas_w,
            canvas_h,
            &scheme,
            slide_index,
            field_ctx,
            slide_autofit,
        );
        stage::paint_slide_stage(&mut ctx, slide, &viewport.selected_ids);
        ctx.layout_read_records
    };
    sp_autofit.slide_mut(slide_index).finish_frame(records);
    Ok(view_ir.nodes)
}

pub fn open_and_paint_slide_frame<I: StringIntern>(
    bytes: &[u8],
    intern: &mut I,
    measure: &mut MeasureBatch,
    outlines: &mut OutlineBatch,
    layout_reads: &mut LayoutReadBatch,
    viewport: &SlidePaintViewport,
) -> Result<(CommandBuffer, Vec<ViewIrNode>), crate::presentation::Error> {
    let mut pres = Presentation::open(bytes)?;
    let mut sp_autofit = SpAutofitState::default();
    let mut buffer = CommandBuffer::new();
    let nodes = paint_slide_frame(
        &mut buffer,
        intern,
        measure,
        outlines,
        layout_reads,
        &mut pres,
        &mut sp_autofit,
        viewport,
    )?;
    Ok((buffer, nodes))
}

#[cfg(test)]
mod tests {
    use super::view_ir::ViewIrCollector;
    use crate::slide::DumpElement;

    // Catches: view-IR parity used model `picture` while the oracle expects `image`.
    #[test]
    fn view_ir_normalizes_picture_to_image() {
        let el = DumpElement {
            type_: "picture".to_owned(),
            id: "pic-1".to_owned(),
            shape_id: None,
            name: None,
            placeholder_type: None,
            placeholder_sz: None,
            x: 1.0,
            y: 2.0,
            width: 10.0,
            height: 20.0,
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
            fill_color: None,
            fill_gradient: None,
            fill_mode: None,
            stroke_color: None,
            stroke_width_px: None,
            stroke_line_align: None,
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
            compat_line_spacing: false,
            auto_fit_mode: None,
            text_warp_preset: None,
            text_warp_adj: None,
            text_warp_adj2: None,
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
            image_tile: None,
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
            path_data: None,
            path_width: None,
            path_height: None,
            fill_image_rot_with_shape: None,
        };
        let mut c = ViewIrCollector::default();
        c.push_painted("element", &el, None);
        assert_eq!(c.nodes[0].type_, "image");
    }
}
