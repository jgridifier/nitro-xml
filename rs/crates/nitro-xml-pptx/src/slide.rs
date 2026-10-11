// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/PptxHandlerRuntimeSpTreeParsing.ts,
// PptxHandlerRuntimeShapeParsing.ts, PptxHandlerRuntimePictureParsing.ts, PptxHandlerRuntimeGroupParsing.ts,
// PptxGraphicFrameParser.ts and PptxHandlerRuntimeBackgroundParsing.ts; changes in PROVENANCE.md.

use std::collections::BTreeMap;

use nitro_xml_opc::drawingml::{
    emu_to_pixels, hex_to_rgb_channels, parse_drawing_color_in, parse_drawing_fill,
    parse_drawing_geometry, parse_drawing_line, parse_transform, resolve_drawing_color,
    DrawingColor, DrawingColorTheme, DrawingFill, EMU_PER_PIXEL,
};
use nitro_xml_opc::xml::{ns, Element, Node};
use serde::Serialize;
use serde_json::{json, Value};

use crate::inheritance::{
    attr, child_local, child_path, children_local, extract_placeholder_info,
    find_placeholder_in_sp_tree, find_singleton_placeholder_in_sp_tree,
    is_header_footer_placeholder, merge_elements, merge_placeholder_context, sp_pr_of_placeholder,
    tx_body_of_placeholder, PlaceholderInfo, PlaceholderNode,
};
use crate::level_style::{LevelStylesMap, MasterTxStyles};
use crate::placeholder_defaults::PlaceholderDefaultsCache;
use crate::table::table_row_count;
use crate::text::{
    extract_element_text, placeholder_is_body_type, text_body_has_renderable_content,
    TextExtractContext,
};

/// EMU per CSS pixel at 96 DPI (same as [`EMU_PER_PIXEL`]).
pub const EMU_PER_PX: i64 = EMU_PER_PIXEL;

const PORTED: &[&str] = &[
    "shape",
    "text",
    "picture",
    "image",
    "group",
    "connector",
    "table",
];

#[derive(Debug, Clone, Serialize)]
pub struct DumpElement {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder_sz: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_emu: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_emu: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_emu: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_emu: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flip_horizontal: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flip_vertical: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inherited_transform: Option<InheritedTransform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_row_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<DumpElement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_gradient: Option<String>,
    /// `none`, `solid`, or `gradient`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_width_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart_r_id: Option<String>,
    /// Chart part XML. Not part of the issue-24 dump.
    #[serde(skip)]
    pub chart_xml: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smartart_dm_r_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smartart_lo_r_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smartart_qs_r_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smartart_cs_r_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smartart_drawing_path: Option<String>,
    /// Diagram drawing part XML (`dsp:drawing`). Not part of the issue-24 dump.
    #[serde(skip)]
    pub smartart_drawing_xml: Option<String>,
    /// Document order of this element among emitted spTree children (dump merge).
    #[serde(skip)]
    pub seq: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct InheritedTransform {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flip_horizontal: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flip_vertical: bool,
}

pub struct ParseCtx<'a> {
    #[allow(dead_code)]
    pub slide_path: &'a str,
    pub layout_sp_tree: Option<&'a Element>,
    pub master_sp_tree: Option<&'a Element>,
    pub presentation_level_styles: Option<&'a LevelStylesMap>,
    pub layout_placeholder_levels: Option<&'a PlaceholderDefaultsCache>,
    pub master_placeholder_levels: Option<&'a PlaceholderDefaultsCache>,
    pub master_tx_styles: Option<&'a MasterTxStyles>,
    /// Presentation theme scheme (`dk1`, `accent1`, …) plus Office defaults.
    pub theme_colors: Option<&'a BTreeMap<String, String>>,
}

