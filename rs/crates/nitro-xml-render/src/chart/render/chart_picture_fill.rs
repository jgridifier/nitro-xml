// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/ui/src/pptx/render/chart-datapoint-picture-fills.ts and chart-picture-pattern-def.ts; changes listed in PROVENANCE.md.

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::chart::{
    chart_source_values, ChartPictureOptions, ChartSpace, ChartSpaceSeries,
};
use nitro_xml_opc::drawingml::DrawingFill;

use super::svg_util::{esc, n, Rect};

#[derive(Clone, Debug)]
pub struct ResolvedPointPicture {
    pub image_url: String,
    pub format: PictureFormat,
    pub tile_height_px: Option<f64>,
    pub opacity: Option<f64>,
    pub apply_front: bool,
    pub apply_side: bool,
    pub apply_end: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureFormat {
    Stretch,
    Stack,
    StackScale,
}

#[derive(Clone, Debug, Default)]
pub struct ChartPictureFills {
    pub by_series: Vec<HashMap<usize, ResolvedPointPicture>>,
}

struct PictureSource {
    rel_id: Option<String>,
    opacity: Option<f64>,
    options: Option<ChartPictureOptions>,
    implied: bool,
}

fn picture_from_sp_pr(sp_pr: Option<&nitro_xml_opc::chart::ChartShapeProperties>) -> PictureSource {
    match sp_pr.and_then(|sp| sp.fill.as_ref()) {
        Some(DrawingFill::Picture { rel_id, opacity }) => PictureSource {
            rel_id: rel_id.clone(),
            opacity: *opacity,
            options: None,
            implied: true,
        },
        _ => PictureSource {
            rel_id: None,
            opacity: None,
            options: None,
            implied: false,
        },
    }
}

fn merge_picture(
    sp_pr: Option<&nitro_xml_opc::chart::ChartShapeProperties>,
    options: Option<&ChartPictureOptions>,
) -> PictureSource {
    let base = picture_from_sp_pr(sp_pr);
    if options.is_some() {
        PictureSource {
            rel_id: base.rel_id,
            opacity: base.opacity,
            options: options.cloned(),
            implied: false,
        }
    } else {
        base
    }
}

fn face_targets(options: Option<&ChartPictureOptions>, has_picture: bool) -> (bool, bool, bool) {
    if !has_picture {
        return (false, false, false);
    }
    let Some(po) = options else {
        return (true, true, true);
    };
    if po.apply_to_front.is_none() && po.apply_to_sides.is_none() && po.apply_to_end.is_none() {
        return (true, true, true);
    }
    (
        po.apply_to_front.unwrap_or(false),
        po.apply_to_sides.unwrap_or(false),
        po.apply_to_end.unwrap_or(false),
    )
}

fn parse_format(options: Option<&ChartPictureOptions>, implied: bool) -> PictureFormat {
    let raw = options.and_then(|o| o.picture_format.as_deref());
    match raw {
        Some("stack") => PictureFormat::Stack,
        Some("stackScale") => PictureFormat::StackScale,
        _ if implied => PictureFormat::Stretch,
        Some("stretch") | Some("sideways") | None => PictureFormat::Stretch,
        _ => PictureFormat::Stretch,
    }
}

fn stack_tile_height(options: Option<&ChartPictureOptions>, format: PictureFormat) -> Option<f64> {
    if format == PictureFormat::Stretch {
        return None;
    }
    options
        .and_then(|o| o.picture_stack_unit)
        .map(|pt| pt * 96.0 / 72.0)
}

fn resolve_one(
    source: &PictureSource,
    images: &HashMap<String, String>,
) -> Option<ResolvedPointPicture> {
    let rel_id = source.rel_id.as_deref()?;
    let image_url = images.get(rel_id)?.clone();
    let format = parse_format(source.options.as_ref(), source.implied);
    let (apply_front, apply_side, apply_end) = face_targets(source.options.as_ref(), true);
    Some(ResolvedPointPicture {
        image_url,
        format,
        tile_height_px: stack_tile_height(source.options.as_ref(), format),
        opacity: source.opacity,
        apply_front,
        apply_side,
        apply_end,
    })
}

fn active_point_picture(
    series: &ChartSpaceSeries,
    point_index: usize,
    images: &HashMap<String, String>,
) -> Option<ResolvedPointPicture> {
    let point = series
        .data_points
        .iter()
        .find(|p| p.index == Some(point_index as i64));
    if let Some(dp) = point {
        let src = merge_picture(dp.sp_pr.as_ref(), dp.picture_options.as_ref());
        if let Some(resolved) = resolve_one(&src, images) {
            return Some(resolved);
        }
    }
    let series_src = merge_picture(series.sp_pr.as_ref(), series.picture_options.as_ref());
    resolve_one(&series_src, images)
}

fn blip_rel_from_sp_pr(
    sp_pr: Option<&nitro_xml_opc::chart::ChartShapeProperties>,
) -> Option<String> {
    match sp_pr.and_then(|sp| sp.fill.as_ref()) {
        Some(DrawingFill::Picture { rel_id, .. }) => rel_id.clone(),
        _ => None,
    }
}

/// Every `a:blipFill` relationship id referenced by the chart model.
pub fn chart_blip_rel_ids(space: &ChartSpace) -> HashSet<String> {
    let mut ids = HashSet::new();
    let group = space.plot_area.groups.first();
    if let Some(g) = group {
        for series in &g.series {
            if let Some(id) = blip_rel_from_sp_pr(series.sp_pr.as_ref()) {
                ids.insert(id);
            }
            for dp in &series.data_points {
                if let Some(id) = blip_rel_from_sp_pr(dp.sp_pr.as_ref()) {
                    ids.insert(id);
                }
            }
        }
    }
    ids
}

pub fn resolve_chart_picture_fills(
    space: &ChartSpace,
    images: &HashMap<String, String>,
) -> ChartPictureFills {
    let group = space.plot_area.groups.first();
    let mut by_series = Vec::new();
    if let Some(g) = group {
        for series in &g.series {
            let max_points = chart_source_values(series.values.as_ref())
                .len()
                .max(chart_source_values(series.categories.as_ref()).len());
            let mut points = HashMap::new();
            for pi in 0..max_points {
                if let Some(pic) = active_point_picture(series, pi, images) {
                    points.insert(pi, pic);
                }
            }
            by_series.push(points);
        }
    }
    ChartPictureFills { by_series }
}

pub fn pattern_id(prefix: &str, series_index: usize, point_index: usize) -> String {
    format!("{}-s{}-p{}", prefix, series_index, point_index)
}

pub fn build_pattern_def(
    id: &str,
    href: &str,
    format: PictureFormat,
    rect: Rect,
    tile_height_px: Option<f64>,
) -> String {
    let stretch = format == PictureFormat::Stretch;
    let height = if stretch {
        rect.h
    } else {
        tile_height_px.unwrap_or(rect.h)
    };
    let preserve = if stretch { "none" } else { "xMidYMid slice" };
    format!(
        "<pattern id=\"{}\" patternUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"{}\"><image href=\"{}\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\"/></pattern>",
        esc(id),
        n(rect.x),
        n(rect.y),
        n(rect.w.max(0.0)),
        n(height.max(0.0)),
        preserve,
        esc(href),
        n(rect.w.max(0.0)),
        n(height.max(0.0))
    )
}

#[allow(clippy::too_many_arguments)]
pub fn fill_for_bar_face(
    picture: Option<&ResolvedPointPicture>,
    is3_d: bool,
    face: &str,
    solid: &str,
    prefix: &str,
    series_index: usize,
    point_index: usize,
    rect: Rect,
    defs: &mut Vec<String>,
) -> String {
    let Some(pic) = picture else {
        return solid.to_string();
    };
    let targeted = if is3_d {
        match face {
            "front" => pic.apply_front,
            "side" => pic.apply_side,
            "end" => pic.apply_end,
            _ => true,
        }
    } else {
        true
    };
    if !targeted {
        return solid.to_string();
    }
    let id = pattern_id(prefix, series_index, point_index);
    if face == "front" {
        defs.push(build_pattern_def(
            &id,
            &pic.image_url,
            pic.format,
            rect,
            pic.tile_height_px,
        ));
    }
    format!("url(#{})", id)
}