impl<'a> ParseCtx<'a> {
    fn extract_text(
        &self,
        own_tx_body: Option<&Element>,
        inherited_tx_body: Option<&Element>,
        effective_tx_body: &Element,
        ph_info: Option<&PlaceholderInfo>,
    ) -> String {
        let effective_levels = ph_info.map(|ph| {
            let layout_levels = self
                .layout_placeholder_levels
                .and_then(|c| c.get(&crate::placeholder_defaults::placeholder_defaults_key(ph)));
            let master_levels = self
                .master_placeholder_levels
                .and_then(|c| c.get(&crate::placeholder_defaults::placeholder_defaults_key(ph)));
            crate::placeholder_defaults::lookup_effective_level_styles(
                layout_levels,
                master_levels,
                self.master_tx_styles,
                ph.type_.as_deref(),
                self.presentation_level_styles,
            )
        });
        let is_body = ph_info.is_some_and(placeholder_is_body_type);
        let text_ctx = TextExtractContext {
            tx_body: own_tx_body,
            inherited_tx_body,
            is_body_placeholder: is_body,
            effective_level_styles: effective_levels.as_ref(),
            presentation_level_styles: self.presentation_level_styles,
        };
        extract_element_text(&text_ctx, effective_tx_body)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SpTreeOptions {
    pub skip_placeholders: bool,
    pub template_scope: Option<String>,
    pub template_token: Option<String>,
}

fn sp_tree_child_id(prefix: &str, kind: &str, index: usize, options: &SpTreeOptions) -> String {
    if let (Some(scope), Some(token)) = (&options.template_scope, &options.template_token) {
        format!("{}-{}-{}-{}-{}", scope, scope, kind, token, index)
    } else {
        format!("{}{}-{}", prefix, kind, index)
    }
}

/// Unwrap `mc:AlternateContent` Choice (else Fallback) in place so spTree children are visible.
pub fn unwrap_alternate_content_deep(root: &mut Element) {
    let mut i = 0;
    while i < root.children.len() {
        let hoist = match &root.children[i] {
            Node::Element(el)
                if el.ns.as_deref() == Some(ns::MC) && el.local == "AlternateContent" =>
            {
                let branch = children_local(el, "Choice", ns::MC)
                    .into_iter()
                    .next()
                    .or_else(|| children_local(el, "Fallback", ns::MC).into_iter().next());
                branch.map(|b| b.children.clone())
            }
            _ => None,
        };
        if let Some(new_children) = hoist {
            root.children.remove(i);
            for child in new_children {
                if let Node::Element(mut el) = child {
                    unwrap_alternate_content_deep(&mut el);
                    root.children.insert(i, Node::Element(el));
                    i += 1;
                } else {
                    root.children.insert(i, child);
                    i += 1;
                }
            }
            continue;
        }
        if let Node::Element(el) = &mut root.children[i] {
            unwrap_alternate_content_deep(el);
        }
        i += 1;
    }
}

#[allow(clippy::large_enum_variant)]
enum SpTreeParseOutcome {
    Element(DumpElement),
    Other(String),
}

pub struct ParsedSpTree {
    pub elements: Vec<DumpElement>,
    pub other_kinds: Vec<String>,
    /// `(seq, kind)` for kinds that stayed out of `elements`, in document order.
    pub other_kind_seq: Vec<(u32, String)>,
}

pub fn parse_sp_tree(
    sp_tree: &Element,
    raw_xml: Option<&str>,
    id_prefix: &str,
    ctx: &ParseCtx,
    options: &SpTreeOptions,
) -> ParsedSpTree {
    let order = extract_sp_tree_child_order(raw_xml, sp_tree, "p:spTree");
    let mut parsed = ParsedSpTree {
        elements: Vec::new(),
        other_kinds: Vec::new(),
        other_kind_seq: Vec::new(),
    };
    let mut seq = 1u32;
    for entry in order {
        match parse_sp_tree_child(&entry, sp_tree, raw_xml, id_prefix, ctx, options) {
            Some(SpTreeParseOutcome::Element(mut el)) => {
                el.seq = seq;
                parsed.elements.push(el);
                seq += 1;
            }
            Some(SpTreeParseOutcome::Other(kind)) => {
                parsed.other_kind_seq.push((seq, kind.clone()));
                parsed.other_kinds.push(kind);
                seq += 1;
            }
            None => {}
        }
    }
    parsed
}

pub fn shift_parse_seq(tree: &mut ParsedSpTree, base: u32) {
    for el in &mut tree.elements {
        el.seq = el.seq.saturating_add(base);
    }
    for (seq, _) in &mut tree.other_kind_seq {
        *seq = seq.saturating_add(base);
    }
}

pub fn concat_parsed(base: &mut ParsedSpTree, extra: ParsedSpTree) {
    base.elements.extend(extra.elements);
    base.other_kinds.extend(extra.other_kinds);
    base.other_kind_seq.extend(extra.other_kind_seq);
}

struct SpTreeEntry {
    tag: String,
    index_in_type: usize,
}

fn extract_sp_tree_child_order(
    _xml_str: Option<&str>,
    sp_tree: &Element,
    _container_tag: &str,
) -> Vec<SpTreeEntry> {
    // Walk the owned element tree in document order (avoids UTF-8 index
    // panics from raw-XML scanning and matches upstream once AC is unwrapped).
    let mut order = Vec::new();
    let mut counters: BTreeMap<String, usize> = BTreeMap::new();
    collect_sp_tree_entries(sp_tree, &mut order, &mut counters);
    if order.is_empty() {
        build_type_grouped_order(sp_tree)
    } else {
        order
    }
}

fn build_type_grouped_order(sp_tree: &Element) -> Vec<SpTreeEntry> {
    let tags = ["sp", "pic", "graphicFrame", "grpSp", "cxnSp", "contentPart"];
    let mut order = Vec::new();
    for tag in tags {
        let count = children_local(sp_tree, tag, ns::P).len();
        for i in 0..count {
            order.push(SpTreeEntry {
                tag: format!("p:{tag}"),
                index_in_type: i,
            });
        }
    }
    order
}

fn collect_sp_tree_entries(
    container: &Element,
    order: &mut Vec<SpTreeEntry>,
    counters: &mut BTreeMap<String, usize>,
) {
    for child in &container.children {
        let Node::Element(el) = child else {
            continue;
        };
        if el.ns.as_deref() == Some(ns::MC) && el.local == "AlternateContent" {
            // Prefer Choice branch, else Fallback (same as upstream unwrap).
            let branch = children_local(el, "Choice", ns::MC)
                .into_iter()
                .next()
                .or_else(|| children_local(el, "Fallback", ns::MC).into_iter().next());
            if let Some(branch) = branch {
                collect_sp_tree_entries(branch, order, counters);
            }
            continue;
        }
        let tag = match (el.ns.as_deref(), el.local.as_str()) {
            (Some(n), "sp") if n == ns::P => Some("p:sp"),
            (Some(n), "pic") if n == ns::P => Some("p:pic"),
            (Some(n), "graphicFrame") if n == ns::P => Some("p:graphicFrame"),
            (Some(n), "grpSp") if n == ns::P => Some("p:grpSp"),
            (Some(n), "cxnSp") if n == ns::P => Some("p:cxnSp"),
            (Some(n), "contentPart") if n == ns::P => Some("p:contentPart"),
            (_, "model3D") => Some("p16:model3D"),
            _ => None,
        };
        let Some(tag) = tag else {
            continue;
        };
        let idx = counters.get(tag).copied().unwrap_or(0);
        counters.insert(tag.to_owned(), idx + 1);
        order.push(SpTreeEntry {
            tag: tag.to_owned(),
            index_in_type: idx,
        });
    }
}

fn parse_sp_tree_child(
    entry: &SpTreeEntry,
    sp_tree: &Element,
    raw_xml: Option<&str>,
    id_prefix: &str,
    ctx: &ParseCtx,
    options: &SpTreeOptions,
) -> Option<SpTreeParseOutcome> {
    if entry.tag == "p16:model3D" {
        return Some(SpTreeParseOutcome::Other("unknown".to_owned()));
    }
    let local = entry.tag.strip_prefix("p:").unwrap_or(&entry.tag);
    let nodes = children_local(sp_tree, local, ns::P);
    let node = nodes.get(entry.index_in_type)?;
    match local {
        "sp" => {
            if options.skip_placeholders {
                let ph = child_path(node, &[("nvSpPr", ns::P), ("nvPr", ns::P), ("ph", ns::P)]);
                if ph.is_some() {
                    return None;
                }
            }
            let sp_pr = child_local(node, "spPr", ns::P);
            let image_fill = sp_pr.is_some_and(|sp| child_local(sp, "blipFill", ns::A).is_some());
            if image_fill {
                parse_shape_with_image_fill(
                    node,
                    sp_tree_child_id(id_prefix, "shape-img", entry.index_in_type, options),
                    ctx,
                    false,
                )
                .map(SpTreeParseOutcome::Element)
            } else {
                parse_shape(
                    node,
                    sp_tree_child_id(id_prefix, "shape", entry.index_in_type, options),
                    ctx,
                    false,
                )
                .map(SpTreeParseOutcome::Element)
            }
        }
        "pic" => {
            if picture_is_media(node) {
                return Some(SpTreeParseOutcome::Other("media".to_owned()));
            }
            parse_picture(
                node,
                sp_tree_child_id(id_prefix, "pic", entry.index_in_type, options),
                ctx,
            )
            .map(SpTreeParseOutcome::Element)
        }
        "graphicFrame" => parse_graphic_frame(
            node,
            sp_tree_child_id(id_prefix, "frame", entry.index_in_type, options),
            ctx,
        )
        .map(SpTreeParseOutcome::Element)
        .or(Some(SpTreeParseOutcome::Other(graphic_frame_other_kind(
            node,
        )))),
        "grpSp" => parse_group(
            node,
            sp_tree_child_id(id_prefix, "group", entry.index_in_type, options),
            raw_xml,
            id_prefix,
            ctx,
            options,
        )
        .map(SpTreeParseOutcome::Element),
        "cxnSp" => parse_connector(
            node,
            sp_tree_child_id(id_prefix, "conn", entry.index_in_type, options),
            ctx,
        )
        .map(SpTreeParseOutcome::Element),
        "contentPart" => Some(SpTreeParseOutcome::Other("contentPart".to_owned())),
        _ => None,
    }
}

fn picture_is_media(pic: &Element) -> bool {
    let nv_pr = child_path(pic, &[("nvPicPr", ns::P), ("nvPr", ns::P)]);
    drawing_nvpr_has_media(nv_pr)
}

fn drawing_nvpr_has_media(nv_pr: Option<&Element>) -> bool {
    let Some(nv_pr) = nv_pr else {
        return false;
    };
    for child in &nv_pr.children {
        let Node::Element(el) = child else {
            continue;
        };
        let ns_uri = el.ns.as_deref();
        let local = el.local.as_str();
        if ns_uri == Some(ns::P) && (local == "video" || local == "audio") {
            return true;
        }
        if ns_uri == Some(ns::A)
            && matches!(
                local,
                "videoFile" | "audioFile" | "wavAudioFile" | "quickTimeFile" | "audioCd"
            )
        {
            return true;
        }
    }
    if nv_pr_has_p14_media(nv_pr) {
        return true;
    }
    false
}

fn nv_pr_has_p14_media(nv_pr: &Element) -> bool {
    let ext_lst = child_local(nv_pr, "extLst", ns::P);
    let Some(ext_lst) = ext_lst else {
        return false;
    };
    for ext in children_local(ext_lst, "ext", ns::P) {
        let uri = attr(ext, "uri").map(str::to_lowercase);
        if uri.as_deref() == Some("{daa4b4d4-6d71-4841-9c94-3de7fcfb9230}") {
            for child in &ext.children {
                if let Node::Element(el) = child {
                    if el.local == "media" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn graphic_frame_other_kind(frame: &Element) -> String {
    let graphic = child_path(frame, &[("graphic", ns::A), ("graphicData", ns::A)]);
    if let Some(g) = graphic {
        let uri = attr(g, "uri").map(str::to_lowercase).unwrap_or_default();
        if child_local(g, "tbl", ns::A).is_some() || uri.contains("/drawingml/2006/table") {
            return "table".to_owned();
        }
        if child_local(g, "chart", ns::C).is_some()
            || uri.contains("/drawingml/2006/chart")
            || uri.contains("/2014/chartex")
            || graphic_data_has_chartex(g)
        {
            return "chart".to_owned();
        }
        if child_local(g, "relIds", ns::DGM).is_some() || uri.contains("/drawingml/2006/diagram") {
            return "smartArt".to_owned();
        }
        if uri.contains("/presentationml/2006/ole") || uri.contains("/drawingml/2006/ole") {
            return "ole".to_owned();
        }
        if graphic_data_has_media_payload(g) || uri.contains("/drawingml/2006/media") {
            return "media".to_owned();
        }
        if graphic_data_has_ink(g)
            || uri.contains("/2010/ink")
            || uri.contains("drawing/2010/ink")
            || graphic_data_has_ink_choice(g)
        {
            return "ink".to_owned();
        }
    }
    "unknown".to_owned()
}

fn graphic_data_has_chartex(g: &Element) -> bool {
    for child in &g.children {
        if let Node::Element(el) = child {
            if el.local == "chart" {
                return true;
            }
        }
    }
    false
}

fn graphic_data_has_media_payload(g: &Element) -> bool {
    for child in &g.children {
        if let Node::Element(el) = child {
            if matches!(
                el.local.as_str(),
                "videoFile" | "audioFile" | "wavAudioFile" | "quickTimeFile" | "audioCd"
            ) {
                return true;
            }
        }
    }
    false
}

fn graphic_data_has_ink(g: &Element) -> bool {
    for child in &g.children {
        if let Node::Element(el) = child {
            if el.local == "ink" {
                return true;
            }
        }
    }
    false
}

fn graphic_data_has_ink_choice(g: &Element) -> bool {
    for child in &g.children {
        let Node::Element(el) = child else {
            continue;
        };
        if el.ns.as_deref() == Some(ns::MC) && el.local == "AlternateContent" {
            for choice in children_local(el, "Choice", ns::MC) {
                let requires = attr(choice, "Requires")
                    .map(str::to_lowercase)
                    .unwrap_or_default();
                if requires.contains("aink") || graphic_data_has_ink(choice) {
                    return true;
                }
            }
        }
    }
    false
}

fn find_placeholder_context(ctx: &ParseCtx, info: &PlaceholderInfo) -> Option<PlaceholderNode> {
    let layout = ctx.layout_sp_tree.and_then(|st| {
        find_placeholder_in_sp_tree(st, info)
            .or_else(|| find_singleton_placeholder_in_sp_tree(st, info))
    });
    let master = ctx.master_sp_tree.and_then(|st| {
        find_placeholder_in_sp_tree(st, info)
            .or_else(|| find_singleton_placeholder_in_sp_tree(st, info))
    });
    merge_placeholder_context(layout, master)
}

fn parse_shape(shape: &Element, id: String, ctx: &ParseCtx, in_group: bool) -> Option<DumpElement> {
    let nv_pr = child_path(shape, &[("nvSpPr", ns::P), ("nvPr", ns::P)]);
    let ph_info = extract_placeholder_info(nv_pr);
    let inherited = ph_info
        .as_ref()
        .and_then(|i| find_placeholder_context(ctx, i));
    let sp_pr = child_local(shape, "spPr", ns::P);
    let inherited_sp_pr = inherited.as_ref().and_then(sp_pr_of_placeholder);
    // Bare `<p:spPr/>` is `''` (falsy) in fast-xml-parser, so upstream falls back to
    // the full inherited placeholder spPr — including master `a:noFill` / `a:ln/a:noFill`.
    // An owned spPr that only carries xfrm/geom stays slide-local and still sees fillRef.
    let effective_sp_pr = if let Some(own) = sp_pr {
        if !element_has_element_children(own) {
            inherited_sp_pr.cloned()
        } else {
            let mut merged = own.clone();
            if let Some(inh) = inherited_sp_pr {
                if let Some(inh_xfrm) = child_local(inh, "xfrm", ns::A) {
                    if let Some(own_xfrm) = child_local(own, "xfrm", ns::A) {
                        let mx = merge_elements(Some(inh_xfrm), Some(own_xfrm), 0);
                        if let Some(mx) = mx {
                            replace_child(&mut merged, "xfrm", ns::A, mx);
                        }
                    } else {
                        merged.children.push(Node::Element(inh_xfrm.clone()));
                    }
                }
            }
            Some(merged)
        }
    } else {
        inherited_sp_pr.cloned()
    };
    let xfrm = effective_sp_pr
        .as_ref()
        .and_then(|sp| child_local(sp, "xfrm", ns::A))
        .or_else(|| sp_pr.and_then(|sp| child_local(sp, "xfrm", ns::A)));
    let own_tx = child_local(shape, "txBody", ns::P);
    let inh_tx = inherited.as_ref().and_then(tx_body_of_placeholder);
    let inherits_hf_text = ph_info
        .as_ref()
        .map(|p| is_header_footer_placeholder(p.type_.as_deref()))
        .unwrap_or(false)
        && !text_body_has_content(own_tx)
        && text_body_has_content(inh_tx);
    let tx_body = if inherits_hf_text {
        inh_tx
    } else if ph_info.is_some() && own_tx.is_some() && !text_body_has_content(own_tx) {
        own_tx
    } else {
        own_tx.or(inh_tx)
    };
    let off = xfrm.and_then(|x| child_local(x, "off", ns::A));
    let ext = xfrm.and_then(|x| child_local(x, "ext", ns::A));
    if (xfrm.is_none() || off.is_none() || ext.is_none()) && !text_body_has_content(tx_body) {
        return None;
    }
    let (x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h, flip_v) = xfrm_to_box(xfrm);
    let own_has_xfrm = sp_pr.is_some_and(|sp| child_local(sp, "xfrm", ns::A).is_some());
    let inherited_transform = if !own_has_xfrm && xfrm.is_some() {
        Some(InheritedTransform {
            x,
            y,
            width: w,
            height: h,
            rotation: rot,
            flip_horizontal: flip_h,
            flip_vertical: flip_v,
        })
    } else {
        None
    };
    let shape_type = geometry_name(effective_sp_pr.as_ref());
    let is_plain_rect = shape_type == "rect";
    let has_visible = shape_element_has_visible_style(
        shape,
        effective_sp_pr.as_ref(),
        inherited.as_ref().map(|p| &p.element),
    );
    let text = tx_body
        .map(|tb| ctx.extract_text(own_tx, inh_tx, tb, ph_info.as_ref()))
        .unwrap_or_default();
    let has_text = !text.trim().is_empty();
    let type_ = if has_text && is_plain_rect && !has_visible {
        "text"
    } else {
        "shape"
    };
    let c_nv_pr = child_path(shape, &[("nvSpPr", ns::P), ("cNvPr", ns::P)]);
    let extras = shape_paint_extras(
        shape,
        effective_sp_pr.as_ref(),
        inherited.as_ref().map(|p| &p.element),
        tx_body,
        ctx,
    );
    build_dump(
        type_,
        id,
        c_nv_pr,
        ph_info,
        x,
        y,
        w,
        h,
        x_emu,
        y_emu,
        w_emu,
        h_emu,
        rot,
        flip_h,
        flip_v,
        inherited_transform,
        if tx_body.is_some() || own_tx.is_some() || inh_tx.is_some() {
            Some(text)
        } else {
            None
        },
        Some(shape_type),
        None,
        None,
        in_group,
        extras,
    )
}

fn parse_shape_with_image_fill(
    shape: &Element,
    id: String,
    ctx: &ParseCtx,
    in_group: bool,
) -> Option<DumpElement> {
    let nv_pr = child_path(shape, &[("nvSpPr", ns::P), ("nvPr", ns::P)]);
    let ph_info = extract_placeholder_info(nv_pr);
    let inherited = ph_info
        .as_ref()
        .and_then(|i| find_placeholder_context(ctx, i));
    let sp_pr = child_local(shape, "spPr", ns::P);
    let inherited_sp_pr = inherited.as_ref().and_then(sp_pr_of_placeholder);
    let effective_sp_pr = if let Some(own) = sp_pr {
        let mut merged = own.clone();
        if let Some(inh) = inherited_sp_pr {
            if let Some(inh_xfrm) = child_local(inh, "xfrm", ns::A) {
                if let Some(own_xfrm) = child_local(own, "xfrm", ns::A) {
                    let mx = merge_elements(Some(inh_xfrm), Some(own_xfrm), 0);
                    if let Some(mx) = mx {
                        replace_child(&mut merged, "xfrm", ns::A, mx);
                    }
                } else {
                    merged.children.push(Node::Element(inh_xfrm.clone()));
                }
            }
        }
        Some(merged)
    } else {
        inherited_sp_pr.cloned()
    };
    let xfrm = effective_sp_pr
        .as_ref()
        .and_then(|sp| child_local(sp, "xfrm", ns::A))
        .or_else(|| sp_pr.and_then(|sp| child_local(sp, "xfrm", ns::A)));
    let off = xfrm.and_then(|x| child_local(x, "off", ns::A));
    let ext = xfrm.and_then(|x| child_local(x, "ext", ns::A));
    if xfrm.is_none() || off.is_none() || ext.is_none() {
        return None;
    }
    let (x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h, flip_v) = xfrm_to_box(xfrm);
    let own_has_xfrm = sp_pr.is_some_and(|sp| child_local(sp, "xfrm", ns::A).is_some());
    let inherited_transform = if !own_has_xfrm {
        Some(InheritedTransform {
            x,
            y,
            width: w,
            height: h,
            rotation: rot,
            flip_horizontal: flip_h,
            flip_vertical: flip_v,
        })
    } else {
        None
    };
    let shape_type = geometry_name(effective_sp_pr.as_ref());
    let c_nv_pr = child_path(shape, &[("nvSpPr", ns::P), ("cNvPr", ns::P)]);
    build_dump(
        "picture",
        id,
        c_nv_pr,
        ph_info,
        x,
        y,
        w,
        h,
        x_emu,
        y_emu,
        w_emu,
        h_emu,
        rot,
        flip_h,
        flip_v,
        inherited_transform,
        None,
        Some(shape_type),
        None,
        None,
        in_group,
        PaintExtras::default(),
    )
}

fn parse_picture(pic: &Element, id: String, ctx: &ParseCtx) -> Option<DumpElement> {
    let nv_pr = child_path(pic, &[("nvPicPr", ns::P), ("nvPr", ns::P)]);
    let ph_info = extract_placeholder_info(nv_pr);
    let inherited = ph_info
        .as_ref()
        .and_then(|i| find_placeholder_context(ctx, i));
    let sp_pr = child_local(pic, "spPr", ns::P);
    let inherited_sp_pr = inherited.as_ref().and_then(sp_pr_of_placeholder);
    let effective_sp_pr = merge_elements(inherited_sp_pr, sp_pr, 0)?;
    let xfrm = child_local(&effective_sp_pr, "xfrm", ns::A)?;
    let (x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h, flip_v) = xfrm_to_box(Some(xfrm));
    let own_has_xfrm = sp_pr.is_some_and(|sp| child_local(sp, "xfrm", ns::A).is_some());
    let inherited_transform = if !own_has_xfrm {
        Some(InheritedTransform {
            x,
            y,
            width: w,
            height: h,
            rotation: rot,
            flip_horizontal: flip_h,
            flip_vertical: flip_v,
        })
    } else {
        None
    };
    let shape_type = geometry_name(Some(&effective_sp_pr));
    let c_nv_pr = child_path(pic, &[("nvPicPr", ns::P), ("cNvPr", ns::P)]);
    build_dump(
        "picture",
        id,
        c_nv_pr,
        None,
        x,
        y,
        w,
        h,
        x_emu,
        y_emu,
        w_emu,
        h_emu,
        rot,
        flip_h,
        flip_v,
        inherited_transform,
        None,
        Some(shape_type),
        None,
        None,
        false,
        PaintExtras::default(),
    )
}

fn parse_connector(cxn: &Element, id: String, ctx: &ParseCtx) -> Option<DumpElement> {
    let sp_pr = child_local(cxn, "spPr", ns::P)?;
    let xfrm = child_local(sp_pr, "xfrm", ns::A)?;
    let (x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h, flip_v) = xfrm_to_box(Some(xfrm));
    let tx_body = child_local(cxn, "txBody", ns::P);
    let text = tx_body.map(|tb| ctx.extract_text(tx_body, None, tb, None));
    let shape_type = geometry_name(Some(sp_pr));
    let c_nv_pr = child_path(cxn, &[("nvCxnSpPr", ns::P), ("cNvPr", ns::P)]);
    build_dump(
        "connector",
        id,
        c_nv_pr,
        None,
        x,
        y,
        w,
        h,
        x_emu,
        y_emu,
        w_emu,
        h_emu,
        rot,
        flip_h,
        flip_v,
        None,
        text,
        Some(shape_type),
        None,
        None,
        false,
        PaintExtras::default(),
    )
}

struct GroupTransform {
    parent_x: f64,
    parent_y: f64,
    parent_w: f64,
    parent_h: f64,
    parent_x_emu: i64,
    parent_y_emu: i64,
    parent_w_emu: i64,
    parent_h_emu: i64,
    ch_x: f64,
    ch_y: f64,
    ch_w: f64,
    ch_h: f64,
    scale_x: f64,
    scale_y: f64,
}

fn read_group_transform(xfrm: Option<&Element>) -> GroupTransform {
    let mut t = GroupTransform {
        parent_x: 0.0,
        parent_y: 0.0,
        parent_w: 0.0,
        parent_h: 0.0,
        parent_x_emu: 0,
        parent_y_emu: 0,
        parent_w_emu: 0,
        parent_h_emu: 0,
        ch_x: 0.0,
        ch_y: 0.0,
        ch_w: 0.0,
        ch_h: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
    };
    let Some(xfrm) = xfrm else { return t };
    if let Some(off) = child_local(xfrm, "off", ns::A) {
        t.parent_x_emu = parse_emu_attr(attr(off, "x"));
        t.parent_y_emu = parse_emu_attr(attr(off, "y"));
        t.parent_x = t.parent_x_emu as f64 / EMU_PER_PX as f64;
        t.parent_y = t.parent_y_emu as f64 / EMU_PER_PX as f64;
    }
    if let Some(ext) = child_local(xfrm, "ext", ns::A) {
        t.parent_w_emu = parse_emu_attr(attr(ext, "cx"));
        t.parent_h_emu = parse_emu_attr(attr(ext, "cy"));
        t.parent_w = t.parent_w_emu as f64 / EMU_PER_PX as f64;
        t.parent_h = t.parent_h_emu as f64 / EMU_PER_PX as f64;
    }
    if let Some(ch_off) = child_local(xfrm, "chOff", ns::A) {
        let x = parse_emu_attr(attr(ch_off, "x"));
        let y = parse_emu_attr(attr(ch_off, "y"));
        t.ch_x = x as f64 / EMU_PER_PX as f64;
        t.ch_y = y as f64 / EMU_PER_PX as f64;
    }
    if let Some(ch_ext) = child_local(xfrm, "chExt", ns::A) {
        let w = parse_emu_attr(attr(ch_ext, "cx"));
        let h = parse_emu_attr(attr(ch_ext, "cy"));
        t.ch_w = w as f64 / EMU_PER_PX as f64;
        t.ch_h = h as f64 / EMU_PER_PX as f64;
    }
    t.scale_x = if t.ch_w > 0.0 {
        t.parent_w / t.ch_w
    } else {
        1.0
    };
    t.scale_y = if t.ch_h > 0.0 {
        t.parent_h / t.ch_h
    } else {
        1.0
    };
    t
}

fn transform_group_child(el: &mut DumpElement, t: &GroupTransform) {
    let rx = el.x - t.ch_x;
    let ry = el.y - t.ch_y;
    el.x = t.parent_x + rx * t.scale_x;
    el.y = t.parent_y + ry * t.scale_y;
    el.width *= t.scale_x;
    el.height *= t.scale_y;
    if let Some(children) = &mut el.children {
        for nested in children.iter_mut() {
            scale_element_subtree(nested, t.scale_x, t.scale_y);
        }
    }
}

fn scale_element_subtree(el: &mut DumpElement, scale_x: f64, scale_y: f64) {
    el.x *= scale_x;
    el.y *= scale_y;
    el.width *= scale_x;
    el.height *= scale_y;
    if let Some(children) = &mut el.children {
        for nested in children.iter_mut() {
            scale_element_subtree(nested, scale_x, scale_y);
        }
    }
}

fn parse_graphic_frame(frame: &Element, id: String, _ctx: &ParseCtx) -> Option<DumpElement> {
    let nv_pr = child_path(frame, &[("nvGraphicFramePr", ns::P), ("nvPr", ns::P)]);
    let ph_info = extract_placeholder_info(nv_pr);
    let xfrm = child_path(frame, &[("xfrm", ns::P)])?;
    let (x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h, flip_v) = xfrm_to_box(Some(xfrm));
    let graphic = child_path(frame, &[("graphic", ns::A), ("graphicData", ns::A)])?;
    let c_nv_pr = child_path(frame, &[("nvGraphicFramePr", ns::P), ("cNvPr", ns::P)]);
    if let Some(table) = child_local(graphic, "tbl", ns::A) {
        let (w, h) = table_grid_pixel_extent(table).unwrap_or((w, h));
        let rows = table_row_count(table);
        return build_dump(
            "table",
            id,
            c_nv_pr,
            ph_info,
            x,
            y,
            w,
            h,
            x_emu,
            y_emu,
            w_emu,
            h_emu,
            rot,
            flip_h,
            flip_v,
            None,
            None,
            None,
            Some(rows),
            None,
            false,
            PaintExtras::default(),
        );
    }
    if graphic_is_chart(graphic) {
        let extras = PaintExtras {
            chart_r_id: chart_rel_id(graphic),
            ..PaintExtras::default()
        };
        return build_dump(
            "chart", id, c_nv_pr, ph_info, x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h,
            flip_v, None, None, None, None, None, false, extras,
        );
    }
    if graphic_is_smartart(graphic) {
        let rels = smartart_rel_ids(graphic);
        let extras = PaintExtras {
            smartart_dm_r_id: rels.dm,
            smartart_lo_r_id: rels.lo,
            smartart_qs_r_id: rels.qs,
            smartart_cs_r_id: rels.cs,
            ..PaintExtras::default()
        };
        return build_dump(
            "smartArt", id, c_nv_pr, ph_info, x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h,
            flip_v, None, None, None, None, None, false, extras,
        );
    }
    None
}

fn graphic_is_chart(g: &Element) -> bool {
    let uri = attr(g, "uri").map(str::to_lowercase).unwrap_or_default();
    child_local(g, "chart", ns::C).is_some()
        || uri.contains("/drawingml/2006/chart")
        || uri.contains("/2014/chartex")
        || graphic_data_has_chartex(g)
}

fn graphic_is_smartart(g: &Element) -> bool {
    let uri = attr(g, "uri").map(str::to_lowercase).unwrap_or_default();
    child_local(g, "relIds", ns::DGM).is_some()
        || g.children
            .iter()
            .any(|n| matches!(n, Node::Element(e) if e.local == "relIds"))
        || uri.contains("/drawingml/2006/diagram")
}

fn chart_rel_id(g: &Element) -> Option<String> {
    for child in &g.children {
        let Node::Element(el) = child else {
            continue;
        };
        if el.local == "chart" {
            return rel_attr_value(el, "id");
        }
    }
    None
}

struct SmartArtRelIds {
    dm: Option<String>,
    lo: Option<String>,
    qs: Option<String>,
    cs: Option<String>,
}

fn smartart_rel_ids(g: &Element) -> SmartArtRelIds {
    let rel = child_local(g, "relIds", ns::DGM).or_else(|| {
        g.children.iter().find_map(|n| match n {
            Node::Element(e) if e.local == "relIds" => Some(e),
            _ => None,
        })
    });
    let Some(rel) = rel else {
        return SmartArtRelIds {
            dm: None,
            lo: None,
            qs: None,
            cs: None,
        };
    };
    SmartArtRelIds {
        dm: rel_attr_value(rel, "dm"),
        lo: rel_attr_value(rel, "lo"),
        qs: rel_attr_value(rel, "qs"),
        cs: rel_attr_value(rel, "cs"),
    }
}

fn rel_attr_value(el: &Element, local: &str) -> Option<String> {
    el.attrs
        .iter()
        .find(|a| a.local == local && a.ns.as_deref() == Some(ns::R))
        .map(|a| a.value.clone())
}

fn table_grid_pixel_extent(table: &Element) -> Option<(f64, f64)> {
    let grid = child_local(table, "tblGrid", ns::A)?;
    let w_emu: i64 = children_local(grid, "gridCol", ns::A)
        .iter()
        .map(|c| parse_emu_attr(attr(c, "w")))
        .sum();
    let h_emu: i64 = children_local(table, "tr", ns::A)
        .iter()
        .map(|r| parse_emu_attr(attr(r, "h")))
        .sum();
    if w_emu > 0 && h_emu > 0 {
        Some((emu_to_px_rounded(w_emu), emu_to_px_rounded(h_emu)))
    } else {
        None
    }
}

fn find_group_xml_offset(slide_xml: &str, c_nv_pr_id: &str) -> Option<usize> {
    if c_nv_pr_id.is_empty() {
        return None;
    }
    let mut pos = 0usize;
    while let Some(rel) = slide_xml[pos..].find("<p:grpSp") {
        let start = pos + rel;
        let c_nv_idx = slide_xml[start..].find("<p:cNvPr");
        let c_nv_idx = start + c_nv_idx?;
        let tag_end = slide_xml[c_nv_idx..]
            .find('>')
            .map(|i| c_nv_idx + i)
            .unwrap_or(slide_xml.len());
        let tag = &slide_xml[c_nv_idx..=tag_end.min(slide_xml.len() - 1)];
        if let Some(id_val) = tag.split("id=\"").nth(1).and_then(|s| s.split('"').next()) {
            if id_val == c_nv_pr_id {
                return Some(start);
            }
        }
        pos = start + 1;
    }
    None
}

fn resolve_group_xml_slice<'a>(group: &Element, raw_xml: Option<&'a str>) -> Option<&'a str> {
    let raw = raw_xml?;
    let c_nv_pr = child_path(group, &[("nvGrpSpPr", ns::P), ("cNvPr", ns::P)]);
    let id = c_nv_pr.and_then(|n| attr(n, "id"))?;
    let offset = find_group_xml_offset(raw, id)?;
    Some(&raw[offset..])
}

fn apply_raw_child_geometry(el: &mut DumpElement, child: &Element) {
    let xfrm = child_path(child, &[("spPr", ns::P), ("xfrm", ns::A)])
        .or_else(|| child_local(child, "xfrm", ns::P));
    let Some(xfrm) = xfrm else { return };
    let off = child_local(xfrm, "off", ns::A);
    let ext = child_local(xfrm, "ext", ns::A);
    if let Some(off) = off {
        let x_emu = parse_emu_attr(attr(off, "x"));
        let y_emu = parse_emu_attr(attr(off, "y"));
        el.x = emu_to_px_f64(x_emu);
        el.y = emu_to_px_f64(y_emu);
        el.x_emu = Some(x_emu);
        el.y_emu = Some(y_emu);
    }
    if let Some(ext) = ext {
        let w_emu = parse_emu_attr(attr(ext, "cx"));
        let h_emu = parse_emu_attr(attr(ext, "cy"));
        el.width = emu_to_px_f64(w_emu);
        el.height = emu_to_px_f64(h_emu);
        el.width_emu = Some(w_emu);
        el.height_emu = Some(h_emu);
    }
}

#[allow(clippy::only_used_in_recursion)]
fn parse_group(
    group: &Element,
    id: String,
    raw_xml: Option<&str>,
    id_prefix: &str,
    ctx: &ParseCtx,
    options: &SpTreeOptions,
) -> Option<DumpElement> {
    let grp_sp_pr = child_local(group, "grpSpPr", ns::P);
    let xfrm = grp_sp_pr.and_then(|p| child_local(p, "xfrm", ns::A));
    let (_, _, _, _, _, _, _, _, rot, flip_h, flip_v) = xfrm_to_box(xfrm);
    let grp_transform = read_group_transform(xfrm);
    let group_xml = resolve_group_xml_slice(group, raw_xml);
    let child_order = extract_sp_tree_child_order(group_xml, group, "p:grpSp");
    let child_id_prefix = format!("{}-", id);
    let mut children = Vec::new();
    for entry in child_order {
        let local = entry.tag.strip_prefix("p:").unwrap_or(&entry.tag);
        let nodes = children_local(group, local, ns::P);
        let node = nodes.get(entry.index_in_type)?;
        let child = match local {
            "grpSp" => parse_group(
                node,
                format!("{}-group-{}", id, entry.index_in_type),
                raw_xml,
                id_prefix,
                ctx,
                options,
            ),
            "sp" => {
                let sp_pr = child_local(node, "spPr", ns::P);
                let image_fill =
                    sp_pr.is_some_and(|sp| child_local(sp, "blipFill", ns::A).is_some());
                if image_fill {
                    parse_shape_with_image_fill(
                        node,
                        format!("{}shape-img-{}", child_id_prefix, entry.index_in_type),
                        ctx,
                        true,
                    )
                } else {
                    parse_shape(
                        node,
                        format!("{}shape-{}", child_id_prefix, entry.index_in_type),
                        ctx,
                        true,
                    )
                }
            }
            "pic" => parse_picture(
                node,
                format!("{}pic-{}", child_id_prefix, entry.index_in_type),
                ctx,
            ),
            "cxnSp" => parse_connector(
                node,
                format!("{}conn-{}", child_id_prefix, entry.index_in_type),
                ctx,
            ),
            "graphicFrame" => parse_graphic_frame(
                node,
                format!("{}frame-{}", child_id_prefix, entry.index_in_type),
                ctx,
            ),
            _ => None,
        };
        if let Some(mut c) = child {
            apply_raw_child_geometry(&mut c, node);
            transform_group_child(&mut c, &grp_transform);
            children.push(c);
        }
    }
    for child in &mut children {
        child.x -= grp_transform.parent_x;
        child.y -= grp_transform.parent_y;
    }
    let c_nv_pr = child_path(group, &[("nvGrpSpPr", ns::P), ("cNvPr", ns::P)]);
    let gx = grp_transform.parent_x.round();
    let gy = grp_transform.parent_y.round();
    let gw = grp_transform.parent_w.round();
    let gh = grp_transform.parent_h.round();
    build_dump(
        "group",
        id,
        c_nv_pr,
        None,
        gx,
        gy,
        gw,
        gh,
        grp_transform.parent_x_emu,
        grp_transform.parent_y_emu,
        grp_transform.parent_w_emu,
        grp_transform.parent_h_emu,
        rot,
        flip_h,
        flip_v,
        None,
        None,
        None,
        None,
        Some(children),
        false,
        PaintExtras::default(),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_dump(
    type_: &str,
    id: String,
    c_nv_pr: Option<&Element>,
    ph_info: Option<PlaceholderInfo>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    x_emu: i64,
    y_emu: i64,
    w_emu: i64,
    h_emu: i64,
    rotation: Option<f64>,
    flip_h: bool,
    flip_v: bool,
    inherited_transform: Option<InheritedTransform>,
    text: Option<String>,
    shape_type: Option<String>,
    table_row_count: Option<usize>,
    children: Option<Vec<DumpElement>>,
    _in_group: bool,
    extras: PaintExtras,
) -> Option<DumpElement> {
    let shape_id = c_nv_pr.and_then(|n| attr(n, "id")).map(str::to_owned);
    let name = c_nv_pr
        .and_then(|n| attr(n, "name"))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    Some(DumpElement {
        type_: type_.to_owned(),
        id,
        shape_id,
        name,
        placeholder_type: ph_info.as_ref().and_then(|p| p.type_.clone()),
        placeholder_sz: ph_info.as_ref().and_then(|p| p.sz.clone()),
        x,
        y,
        width: w,
        height: h,
        x_emu: Some(x_emu),
        y_emu: Some(y_emu),
        width_emu: Some(w_emu),
        height_emu: Some(h_emu),
        rotation,
        flip_horizontal: flip_h,
        flip_vertical: flip_v,
        inherited_transform,
        text,
        shape_type,
        table_row_count,
        children: match children {
            None => None,
            Some(c) if c.is_empty() => {
                if type_ == "group" {
                    Some(c)
                } else {
                    None
                }
            }
            Some(c) => Some(c),
        },
        fill_color: extras.fill_color,
        fill_gradient: extras.fill_gradient,
        fill_mode: extras.fill_mode,
        stroke_color: extras.stroke_color,
        stroke_width_px: extras.stroke_width_px,
        text_color: extras.text_color,
        font_size_px: extras.font_size_px,
        chart_r_id: extras.chart_r_id,
        chart_xml: None,
        smartart_dm_r_id: extras.smartart_dm_r_id,
        smartart_lo_r_id: extras.smartart_lo_r_id,
        smartart_qs_r_id: extras.smartart_qs_r_id,
        smartart_cs_r_id: extras.smartart_cs_r_id,
        smartart_drawing_path: None,
        smartart_drawing_xml: None,
        seq: 0,
    })
}

type XfrmBox = (
    f64,
    f64,
    f64,
    f64,
    i64,
    i64,
    i64,
    i64,
    Option<f64>,
    bool,
    bool,
);

fn xfrm_to_box(xfrm: Option<&Element>) -> XfrmBox {
    let Some(parsed) = parse_transform(xfrm) else {
        return (0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0, None, false, false);
    };
    let x_emu = parsed.frame.x;
    let y_emu = parsed.frame.y;
    let w_emu = parsed.frame.width;
    let h_emu = parsed.frame.height;
    (
        emu_to_px_rounded(x_emu),
        emu_to_px_rounded(y_emu),
        emu_to_px_rounded(w_emu),
        emu_to_px_rounded(h_emu),
        x_emu,
        y_emu,
        w_emu,
        h_emu,
        parsed.rotation,
        parsed.flip_horizontal.unwrap_or(false),
        parsed.flip_vertical.unwrap_or(false),
    )
}

fn geometry_name(sp_pr: Option<&Element>) -> String {
    parse_drawing_geometry(sp_pr).geometry
}

fn emu_to_px_rounded(emu: i64) -> f64 {
    emu_to_pixels(emu).round()
}

fn emu_to_px_f64(emu: i64) -> f64 {
    emu_to_pixels(emu)
}

pub fn emu_to_px_helper(emu: i64) -> i64 {
    emu_to_px_rounded(emu) as i64
}

fn coord_json(v: f64) -> Value {
    if v.fract() == 0.0 && v.is_finite() {
        json!(v as i64)
    } else {
        json!(v)
    }
}

fn parse_emu_attr(v: Option<&str>) -> i64 {
    v.and_then(|s| s.parse().ok()).unwrap_or(0)
}

fn element_has_element_children(el: &Element) -> bool {
    el.children.iter().any(|n| matches!(n, Node::Element(_)))
}

fn shape_element_has_visible_style(
    shape: &Element,
    sp_pr: Option<&Element>,
    inherited_shape: Option<&Element>,
) -> bool {
    // Mirrors upstream `extractShapeStyle` + `hasVisibleStyle`: `a:noFill` /
    // `a:ln/a:noFill` win over `p:style` fillRef/lnRef (master title placeholders).
    let fill_visible = match sp_pr {
        Some(sp) if child_local(sp, "noFill", ns::A).is_some() => false,
        Some(sp)
            if child_local(sp, "solidFill", ns::A).is_some()
                || child_local(sp, "gradFill", ns::A).is_some()
                || child_local(sp, "pattFill", ns::A).is_some()
                || child_local(sp, "blipFill", ns::A).is_some() =>
        {
            true
        }
        _ => style_ref_idx_positive(shape, inherited_shape, "fillRef"),
    };
    // Upstream gates on resolved `strokeWidth > 0` only (not mere solidFill on ln).
    let stroke_visible = match sp_pr.and_then(|sp| child_local(sp, "ln", ns::A)) {
        Some(ln) if child_local(ln, "noFill", ns::A).is_some() => false,
        Some(ln) => parse_emu_attr(attr(ln, "w")) > 0,
        None => style_ref_idx_positive(shape, inherited_shape, "lnRef"),
    };
    fill_visible || stroke_visible
}

fn style_ref_idx_positive(shape: &Element, inherited_shape: Option<&Element>, local: &str) -> bool {
    let style = child_local(shape, "style", ns::P)
        .or_else(|| inherited_shape.and_then(|el| child_local(el, "style", ns::P)));
    let Some(style) = style else {
        return false;
    };
    let Some(ref_el) = child_local(style, local, ns::A) else {
        return false;
    };
    let idx = attr(ref_el, "idx")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    // idx 0 / 1000 mean "none" in the style matrix (same as upstream).
    idx > 0 && idx != 1000
}

pub fn text_body_has_content(tx_body: Option<&Element>) -> bool {
    tx_body.is_some_and(|body| {
        let ctx = TextExtractContext {
            tx_body: Some(body),
            inherited_tx_body: None,
            is_body_placeholder: false,
            effective_level_styles: None,
            presentation_level_styles: None,
        };
        text_body_has_renderable_content(body, &ctx)
    })
}

pub(crate) fn extract_text_body(tx_body: &Element) -> String {
    let ctx = TextExtractContext {
        tx_body: Some(tx_body),
        inherited_tx_body: None,
        is_body_placeholder: false,
        effective_level_styles: None,
        presentation_level_styles: None,
    };
    extract_element_text(&ctx, tx_body)
}

fn replace_child(parent: &mut Element, local: &str, ns_uri: &str, new_child: Element) {
    if let Some(idx) = parent.children.iter().position(
        |n| matches!(n, Node::Element(e) if e.local == local && e.ns.as_deref() == Some(ns_uri)),
    ) {
        parent.children[idx] = Node::Element(new_child);
    } else {
        parent.children.push(Node::Element(new_child));
    }
}

pub fn slim_element_for_dump(el: &DumpElement) -> Value {
    let mut out = json!({
        "type": el.type_,
        "id": el.id,
    });
    let obj = out.as_object_mut().unwrap();
    obj.insert("x".to_owned(), coord_json(el.x));
    obj.insert("y".to_owned(), coord_json(el.y));
    obj.insert("width".to_owned(), coord_json(el.width));
    obj.insert("height".to_owned(), coord_json(el.height));
    if let Some(v) = &el.shape_id {
        obj.insert("shapeId".to_owned(), json!(v));
    }
    if let Some(v) = &el.name {
        obj.insert("name".to_owned(), json!(v));
    }
    if let Some(v) = &el.placeholder_type {
        obj.insert("placeholderType".to_owned(), json!(v));
    }
    if let Some(v) = &el.placeholder_sz {
        obj.insert("placeholderSz".to_owned(), json!(v));
    }
    if let Some(v) = el.x_emu {
        obj.insert("xEmu".to_owned(), json!(v));
    }
    if let Some(v) = el.y_emu {
        obj.insert("yEmu".to_owned(), json!(v));
    }
    if let Some(v) = el.width_emu {
        obj.insert("widthEmu".to_owned(), json!(v));
    }
    if let Some(v) = el.height_emu {
        obj.insert("heightEmu".to_owned(), json!(v));
    }
    if let Some(v) = el.rotation {
        obj.insert("rotation".to_owned(), json!(v));
    }
    if el.flip_horizontal {
        obj.insert("flipHorizontal".to_owned(), json!(true));
    }
    if el.flip_vertical {
        obj.insert("flipVertical".to_owned(), json!(true));
    }
    if let Some(inh) = &el.inherited_transform {
        let mut i = json!({});
        let io = i.as_object_mut().unwrap();
        io.insert("x".to_owned(), coord_json(inh.x));
        io.insert("y".to_owned(), coord_json(inh.y));
        io.insert("width".to_owned(), coord_json(inh.width));
        io.insert("height".to_owned(), coord_json(inh.height));
        if let Some(r) = inh.rotation {
            io.insert("rotation".to_owned(), json!(r));
        }
        io.insert("flipHorizontal".to_owned(), json!(inh.flip_horizontal));
        io.insert("flipVertical".to_owned(), json!(inh.flip_vertical));
        obj.insert("inheritedTransform".to_owned(), i);
    }
    if let Some(t) = &el.text {
        obj.insert("text".to_owned(), json!(t));
    }
    if let Some(st) = &el.shape_type {
        obj.insert("shapeType".to_owned(), json!(st));
    }
    if let Some(kids) = &el.children {
        let slim_kids = kids
            .iter()
            .filter(|c| PORTED.contains(&c.type_.as_str()))
            .map(slim_element_for_dump)
            .collect::<Vec<_>>();
        if !slim_kids.is_empty() || el.type_ == "group" {
            obj.insert("children".to_owned(), json!(slim_kids));
        }
    }
    out
}

/// Issue-24 dump split: ported kinds stay elements; chart/smartArt and other
/// kinds stay `otherKinds`, in document order (`seq`).
pub fn issue24_dump_parts(
    elements: &[DumpElement],
    other_kind_seq: &[(u32, String)],
) -> (Vec<Value>, Vec<String>) {
    enum Slot<'a> {
        El(&'a DumpElement),
        Other(&'a str),
    }
    let mut slots: Vec<(u32, Slot<'_>)> = Vec::new();
    for el in elements {
        slots.push((el.seq, Slot::El(el)));
    }
    for (seq, kind) in other_kind_seq {
        slots.push((*seq, Slot::Other(kind.as_str())));
    }
    slots.sort_by_key(|(seq, _)| *seq);
    let mut dumped = Vec::new();
    let mut kinds = Vec::new();
    for (_, slot) in slots {
        match slot {
            Slot::El(el) if PORTED.contains(&el.type_.as_str()) => {
                dumped.push(slim_element_for_dump(el));
            }
            Slot::El(el) => kinds.push(el.type_.clone()),
            Slot::Other(kind) => kinds.push(kind.to_owned()),
        }
    }
    (dumped, kinds)
}

#[derive(Default)]
struct PaintExtras {
    fill_color: Option<String>,
    fill_gradient: Option<String>,
    fill_mode: Option<String>,
    stroke_color: Option<String>,
    stroke_width_px: Option<f64>,
    text_color: Option<String>,
    font_size_px: Option<f64>,
    chart_r_id: Option<String>,
    smartart_dm_r_id: Option<String>,
    smartart_lo_r_id: Option<String>,
    smartart_qs_r_id: Option<String>,
    smartart_cs_r_id: Option<String>,
}

struct SchemeTheme<'a>(&'a BTreeMap<String, String>);

impl DrawingColorTheme for SchemeTheme<'_> {
    fn scheme(&self, name: &str) -> Option<String> {
        let slot = match name {
            "bg1" => "lt1",
            "tx1" => "dk1",
            "bg2" => "lt2",
            "tx2" => "dk2",
            other => other,
        };
        self.0
            .get(slot)
            .cloned()
            .or_else(|| self.0.get(name).cloned())
    }
}

/// First paintable `p:cSld/p:bg` in `chain` (slide, then layout, then master).
pub fn first_paintable_background(
    chain: &[Option<&Element>],
    theme: &BTreeMap<String, String>,
) -> (Option<String>, Option<String>) {
    for part in chain.iter().flatten() {
        if let Some(paint) = background_of_c_sld(part, theme) {
            return paint;
        }
    }
    (None, None)
}

fn background_of_c_sld(
    c_sld: &Element,
    theme: &BTreeMap<String, String>,
) -> Option<(Option<String>, Option<String>)> {
    let bg = child_local(c_sld, "bg", ns::P)?;
    if let Some(bg_pr) = child_local(bg, "bgPr", ns::P) {
        match parse_drawing_fill(Some(bg_pr)) {
            Some(DrawingFill::Solid { color }) => {
                let css = css_color(&color, theme, true)?;
                return Some((Some(css), None));
            }
            Some(ref fill @ DrawingFill::Gradient { .. }) => {
                if let Some(gradient) = linear_gradient_css(fill, theme) {
                    let DrawingFill::Gradient { stops, .. } = fill else {
                        return None;
                    };
                    let flat = stops.first().and_then(|s| css_color(&s.color, theme, true));
                    return Some((flat, Some(gradient)));
                }
            }
            _ => {}
        }
    }
    // Pragmatic theme background: `p:bgRef` schemeClr when bgPr has no solid/linear fill.
    let bg_ref = child_local(bg, "bgRef", ns::P)?;
    let color = parse_drawing_color_in(Some(bg_ref))?;
    let css = css_color(&color, theme, true)?;
    Some((Some(css), None))
}

fn shape_paint_extras(
    shape: &Element,
    effective_sp_pr: Option<&Element>,
    inherited_shape: Option<&Element>,
    tx_body: Option<&Element>,
    ctx: &ParseCtx,
) -> PaintExtras {
    let fallback = crate::theme::default_scheme_color_map();
    let theme = ctx.theme_colors.unwrap_or(&fallback);
    let mut extras = PaintExtras::default();
    match effective_sp_pr.and_then(|sp| parse_drawing_fill(Some(sp))) {
        Some(DrawingFill::None) => extras.fill_mode = Some("none".to_owned()),
        Some(DrawingFill::Solid { color }) => {
            extras.fill_mode = Some("solid".to_owned());
            extras.fill_color = css_color(&color, theme, false);
        }
        Some(ref fill @ DrawingFill::Gradient { .. }) => {
            extras.fill_mode = Some("gradient".to_owned());
            extras.fill_gradient = linear_gradient_css(fill, theme);
        }
        Some(_) | None => {}
    }
    if extras.fill_mode.is_none() {
        if let Some(css) = style_ref_color(shape, inherited_shape, "fillRef", theme) {
            extras.fill_mode = Some("solid".to_owned());
            extras.fill_color = Some(css);
        }
    }
    let mut ln_blocks_ref = false;
    if let Some(ln) = effective_sp_pr.and_then(|sp| child_local(sp, "ln", ns::A)) {
        if child_local(ln, "noFill", ns::A).is_some() {
            ln_blocks_ref = true;
        }
        if let Some(line) = parse_drawing_line(Some(ln)) {
            if let Some(width) = line.width_emu {
                if width > 0 {
                    extras.stroke_width_px = Some(emu_to_pixels(width));
                }
            }
            if let Some(DrawingFill::Solid { color }) = line.fill {
                extras.stroke_color = css_color(&color, theme, false);
            }
        }
    }
    if extras.stroke_color.is_none() && !ln_blocks_ref {
        if let Some(css) = style_ref_color(shape, inherited_shape, "lnRef", theme) {
            extras.stroke_color = Some(css);
            if extras.stroke_width_px.is_none() {
                extras.stroke_width_px = Some(1.0);
            }
        }
    }
    if let Some(tx) = tx_body {
        let (color, size) = first_run_text_paint(tx, theme);
        extras.text_color = color;
        extras.font_size_px = size;
    }
    extras
}

fn style_ref_color(
    shape: &Element,
    inherited_shape: Option<&Element>,
    local: &str,
    theme: &BTreeMap<String, String>,
) -> Option<String> {
    let style = child_local(shape, "style", ns::P)
        .or_else(|| inherited_shape.and_then(|el| child_local(el, "style", ns::P)))?;
    let ref_el = child_local(style, local, ns::A)?;
    let idx = attr(ref_el, "idx")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    if idx <= 0 || idx == 1000 {
        return None;
    }
    let color = parse_drawing_color_in(Some(ref_el))?;
    css_color(&color, theme, false)
}

fn first_run_text_paint(
    tx: &Element,
    theme: &BTreeMap<String, String>,
) -> (Option<String>, Option<f64>) {
    let mut color = None;
    let mut size = None;
    for para in children_local(tx, "p", ns::A) {
        let def = child_path(para, &[("pPr", ns::A), ("defRPr", ns::A)]);
        let runs: Vec<&Element> = para
            .children
            .iter()
            .filter_map(|n| match n {
                Node::Element(el) if el.local == "r" || el.local == "fld" => Some(el),
                _ => None,
            })
            .collect();
        if runs.is_empty() {
            apply_rpr(def, &mut color, &mut size, theme);
            if color.is_some() && size.is_some() {
                break;
            }
            continue;
        }
        let run = runs[0];
        let rpr = child_local(run, "rPr", ns::A);
        if color.is_none() {
            color = rpr
                .and_then(|r| solid_css(r, theme))
                .or_else(|| def.and_then(|r| solid_css(r, theme)));
        }
        if size.is_none() {
            size = rpr.and_then(sz_to_px).or_else(|| def.and_then(sz_to_px));
        }
        break;
    }
    (color, size)
}

fn apply_rpr(
    rpr: Option<&Element>,
    color: &mut Option<String>,
    size: &mut Option<f64>,
    theme: &BTreeMap<String, String>,
) {
    let Some(rpr) = rpr else {
        return;
    };
    if color.is_none() {
        *color = solid_css(rpr, theme);
    }
    if size.is_none() {
        *size = sz_to_px(rpr);
    }
}

fn solid_css(rpr: &Element, theme: &BTreeMap<String, String>) -> Option<String> {
    let fill = child_local(rpr, "solidFill", ns::A)?;
    let color = parse_drawing_color_in(Some(fill))?;
    css_color(&color, theme, false)
}

fn sz_to_px(rpr: &Element) -> Option<f64> {
    let sz = attr(rpr, "sz")?.parse::<f64>().ok()?;
    if sz <= 0.0 {
        return None;
    }
    Some(sz / 100.0 * 96.0 / 72.0)
}

fn css_color(
    color: &DrawingColor,
    theme: &BTreeMap<String, String>,
    blend_white: bool,
) -> Option<String> {
    let resolved = resolve_drawing_color(color, Some(&SchemeTheme(theme)), None)?;
    if resolved.alpha >= 0.999 {
        return Some(resolved.hex);
    }
    let (r, g, b) = hex_to_rgb_channels(&resolved.hex)?;
    if blend_white {
        let mix = |c: f64| (c * resolved.alpha + 255.0 * (1.0 - resolved.alpha)).round() as u8;
        return Some(format!("#{:02X}{:02X}{:02X}", mix(r), mix(g), mix(b)));
    }
    let alpha = if (resolved.alpha * 100.0).round() / 100.0 == resolved.alpha {
        format!("{}", resolved.alpha)
    } else {
        format!("{:.2}", resolved.alpha)
    };
    Some(format!(
        "rgba({}, {}, {}, {alpha})",
        r.round() as u8,
        g.round() as u8,
        b.round() as u8
    ))
}

fn linear_gradient_css(fill: &DrawingFill, theme: &BTreeMap<String, String>) -> Option<String> {
    let DrawingFill::Gradient {
        stops, angle, path, ..
    } = fill
    else {
        return None;
    };
    if path.is_some() && angle.is_none() {
        return None;
    }
    if stops.is_empty() {
        return None;
    }
    let ooxml_deg = angle.unwrap_or(90.0);
    let css_deg = norm_deg(ooxml_deg + 90.0);
    let mut parts = Vec::new();
    for stop in stops {
        let color = css_color(&stop.color, theme, false)?;
        parts.push(format!("{color} {}%", format_pct(stop.position)));
    }
    Some(format!(
        "linear-gradient({css_deg:.2}deg, {})",
        parts.join(", ")
    ))
}

fn format_pct(position: f64) -> String {
    if (position - position.round()).abs() < 0.05 {
        format!("{}", position.round() as i64)
    } else {
        let s = format!("{position:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

fn norm_deg(deg: f64) -> f64 {
    if !deg.is_finite() {
        return 0.0;
    }
    ((deg % 360.0) + 360.0) % 360.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::xml::parse_xml_str;

    fn theme() -> BTreeMap<String, String> {
        crate::theme::default_scheme_color_map()
    }

    fn ctx<'a>(theme: &'a BTreeMap<String, String>) -> ParseCtx<'a> {
        ParseCtx {
            slide_path: "ppt/slides/slide1.xml",
            layout_sp_tree: None,
            master_sp_tree: None,
            presentation_level_styles: None,
            layout_placeholder_levels: None,
            master_placeholder_levels: None,
            master_tx_styles: None,
            theme_colors: Some(theme),
        }
    }

    // Catches: OOXML ang=0 (left-to-right) passed straight into CSS as 0deg.
    #[test]
    fn linear_fill_angle_zero_is_css_90() {
        let xml = r#"<p:cSld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:bg><p:bgPr><a:gradFill>
            <a:gsLst>
              <a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs>
              <a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs>
            </a:gsLst>
            <a:lin ang="0" scaled="0"/>
          </a:gradFill></p:bgPr></p:bg>
        </p:cSld>"#;
        let doc = parse_xml_str(xml).unwrap();
        let (color, gradient) = first_paintable_background(&[Some(&doc.root)], &theme());
        assert_eq!(color.as_deref(), Some("#FF0000"));
        assert_eq!(
            gradient.as_deref(),
            Some("linear-gradient(90.00deg, #FF0000 0%, #0000FF 100%)")
        );
    }

    // Catches: an empty slide background hiding a layout solid fill.
    #[test]
    fn background_falls_through_to_layout_solid() {
        let slide = parse_xml_str(
            r#"<p:cSld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"/>"#,
        )
        .unwrap();
        let layout = parse_xml_str(
            r#"<p:cSld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <p:bg><p:bgPr><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></p:bgPr></p:bg>
            </p:cSld>"#,
        )
        .unwrap();
        let (color, gradient) =
            first_paintable_background(&[Some(&slide.root), Some(&layout.root)], &theme());
        assert_eq!(color.as_deref(), Some("#00FF00"));
        assert!(gradient.is_none());
    }

    // Catches: master a:noFill losing to p:style fillRef accent.
    #[test]
    fn no_fill_suppresses_fill_ref() {
        let xml = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="1" name="S"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr>
            <a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm>
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
            <a:noFill/>
          </p:spPr>
          <p:style><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef></p:style>
        </p:sp>"#;
        let doc = parse_xml_str(xml).unwrap();
        let colors = theme();
        let el = parse_shape(&doc.root, "s".into(), &ctx(&colors), false).unwrap();
        assert_eq!(el.fill_mode.as_deref(), Some("none"));
        assert!(el.fill_color.is_none());
    }

    // Catches: fillRef schemeClr left unpainted when spPr has no direct fill.
    #[test]
    fn fill_ref_scheme_is_solid() {
        let xml = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="2" name="R"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr>
            <a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm>
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          </p:spPr>
          <p:style><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef></p:style>
          <p:txBody>
            <a:bodyPr/>
            <a:p><a:r><a:rPr sz="1400"><a:solidFill><a:srgbClr val="112233"/></a:solidFill></a:rPr><a:t>Hi</a:t></a:r></a:p>
          </p:txBody>
        </p:sp>"#;
        let doc = parse_xml_str(xml).unwrap();
        let colors = theme();
        let el = parse_shape(&doc.root, "s".into(), &ctx(&colors), false).unwrap();
        assert_eq!(el.fill_mode.as_deref(), Some("solid"));
        assert_eq!(el.fill_color.as_deref(), Some("#4472C4"));
        assert_eq!(el.text_color.as_deref(), Some("#112233"));
        assert!((el.font_size_px.unwrap() - 14.0 * 96.0 / 72.0).abs() < 0.01);
    }

    // Catches: c:chart graphic frames dumped as otherKinds instead of chart elements.
    #[test]
    fn chart_frame_is_element_with_rid() {
        let xml = r#"<p:graphicFrame xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
          <p:nvGraphicFramePr><p:cNvPr id="3" name="Chart"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>
          <p:xfrm><a:off x="0" y="0"/><a:ext cx="1828800" cy="1828800"/></p:xfrm>
          <a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart">
            <c:chart r:id="rId5"/>
          </a:graphicData></a:graphic>
        </p:graphicFrame>"#;
        let doc = parse_xml_str(xml).unwrap();
        let colors = theme();
        let el = parse_graphic_frame(&doc.root, "c".into(), &ctx(&colors)).unwrap();
        assert_eq!(el.type_, "chart");
        assert_eq!(el.chart_r_id.as_deref(), Some("rId5"));
        assert!(el.x > 0.0 || el.width > 0.0);
    }
}
