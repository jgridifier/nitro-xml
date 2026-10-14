// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/PptxHandlerRuntimeSpTreeParsing.ts,
// PptxHandlerRuntimeShapeParsing.ts, PptxHandlerRuntimePictureParsing.ts, PptxHandlerRuntimeGroupParsing.ts,
// PptxGraphicFrameParser.ts, PptxHandlerRuntimeBackgroundParsing.ts and
// src/core/pptx/core/utils/vml-ole-preview.ts; changes in PROVENANCE.md.

use std::collections::BTreeMap;

use nitro_xml_opc::drawingml::{
    emu_to_pixels, hex_to_rgb_channels, parse_drawing_color_in, parse_drawing_fill,
    parse_drawing_geometry, parse_drawing_line, parse_transform, resolve_drawing_color,
    theme_color_slot_for, DrawingColor, DrawingColorTheme, DrawingFill, DrawingLine, ThemeColorMap,
    ThemeFormatScheme, EMU_PER_PIXEL,
};
use nitro_xml_opc::xml::{ns, Element, Node};
use serde::Serialize;
use serde_json::{json, Value};

use crate::inheritance::{
    attr, child_local, child_path, children_local, extract_placeholder_info,
    find_placeholder_in_sp_tree, find_singleton_placeholder_in_sp_tree,
    is_header_footer_placeholder, merge_elements, merge_placeholder_context,
    parse_optional_bool_attr, sp_pr_of_placeholder, tx_body_of_placeholder, PlaceholderInfo,
    PlaceholderNode,
};
use crate::level_style::{
    extract_lst_style_from_tx_body, level_style_for, merge_level_styles,
    presentation_default_ea_line_break, LevelStylesMap, MasterTxStyles, PlaceholderTextLevelStyle,
};
use crate::placeholder_defaults::PlaceholderDefaultsCache;
use crate::table::{parse_table_model, table_row_count};
use crate::table_style::TableStyleMap;
use crate::text::{
    extract_element_text, placeholder_is_body_type, text_body_has_renderable_content,
    TextExtractContext,
};
use crate::theme::ThemeFontScheme;

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
    /// Parsed `a:tbl` cells. Not part of the issue-24 dump.
    #[serde(skip)]
    pub(crate) table_data: Option<crate::table::TableData>,
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
    /// `a:ln/@algn` (`in` / `ctr`); omitted means centred (`ctr`). Paint only.
    #[serde(skip)]
    pub stroke_line_align: Option<String>,
    /// `a:ln/a:prstDash/@val` (paint only).
    #[serde(skip)]
    pub stroke_dash: Option<String>,
    /// `a:headEnd/@type` on connector line (paint only).
    #[serde(skip)]
    pub connector_start_arrow: Option<String>,
    /// `a:tailEnd/@type` on connector line (paint only).
    #[serde(skip)]
    pub connector_end_arrow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size_px: Option<f64>,
    /// First-run `a:latin/@typeface` or `p:style/a:fontRef` theme latin (paint only).
    #[serde(skip)]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub font_bold: bool,
    #[serde(skip)]
    pub font_italic: bool,
    /// `a:rPr/a:ln/@w` in CSS px (paint only).
    #[serde(skip)]
    pub text_stroke_width_px: Option<f64>,
    #[serde(skip)]
    pub text_stroke_color: Option<String>,
    /// Parsed `p:txBody` anchor / alignment for painters (not in JSON dump).
    #[serde(skip)]
    pub text_body_layout: Option<crate::text_layout::TextBodyLayout>,
    /// Per-paragraph BiDi + runs for painters (not in JSON dump).
    #[serde(skip)]
    pub text_paint_paragraphs: Option<Vec<crate::text_layout::TextPaintParagraph>>,
    /// Strict kinsoku CSS when the deck defines `p:defaultTextStyle/@eaLnBrk` (paint only).
    #[serde(skip)]
    pub presentation_kinsoku: bool,
    /// `a:spAutoFit` / `a:normAutofit` / `a:noAutofit` from `a:bodyPr` (paint only).
    #[serde(skip)]
    pub auto_fit_mode: Option<crate::text_layout::TextAutoFitMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart_r_id: Option<String>,
    /// Chart part XML. Not part of the issue-24 dump.
    #[serde(skip)]
    pub chart_xml: Option<String>,
    /// Package path of the chart part (for resolving chart `a:blipFill` rels).
    #[serde(skip)]
    pub chart_part_path: Option<String>,
    /// Resolved `data:` URLs for chart `a:blipFill` relationship ids.
    #[serde(skip)]
    pub chart_image_urls: Option<std::collections::HashMap<String, String>>,
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
    /// Diagram data part XML (`dgm:dataModel`) for presentation label colours.
    #[serde(skip)]
    pub smartart_data_xml: Option<String>,
    /// `r:embed` on a picture or shape blip fill, or an OLE preview relationship.
    /// Resolved to [`Self::image_src`].
    #[serde(skip)]
    pub image_rel_id: Option<String>,
    /// Part that owns [`Self::image_rel_id`] when it is not the slide (a VML drawing).
    #[serde(skip)]
    pub image_rel_owner: Option<String>,
    /// `p:oleObj/@spid`, matched to a `v:shape/@id` in the slide's VML drawing.
    #[serde(skip)]
    pub ole_spid: Option<String>,
    /// Video/audio picture: the poster still paints, but the issue-24 dump reports `media`.
    #[serde(skip)]
    pub is_media: bool,
    /// `a:videoFile` / `p:video` relationship id (paint only).
    #[serde(skip)]
    pub media_rel_id: Option<String>,
    /// Resolved `data:` URL for [`Self::media_rel_id`].
    #[serde(skip)]
    pub media_src: Option<String>,
    /// `video` or `audio` when [`Self::is_media`].
    #[serde(skip)]
    pub media_type: Option<String>,
    /// `data:` URL for the resolved blip (img `src` / CSS background). Not in the issue-24 dump.
    #[serde(skip)]
    pub image_src: Option<String>,
    /// Document order of this element among emitted spTree children (dump merge).
    #[serde(skip)]
    pub seq: u32,
    /// CSS `box-shadow` from `a:effectLst/a:outerShdw`. Not in the issue-24 dump.
    #[serde(skip)]
    pub box_shadow: Option<String>,
    /// Inset specular `box-shadow` from theme `effectStyleLst/a:sp3d/@prstMaterial`. Not in the issue-24 dump.
    #[serde(skip)]
    pub material_box_shadow: Option<String>,
    /// Material gradient overlay (`background-image` layer). Not in the issue-24 dump.
    #[serde(skip)]
    pub material_overlay: Option<String>,
    /// CSS `text-shadow` from first-run `a:rPr/a:effectLst/a:outerShdw`. Not in the issue-24 dump.
    #[serde(skip)]
    pub text_shadow: Option<String>,
    /// CSS `filter` from `a:effectLst/a:glow` (and legacy metal+bevel compensation). Not in the issue-24 dump.
    #[serde(skip)]
    pub effect_filter: Option<String>,
    /// Draw-tab / `aink` ink strokes (paint only).
    #[serde(skip)]
    pub ink_data: Option<crate::ink::InkData>,
    /// `p:contentPart/@r:id` → InkML part (resolved in presentation load).
    #[serde(skip)]
    pub content_part_ink_rel_id: Option<String>,
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
    /// Master `p:clrMap`, or a layout/slide `a:overrideClrMapping` that replaces it.
    /// Missing keys fall back to `bg1→lt1`, `tx1→dk1`, `bg2→lt2`, `tx2→dk2`.
    pub color_map: Option<&'a ThemeColorMap>,
    /// Theme `a:fmtScheme` used by `a:fillRef` / `a:lnRef` / `a:effectRef` (idx 1–3 and 1001–1003).
    pub format_scheme: Option<&'a ThemeFormatScheme>,
    pub font_scheme: Option<&'a ThemeFontScheme>,
    pub table_style_map: Option<&'a TableStyleMap>,
}

impl<'a> ParseCtx<'a> {
    fn extract_text(
        &self,
        own_tx_body: Option<&Element>,
        inherited_tx_body: Option<&Element>,
        effective_tx_body: &Element,
        ph_info: Option<&PlaceholderInfo>,
    ) -> String {
        let effective_levels = ph_info.map(|ph| self.effective_level_styles(ph));
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

    fn effective_level_styles(&self, ph: &PlaceholderInfo) -> LevelStylesMap {
        let key = crate::placeholder_defaults::placeholder_defaults_key(ph);
        let layout_levels = self.layout_placeholder_levels.and_then(|c| c.get(&key));
        let master_levels = self.master_placeholder_levels.and_then(|c| c.get(&key));
        crate::placeholder_defaults::lookup_effective_level_styles(
            layout_levels,
            master_levels,
            self.master_tx_styles,
            ph.type_.as_deref(),
            self.presentation_level_styles,
        )
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
        "pic" => parse_picture(
            node,
            sp_tree_child_id(id_prefix, "pic", entry.index_in_type, options),
            ctx,
        )
        .map(|mut el| {
            // Poster `a:blip` still paints; the issue-24 dump keeps kind `media`.
            el.is_media = picture_is_media(node);
            SpTreeParseOutcome::Element(el)
        }),
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
        "contentPart" => parse_content_part(
            node,
            sp_tree_child_id(id_prefix, "contentPart", entry.index_in_type, options),
        )
        .map(SpTreeParseOutcome::Element),
        _ => None,
    }
}

fn picture_is_media(pic: &Element) -> bool {
    let nv_pr = child_path(pic, &[("nvPicPr", ns::P), ("nvPr", ns::P)]);
    drawing_nvpr_has_media(nv_pr)
}

fn media_reference_from_nv_pr(nv_pr: &Element) -> Option<(String, String)> {
    for child in &nv_pr.children {
        let Node::Element(el) = child else {
            continue;
        };
        let ns_uri = el.ns.as_deref();
        let local = el.local.as_str();
        let media_type = if ns_uri == Some(ns::P) && (local == "video" || local == "audio") {
            local.to_owned()
        } else if ns_uri == Some(ns::A)
            && matches!(
                local,
                "videoFile" | "audioFile" | "wavAudioFile" | "quickTimeFile" | "audioCd"
            )
        {
            if matches!(local, "videoFile" | "quickTimeFile") {
                "video".to_owned()
            } else {
                "audio".to_owned()
            }
        } else {
            continue;
        };
        let rel_id = relationship_id_from_element(el);
        if let Some(id) = rel_id {
            return Some((id, media_type));
        }
    }
    if nv_pr_has_p14_media(nv_pr) {
        let ext_lst = child_local(nv_pr, "extLst", ns::P)?;
        for ext in children_local(ext_lst, "ext", ns::P) {
            for child in &ext.children {
                let Node::Element(el) = child else {
                    continue;
                };
                if el.local == "media" {
                    if let Some(id) = relationship_id_from_element(el) {
                        return Some((id, "video".to_owned()));
                    }
                }
            }
        }
    }
    None
}

fn relationship_id_from_element(el: &Element) -> Option<String> {
    for a in &el.attrs {
        if (a.local == "embed" || a.local == "link")
            && (a.ns.is_none() || a.ns.as_deref() == Some(ns::R))
            && !a.value.is_empty()
        {
            return Some(a.value.clone());
        }
    }
    None
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
    let mut extras = shape_paint_extras(
        shape,
        effective_sp_pr.as_ref(),
        inherited.as_ref().map(|p| &p.element),
        tx_body,
        ph_info.as_ref(),
        ctx,
        rot,
    );
    apply_norm_autofit_to_extras(&mut extras, own_tx, inh_tx, h);
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
        PaintExtras {
            image_rel_id: effective_sp_pr
                .as_ref()
                .and_then(blip_embed_id)
                .or_else(|| sp_pr.and_then(blip_embed_id)),
            ..PaintExtras::default()
        },
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
    let fallback = crate::theme::default_scheme_color_map();
    let theme = SchemeTheme {
        colors: ctx.theme_colors.unwrap_or(&fallback),
        map: ctx.color_map,
    };
    let mut extras = PaintExtras {
        image_rel_id: blip_embed_id(pic),
        ..PaintExtras::default()
    };
    if let Some(nv) = nv_pr {
        if let Some((rel_id, media_type)) = media_reference_from_nv_pr(nv) {
            extras.media_rel_id = Some(rel_id);
            extras.media_type = Some(media_type);
        }
    }
    extras.box_shadow = crate::outer_shadow::box_shadow_from_sp_pr(&effective_sp_pr, &theme, rot);
    extras.effect_filter = crate::outer_shadow::effect_filter_from_sp_pr(&effective_sp_pr, &theme);
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
        extras,
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
        shape_paint_extras(cxn, Some(sp_pr), None, tx_body, None, ctx, None),
    )
}

fn child_any_local<'a>(parent: &'a Element, local: &str) -> Option<&'a Element> {
    parent.children.iter().find_map(|n| match n {
        Node::Element(e) if e.local == local => Some(e),
        _ => None,
    })
}

fn relationship_id_attr(el: &Element) -> Option<String> {
    el.attrs
        .iter()
        .find(|a| a.local == "id" && a.ns.as_deref() == Some(ns::R))
        .map(|a| a.value.clone())
        .filter(|s| !s.is_empty())
}

fn parse_content_part(node: &Element, id: String) -> Option<DumpElement> {
    let xfrm = child_any_local(node, "xfrm");
    let (x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h, flip_v) = xfrm_to_box(xfrm);
    let width = if w > 0.0 { w } else { 120.0 };
    let height = if h > 0.0 { h } else { 80.0 };
    let w_emu = if w_emu > 0 {
        w_emu
    } else {
        (width * EMU_PER_PX as f64).round() as i64
    };
    let h_emu = if h_emu > 0 {
        h_emu
    } else {
        (height * EMU_PER_PX as f64).round() as i64
    };
    let nv_pr = child_any_local(node, "nvContentPartPr");
    let c_nv_pr = nv_pr.and_then(|nv| child_any_local(nv, "cNvPr"));
    let ink_rel = relationship_id_attr(node);
    let extras = PaintExtras {
        content_part_ink_rel_id: ink_rel,
        ..PaintExtras::default()
    };
    build_dump(
        "contentPart",
        id,
        c_nv_pr,
        None,
        x,
        y,
        width,
        height,
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
        None,
        None,
        false,
        extras,
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

fn parse_graphic_frame(frame: &Element, id: String, ctx: &ParseCtx) -> Option<DumpElement> {
    let nv_pr = child_path(frame, &[("nvGraphicFramePr", ns::P), ("nvPr", ns::P)]);
    let ph_info = extract_placeholder_info(nv_pr);
    let xfrm = child_local(frame, "xfrm", ns::P).or_else(|| child_local(frame, "xfrm", ns::A));
    let graphic = child_path(frame, &[("graphic", ns::A), ("graphicData", ns::A)])?;
    let c_nv_pr = child_path(frame, &[("nvGraphicFramePr", ns::P), ("cNvPr", ns::P)]);
    if graphic_is_ole(graphic) {
        let (x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h, flip_v) = xfrm_to_box(xfrm);
        let ole = find_local_deep(graphic, "oleObj");
        let spid = ole.and_then(|o| attr(o, "spid")).map(str::to_owned);
        // Modern `p:pic/a:blip` or an inline `v:imagedata` (slide-relative).
        // A separate vmlDrawing part is attached later by spid.
        let image_rel_id = deep_blip_embed(graphic).or_else(|| first_imagedata_rel(graphic));
        return build_dump(
            "ole",
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
            None,
            None,
            false,
            PaintExtras {
                image_rel_id,
                ole_spid: spid,
                ..PaintExtras::default()
            },
        );
    }
    let xfrm = xfrm?;
    let (x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h, flip_v) = xfrm_to_box(Some(xfrm));
    if let Some(table) = child_local(graphic, "tbl", ns::A) {
        let (w, h) = table_grid_pixel_extent(table).unwrap_or((w, h));
        let rows = table_row_count(table);
        let mut el = build_dump(
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
        )?;
        el.table_data = Some(parse_table_model(
            table,
            ctx.theme_colors,
            ctx.color_map,
            ctx.table_style_map,
        ));
        return Some(el);
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
    if crate::ink::graphic_data_is_ink(graphic) {
        let ink_data = crate::ink::decode_aink_from_graphic_data(graphic);
        let extras = PaintExtras {
            ink_data,
            ..PaintExtras::default()
        };
        return build_dump(
            "ink", id, c_nv_pr, ph_info, x, y, w, h, x_emu, y_emu, w_emu, h_emu, rot, flip_h,
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

fn graphic_is_ole(g: &Element) -> bool {
    let uri = attr(g, "uri").map(str::to_lowercase).unwrap_or_default();
    uri.contains("/presentationml/2006/ole")
        || uri.contains("/drawingml/2006/ole")
        || find_local_deep(g, "oleObj").is_some()
}

fn find_local_deep<'a>(el: &'a Element, local: &str) -> Option<&'a Element> {
    let mut stack: Vec<&'a Element> = vec![el];
    while let Some(node) = stack.pop() {
        if node.local == local {
            return Some(node);
        }
        for child in node.children.iter().rev() {
            if let Node::Element(c) = child {
                stack.push(c);
            }
        }
    }
    None
}

/// `a:blip/@r:embed` anywhere under `el` (OLE `p:pic` preview).
fn deep_blip_embed(el: &Element) -> Option<String> {
    let mut stack: Vec<&Element> = vec![el];
    while let Some(node) = stack.pop() {
        if node.local == "blip" && (node.ns.as_deref() == Some(ns::A) || node.ns.is_none()) {
            let embed = node
                .attrs
                .iter()
                .find(|a| a.local == "embed" && (a.ns.is_none() || a.ns.as_deref() == Some(ns::R)))
                .map(|a| a.value.clone())
                .filter(|id| !id.is_empty());
            if embed.is_some() {
                return embed;
            }
        }
        for child in node.children.iter().rev() {
            if let Node::Element(c) = child {
                stack.push(c);
            }
        }
    }
    None
}

/// First `v:imagedata` relationship: `@r:id`, else legacy `@o:relid`.
fn first_imagedata_rel(el: &Element) -> Option<String> {
    if el.local == "imagedata" && (el.ns.as_deref() == Some(ns::V) || el.ns.is_none()) {
        if let Some(id) = imagedata_rel_attr(el) {
            return Some(id);
        }
    }
    for child in &el.children {
        if let Node::Element(c) = child {
            if let Some(id) = first_imagedata_rel(c) {
                return Some(id);
            }
        }
    }
    None
}

fn imagedata_rel_attr(el: &Element) -> Option<String> {
    let modern = el
        .attrs
        .iter()
        .find(|a| a.local == "id" && a.ns.as_deref() == Some(ns::R))
        .map(|a| a.value.clone())
        .filter(|id| !id.is_empty());
    if modern.is_some() {
        return modern;
    }
    el.attrs
        .iter()
        .find(|a| a.local.eq_ignore_ascii_case("relid"))
        .map(|a| a.value.clone())
        .filter(|id| !id.is_empty())
}

/// Relationship id of `v:shape/@id == spid` → `v:imagedata`.
pub(crate) fn vml_shape_image_rel(root: &Element, spid: &str) -> Option<String> {
    let mut stack: Vec<&Element> = vec![root];
    while let Some(el) = stack.pop() {
        let is_shape = el.local == "shape" && (el.ns.as_deref() == Some(ns::V) || el.ns.is_none());
        if is_shape && attr(el, "id") == Some(spid) {
            return first_imagedata_rel(el);
        }
        for child in el.children.iter().rev() {
            if let Node::Element(c) = child {
                stack.push(c);
            }
        }
    }
    None
}

/// Fill an OLE element's preview rel from a loaded `vmlDrawing` part.
pub(crate) fn assign_vml_ole_preview(el: &mut DumpElement, vml_path: &str, vml_root: &Element) {
    if el.type_ == "ole" && el.image_rel_id.is_none() {
        if let Some(spid) = el.ole_spid.clone() {
            if let Some(rel) = vml_shape_image_rel(vml_root, &spid) {
                el.image_rel_id = Some(rel);
                el.image_rel_owner = Some(vml_path.to_owned());
            }
        }
    }
    if let Some(children) = &mut el.children {
        for child in children {
            assign_vml_ole_preview(child, vml_path, vml_root);
        }
    }
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
            )
            .map(|mut el| {
                el.is_media = picture_is_media(node);
                el
            }),
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
        table_data: None,
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
        stroke_line_align: extras.stroke_line_align,
        stroke_dash: extras.stroke_dash,
        connector_start_arrow: extras.connector_start_arrow,
        connector_end_arrow: extras.connector_end_arrow,
        text_color: extras.text_color,
        font_size_px: extras.font_size_px,
        font_family: extras.font_family,
        font_bold: extras.font_bold,
        font_italic: extras.font_italic,
        text_stroke_width_px: extras.text_stroke_width_px,
        text_stroke_color: extras.text_stroke_color,
        text_body_layout: extras.text_body_layout,
        text_paint_paragraphs: extras.text_paint_paragraphs,
        presentation_kinsoku: extras.presentation_kinsoku,
        auto_fit_mode: extras.auto_fit_mode,
        chart_r_id: extras.chart_r_id,
        chart_xml: None,
        chart_part_path: None,
        chart_image_urls: None,
        smartart_dm_r_id: extras.smartart_dm_r_id,
        smartart_lo_r_id: extras.smartart_lo_r_id,
        smartart_qs_r_id: extras.smartart_qs_r_id,
        smartart_cs_r_id: extras.smartart_cs_r_id,
        smartart_drawing_path: None,
        smartart_drawing_xml: None,
        smartart_data_xml: None,
        image_rel_id: extras.image_rel_id,
        image_rel_owner: extras.image_rel_owner,
        ole_spid: extras.ole_spid,
        is_media: false,
        media_rel_id: extras.media_rel_id,
        media_src: None,
        media_type: extras.media_type,
        image_src: None,
        seq: 0,
        box_shadow: extras.box_shadow,
        material_box_shadow: extras.material_box_shadow,
        material_overlay: extras.material_overlay,
        text_shadow: extras.text_shadow,
        effect_filter: extras.effect_filter,
        ink_data: extras.ink_data,
        content_part_ink_rel_id: extras.content_part_ink_rel_id,
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
            Slot::El(el) if el.is_media => kinds.push("media".to_owned()),
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
    stroke_line_align: Option<String>,
    stroke_dash: Option<String>,
    connector_start_arrow: Option<String>,
    connector_end_arrow: Option<String>,
    media_rel_id: Option<String>,
    media_type: Option<String>,
    text_color: Option<String>,
    font_size_px: Option<f64>,
    font_family: Option<String>,
    font_bold: bool,
    font_italic: bool,
    text_stroke_width_px: Option<f64>,
    text_stroke_color: Option<String>,
    text_body_layout: Option<crate::text_layout::TextBodyLayout>,
    text_paint_paragraphs: Option<Vec<crate::text_layout::TextPaintParagraph>>,
    presentation_kinsoku: bool,
    auto_fit_mode: Option<crate::text_layout::TextAutoFitMode>,
    chart_r_id: Option<String>,
    smartart_dm_r_id: Option<String>,
    smartart_lo_r_id: Option<String>,
    smartart_qs_r_id: Option<String>,
    smartart_cs_r_id: Option<String>,
    image_rel_id: Option<String>,
    image_rel_owner: Option<String>,
    ole_spid: Option<String>,
    box_shadow: Option<String>,
    material_box_shadow: Option<String>,
    material_overlay: Option<String>,
    text_shadow: Option<String>,
    effect_filter: Option<String>,
    ink_data: Option<crate::ink::InkData>,
    content_part_ink_rel_id: Option<String>,
}

#[derive(Clone, Copy)]
struct SchemeTheme<'a> {
    colors: &'a BTreeMap<String, String>,
    /// `None` uses the PowerPoint defaults (`bg1→lt1`, …). A present map
    /// remaps aliases; keys it omits still use those defaults.
    map: Option<&'a ThemeColorMap>,
}

impl SchemeTheme<'_> {
    fn mapped_slot(&self, name: &str) -> String {
        if let Some(map) = self.map {
            if let Some(slot) = theme_color_slot_for(name, map) {
                return slot.as_str().to_owned();
            }
        }
        match name {
            "bg1" => "lt1",
            "tx1" => "dk1",
            "bg2" => "lt2",
            "tx2" => "dk2",
            other => other,
        }
        .to_owned()
    }
}

impl DrawingColorTheme for SchemeTheme<'_> {
    fn scheme(&self, name: &str) -> Option<String> {
        let slot = self.mapped_slot(name);
        self.colors
            .get(&slot)
            .cloned()
            .or_else(|| self.colors.get(name).cloned())
    }
}

/// Scheme map plus a resolved `phClr` override from a style reference colour child.
struct PhTheme<'a> {
    base: SchemeTheme<'a>,
    ph: Option<String>,
}

impl DrawingColorTheme for PhTheme<'_> {
    fn scheme(&self, name: &str) -> Option<String> {
        if name.eq_ignore_ascii_case("phClr") {
            if let Some(ph) = &self.ph {
                return Some(ph.clone());
            }
        }
        self.base.scheme(name)
    }
}

/// Resolved slide/layout/master background (`p:bg`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundPaint {
    pub color: Option<String>,
    pub gradient: Option<String>,
    /// `r:embed` on `a:blipFill/a:blip`, resolved by the package owner of this part.
    pub image_rel_id: Option<String>,
}

/// First paintable `p:cSld/p:bg` in `chain` (slide, then layout, then master).
pub fn first_paintable_background(
    chain: &[Option<&Element>],
    theme: &BTreeMap<String, String>,
    format_scheme: Option<&ThemeFormatScheme>,
    color_map: Option<&ThemeColorMap>,
) -> Option<BackgroundPaint> {
    let theme = SchemeTheme {
        colors: theme,
        map: color_map,
    };
    for part in chain.iter().flatten() {
        if let Some(paint) = background_of_c_sld(part, theme, format_scheme) {
            return Some(paint);
        }
    }
    None
}

fn background_of_c_sld(
    c_sld: &Element,
    theme: SchemeTheme<'_>,
    format_scheme: Option<&ThemeFormatScheme>,
) -> Option<BackgroundPaint> {
    let bg = child_local(c_sld, "bg", ns::P)?;
    if let Some(bg_pr) = child_local(bg, "bgPr", ns::P) {
        match parse_drawing_fill(Some(bg_pr)) {
            Some(DrawingFill::Solid { color }) => {
                let css = css_color(&color, theme, true)?;
                return Some(BackgroundPaint {
                    color: Some(css),
                    gradient: None,
                    image_rel_id: None,
                });
            }
            Some(ref fill @ DrawingFill::Gradient { .. }) => {
                if let Some(gradient) = linear_gradient_css(fill, theme) {
                    let DrawingFill::Gradient { stops, .. } = fill else {
                        return None;
                    };
                    let flat = stops.first().and_then(|s| css_color(&s.color, theme, true));
                    return Some(BackgroundPaint {
                        color: flat,
                        gradient: Some(gradient),
                        image_rel_id: None,
                    });
                }
            }
            Some(DrawingFill::Picture { rel_id, .. }) if rel_id.is_some() => {
                return Some(BackgroundPaint {
                    color: None,
                    gradient: None,
                    image_rel_id: rel_id,
                });
            }
            _ => {}
        }
    }
    let bg_ref = child_local(bg, "bgRef", ns::P)?;
    background_from_bg_ref(bg_ref, theme, format_scheme)
}

fn background_from_bg_ref(
    bg_ref: &Element,
    theme: SchemeTheme<'_>,
    format_scheme: Option<&ThemeFormatScheme>,
) -> Option<BackgroundPaint> {
    let idx = attr(bg_ref, "idx")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    if idx == 0 || idx == 1000 {
        return None;
    }
    let override_color = parse_drawing_color_in(Some(bg_ref));
    if let Some(fill) = matrix_fill(format_scheme, idx) {
        match fill {
            DrawingFill::None => return None,
            DrawingFill::Picture { rel_id, .. } if rel_id.is_some() => {
                return Some(BackgroundPaint {
                    color: None,
                    gradient: None,
                    image_rel_id: rel_id.clone(),
                });
            }
            DrawingFill::Gradient { .. } => {
                let ph = ph_theme(theme, override_color.as_ref());
                if let Some(gradient) = linear_gradient_css_in(fill, &ph) {
                    let flat = override_color
                        .as_ref()
                        .and_then(|c| css_color(c, theme, true))
                        .or_else(|| gradient_flat_color(fill, &ph, true));
                    return Some(BackgroundPaint {
                        color: flat,
                        gradient: Some(gradient),
                        image_rel_id: None,
                    });
                }
            }
            DrawingFill::Solid { color } => {
                let ph = ph_theme(theme, override_color.as_ref());
                // Colour child wins as the flat bgRef colour; otherwise the matrix solid.
                let css = override_color
                    .as_ref()
                    .and_then(|c| css_color(c, theme, true))
                    .or_else(|| css_color_in(color, &ph, true));
                if let Some(css) = css {
                    return Some(BackgroundPaint {
                        color: Some(css),
                        gradient: None,
                        image_rel_id: None,
                    });
                }
            }
            _ => {}
        }
    }
    let color = override_color.and_then(|c| css_color(&c, theme, true))?;
    Some(BackgroundPaint {
        color: Some(color),
        gradient: None,
        image_rel_id: None,
    })
}

fn shape_paint_extras(
    shape: &Element,
    effective_sp_pr: Option<&Element>,
    inherited_shape: Option<&Element>,
    tx_body: Option<&Element>,
    ph_info: Option<&PlaceholderInfo>,
    ctx: &ParseCtx,
    rotation_deg: Option<f64>,
) -> PaintExtras {
    let fallback = crate::theme::default_scheme_color_map();
    let theme = SchemeTheme {
        colors: ctx.theme_colors.unwrap_or(&fallback),
        map: ctx.color_map,
    };
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
        Some(DrawingFill::Picture { rel_id, .. }) => {
            extras.image_rel_id = rel_id;
        }
        Some(_) | None => {}
    }
    if extras.fill_mode.is_none() {
        apply_fill_ref(
            shape,
            inherited_shape,
            theme,
            ctx.format_scheme,
            &mut extras,
        );
    }
    let mut ln_blocks_ref = false;
    if let Some(ln) = effective_sp_pr.and_then(|sp| child_local(sp, "ln", ns::A)) {
        if child_local(ln, "noFill", ns::A).is_some() {
            ln_blocks_ref = true;
        }
        if let Some(algn) = attr(ln, "algn") {
            extras.stroke_line_align = Some(algn.to_owned());
        }
        extras.connector_start_arrow = parse_line_end_arrow(ln, "headEnd");
        extras.connector_end_arrow = parse_line_end_arrow(ln, "tailEnd");
        if let Some(line) = parse_drawing_line(Some(ln)) {
            if let Some(width) = line.width_emu {
                if width > 0 {
                    extras.stroke_width_px = Some(emu_to_pixels(width));
                }
            }
            if let Some(DrawingFill::Solid { color }) = line.fill {
                extras.stroke_color = css_color(&color, theme, false);
            }
            if let Some(dash) = line.dash {
                extras.stroke_dash = Some(dash);
            }
        }
    }
    if extras.stroke_color.is_none() && !ln_blocks_ref {
        apply_ln_ref(
            shape,
            inherited_shape,
            theme,
            ctx.format_scheme,
            &mut extras,
        );
    }
    if let Some(tx) = tx_body {
        let own_tx = child_local(shape, "txBody", ns::P);
        let inherited_tx = inherited_shape.and_then(|s| child_local(s, "txBody", ns::P));
        let mut effective_levels = ph_info.map(|ph| ctx.effective_level_styles(ph));
        let lst_levels = extract_lst_style_from_tx_body(tx);
        if !lst_levels.is_empty() {
            let base = effective_levels.unwrap_or_default();
            effective_levels = Some(merge_level_styles(&base, &lst_levels));
        }
        let is_body_ph = ph_info.is_some_and(crate::text::placeholder_is_body_type);
        let fallback = effective_levels
            .as_ref()
            .and_then(|styles| level_style_for(styles, 0).cloned());
        let paint = first_run_text_paint(
            tx,
            theme,
            FirstRunTextPaintCtx {
                shape,
                inherited_shape,
                font_scheme: ctx.font_scheme,
                level_fallback: fallback.as_ref(),
                presentation_level_styles: ctx.presentation_level_styles,
                master_tx_styles: ctx.master_tx_styles,
            },
        );
        extras.text_color = paint.color;
        extras.font_size_px = paint.size;
        extras.font_family = paint.font_family;
        extras.font_bold = paint.bold;
        extras.font_italic = paint.italic;
        extras.text_stroke_width_px = paint.stroke_width_px;
        extras.text_stroke_color = paint.stroke_color;
        extras.text_body_layout = Some(crate::text_layout::parse_text_body_layout_with_levels(
            tx,
            effective_levels.as_ref(),
            inherited_tx,
        ));
        let body_pr = child_local(tx, "bodyPr", ns::A)
            .or_else(|| inherited_tx.and_then(|t| child_local(t, "bodyPr", ns::A)));
        let space_first_last = body_pr
            .and_then(|bp| parse_optional_bool_attr(attr(bp, "spcFirstLastPara")))
            == Some(true);
        let compat_line_spacing =
            body_pr.and_then(|bp| parse_optional_bool_attr(attr(bp, "compatLnSpc"))) == Some(true);
        let line_spacing_reduction = body_pr
            .and_then(|bp| child_local(bp, "normAutofit", ns::A))
            .and_then(|n| attr(n, "lnSpcReduction"))
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| *v > 0.0)
            .map(|v| v / 100_000.0);
        let inherits_hf_text = ph_info
            .map(|p| is_header_footer_placeholder(p.type_.as_deref()))
            .unwrap_or(false)
            && !text_body_has_content(own_tx)
            && text_body_has_content(inherited_tx);
        let inherited_field_type = if inherits_hf_text {
            ph_info.and_then(|ph| {
                crate::inheritance::inherited_placeholder_field_type(ph.type_.as_deref())
                    .map(|s| s.to_owned())
            })
        } else {
            None
        };
        let auto_fit_mode = crate::text_layout::parse_body_autofit_cascade(tx, inherited_tx);
        let list_ctx = crate::text_layout::TextPaintListStyleCtx {
            own_tx_body: own_tx,
            inherited_tx_body: inherited_tx,
            is_body_placeholder: is_body_ph,
            effective_level_styles: effective_levels.as_ref(),
            presentation_level_styles: ctx.presentation_level_styles,
            font_scheme: ctx.font_scheme,
            space_first_last,
            compat_line_spacing,
            line_spacing_reduction,
            inherited_field_type,
            placeholder_type: ph_info.and_then(|ph| ph.type_.clone()),
        };
        let mut paragraphs =
            crate::text_layout::parse_text_paint_paragraphs_with_list_styles(tx, Some(&list_ctx));
        if let Some(levels) = effective_levels.as_ref() {
            for para in &mut paragraphs {
                if para.level_text_color.is_some() {
                    continue;
                }
                if let Some(style) = level_style_for(levels, para.list_level) {
                    para.level_text_color = level_style_text_color(style, theme);
                }
            }
            for para in &mut paragraphs {
                if para.bullet_text_color.is_some() {
                    continue;
                }
                if let Some(style) = level_style_for(levels, para.list_level) {
                    para.bullet_text_color =
                        crate::level_style::level_style_bullet_color(style, &theme);
                }
            }
        }
        extras.text_paint_paragraphs = Some(paragraphs);
        extras.presentation_kinsoku =
            presentation_default_ea_line_break(ctx.presentation_level_styles);
        extras.auto_fit_mode = auto_fit_mode;
        extras.text_shadow = first_run_text_shadow(tx, theme, rotation_deg);
    }
    if let Some(sp_pr) = effective_sp_pr {
        extras.box_shadow = crate::outer_shadow::box_shadow_from_sp_pr(sp_pr, &theme, rotation_deg);
        crate::outer_shadow::merge_effect_filter_into(
            &mut extras.effect_filter,
            crate::outer_shadow::effect_filter_from_sp_pr(sp_pr, &theme),
        );
    }
    if extras.box_shadow.is_none() {
        apply_effect_ref(
            shape,
            inherited_shape,
            theme,
            ctx.format_scheme,
            rotation_deg,
            &mut extras,
        );
    }
    extras
}

struct ResolvedFill {
    mode: String,
    color: Option<String>,
    gradient: Option<String>,
}

fn blip_embed_id(parent: &Element) -> Option<String> {
    let fill = child_local(parent, "blipFill", ns::A)
        .or_else(|| child_local(parent, "blipFill", ns::P))?;
    let blip = child_local(fill, "blip", ns::A)?;
    blip.attrs
        .iter()
        .find(|a| a.local == "embed" && (a.ns.is_none() || a.ns.as_deref() == Some(ns::R)))
        .map(|a| a.value.clone())
        .filter(|id| !id.is_empty())
}

fn style_ref_node<'a>(
    shape: &'a Element,
    inherited_shape: Option<&'a Element>,
    local: &str,
) -> Option<&'a Element> {
    let style = child_local(shape, "style", ns::P)
        .or_else(|| inherited_shape.and_then(|el| child_local(el, "style", ns::P)))?;
    child_local(style, local, ns::A)
}

fn style_ref_idx(ref_el: &Element) -> i32 {
    attr(ref_el, "idx")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0)
}

/// idx 1–999 → `fillStyleLst`; idx 1001–1003 → `bgFillStyleLst`.
fn matrix_fill(scheme: Option<&ThemeFormatScheme>, idx: i32) -> Option<&DrawingFill> {
    let scheme = scheme?;
    if (1001..=1003).contains(&idx) {
        scheme.background_fill_styles.get((idx - 1001) as usize)
    } else if (1..=999).contains(&idx) {
        scheme.fill_styles.get((idx - 1) as usize)
    } else {
        None
    }
}

fn matrix_line(scheme: Option<&ThemeFormatScheme>, idx: i32) -> Option<&DrawingLine> {
    let scheme = scheme?;
    if (1..=999).contains(&idx) {
        scheme.line_styles.get((idx - 1) as usize)
    } else {
        None
    }
}

fn matrix_effect(
    scheme: Option<&ThemeFormatScheme>,
    idx: i32,
) -> Option<&nitro_xml_opc::drawingml::ThemeEffectStyle> {
    let scheme = scheme?;
    if (1..=999).contains(&idx) {
        scheme.effect_styles.get((idx - 1) as usize)
    } else {
        None
    }
}

fn apply_effect_ref(
    shape: &Element,
    inherited_shape: Option<&Element>,
    theme: SchemeTheme<'_>,
    format_scheme: Option<&ThemeFormatScheme>,
    rotation_deg: Option<f64>,
    extras: &mut PaintExtras,
) {
    let Some(ref_el) = style_ref_node(shape, inherited_shape, "effectRef") else {
        return;
    };
    let idx = style_ref_idx(ref_el);
    if idx <= 0 || idx == 1000 {
        return;
    }
    let override_color = parse_drawing_color_in(Some(ref_el));
    let Some(effect_style) = matrix_effect(format_scheme, idx) else {
        return;
    };
    let ph = ph_theme(theme, override_color.as_ref());
    if let Some(effect_lst) = effect_style.effect_lst.as_ref() {
        extras.box_shadow =
            crate::outer_shadow::box_shadow_from_effect_lst(effect_lst, &ph, rotation_deg);
        crate::outer_shadow::merge_effect_filter_into(
            &mut extras.effect_filter,
            crate::outer_shadow::effect_filter_from_effect_lst(effect_lst, &ph),
        );
    }
    let mut inset_shadows: Vec<String> = Vec::new();
    let mut has_bevel = false;
    if let Some(bevel) = effect_style.sp3d_bevel.as_ref() {
        if let Some(shadow) = crate::visual_3d_bevel::get_3d_bevel_shadow(bevel, None) {
            inset_shadows.push(shadow);
            has_bevel = true;
        }
    }
    if let Some(material) = effect_style.prst_material.as_deref() {
        if material == "metal" && has_bevel {
            crate::outer_shadow::merge_effect_filter_into(
                &mut extras.effect_filter,
                Some(crate::visual_3d_materials::METAL_BEVEL_LEGACY_FILTER.to_owned()),
            );
        }
        let mat = crate::visual_3d_materials::get_material_css_overrides(material);
        if let Some(shadow) = mat.box_shadow {
            inset_shadows.push(shadow.to_owned());
        }
        extras.material_overlay = mat.background_image.map(str::to_owned);
    }
    if !inset_shadows.is_empty() {
        extras.material_box_shadow = Some(inset_shadows.join(", "));
    }
}

fn ph_hex(color: &DrawingColor, theme: SchemeTheme<'_>) -> Option<String> {
    resolve_drawing_color(color, Some(&theme), None).map(|r| r.hex)
}

fn ph_theme<'a>(theme: SchemeTheme<'a>, override_color: Option<&DrawingColor>) -> PhTheme<'a> {
    PhTheme {
        base: theme,
        ph: override_color.and_then(|c| ph_hex(c, theme)),
    }
}

fn apply_fill_ref(
    shape: &Element,
    inherited_shape: Option<&Element>,
    theme: SchemeTheme<'_>,
    format_scheme: Option<&ThemeFormatScheme>,
    extras: &mut PaintExtras,
) {
    let Some(ref_el) = style_ref_node(shape, inherited_shape, "fillRef") else {
        return;
    };
    let idx = style_ref_idx(ref_el);
    if idx <= 0 || idx == 1000 {
        return;
    }
    let override_color = parse_drawing_color_in(Some(ref_el));
    if let Some(fill) = matrix_fill(format_scheme, idx) {
        if matches!(fill, DrawingFill::None) {
            extras.fill_mode = Some("none".to_owned());
            return;
        }
        if let Some(resolved) = resolve_matrix_fill(fill, override_color.as_ref(), theme, false) {
            extras.fill_mode = Some(resolved.mode);
            extras.fill_color = resolved.color;
            extras.fill_gradient = resolved.gradient;
            return;
        }
    }
    if let Some(css) = override_color.and_then(|c| css_color(&c, theme, false)) {
        extras.fill_mode = Some("solid".to_owned());
        extras.fill_color = Some(css);
    }
}

fn apply_ln_ref(
    shape: &Element,
    inherited_shape: Option<&Element>,
    theme: SchemeTheme<'_>,
    format_scheme: Option<&ThemeFormatScheme>,
    extras: &mut PaintExtras,
) {
    let Some(ref_el) = style_ref_node(shape, inherited_shape, "lnRef") else {
        return;
    };
    let idx = style_ref_idx(ref_el);
    if idx <= 0 || idx == 1000 {
        return;
    }
    let override_color = parse_drawing_color_in(Some(ref_el));
    if let Some(line) = matrix_line(format_scheme, idx) {
        if matches!(line.fill, Some(DrawingFill::None)) {
            return;
        }
        let ph = ph_theme(theme, override_color.as_ref());
        let color = match &line.fill {
            Some(DrawingFill::Solid { color }) => css_color_in(color, &ph, false),
            _ => None,
        }
        .or_else(|| {
            override_color
                .as_ref()
                .and_then(|c| css_color(c, theme, false))
        });
        if let Some(color) = color {
            extras.stroke_color = Some(color);
            if extras.stroke_width_px.is_none() {
                extras.stroke_width_px = line
                    .width_emu
                    .filter(|w| *w > 0)
                    .map(emu_to_pixels)
                    .or(Some(1.0));
            }
            return;
        }
    }
    if let Some(css) = override_color.and_then(|c| css_color(&c, theme, false)) {
        extras.stroke_color = Some(css);
        if extras.stroke_width_px.is_none() {
            extras.stroke_width_px = Some(1.0);
        }
    }
}

fn resolve_matrix_fill(
    fill: &DrawingFill,
    override_color: Option<&DrawingColor>,
    theme: SchemeTheme<'_>,
    blend_white: bool,
) -> Option<ResolvedFill> {
    let ph = ph_theme(theme, override_color);
    match fill {
        DrawingFill::None => Some(ResolvedFill {
            mode: "none".to_owned(),
            color: None,
            gradient: None,
        }),
        DrawingFill::Solid { color } => {
            let css = css_color_in(color, &ph, blend_white)
                .or_else(|| override_color.and_then(|c| css_color(c, theme, blend_white)));
            css.map(|color| ResolvedFill {
                mode: "solid".to_owned(),
                color: Some(color),
                gradient: None,
            })
        }
        DrawingFill::Gradient { .. } => {
            let gradient = linear_gradient_css_in(fill, &ph)?;
            Some(ResolvedFill {
                mode: "gradient".to_owned(),
                color: gradient_flat_color(fill, &ph, blend_white),
                gradient: Some(gradient),
            })
        }
        _ => None,
    }
}

fn gradient_flat_color(
    fill: &DrawingFill,
    theme: &dyn DrawingColorTheme,
    blend_white: bool,
) -> Option<String> {
    let DrawingFill::Gradient { stops, .. } = fill else {
        return None;
    };
    stops
        .first()
        .and_then(|stop| css_color_in(&stop.color, theme, blend_white))
}

struct FirstRunTextPaint {
    color: Option<String>,
    size: Option<f64>,
    font_family: Option<String>,
    bold: bool,
    italic: bool,
    stroke_width_px: Option<f64>,
    stroke_color: Option<String>,
}

struct FirstRunTextPaintCtx<'a> {
    shape: &'a Element,
    inherited_shape: Option<&'a Element>,
    font_scheme: Option<&'a ThemeFontScheme>,
    level_fallback: Option<&'a PlaceholderTextLevelStyle>,
    presentation_level_styles: Option<&'a LevelStylesMap>,
    master_tx_styles: Option<&'a MasterTxStyles>,
}

fn apply_norm_autofit_to_extras(
    extras: &mut PaintExtras,
    own_tx: Option<&Element>,
    inherited_tx: Option<&Element>,
    box_height_px: f64,
) {
    use crate::text_layout::{
        norm_autofit_font_scale_multiplier, scale_font_size_for_autofit,
        scale_norm_autofit_font_size_px, TextAutoFitMode,
    };
    if extras.auto_fit_mode != Some(TextAutoFitMode::Normal) {
        return;
    }
    let Some(sz) = extras.font_size_px else {
        return;
    };
    let para_count = extras
        .text_paint_paragraphs
        .as_ref()
        .map(|p| p.len())
        .unwrap_or(1);
    let inset_tb = extras
        .text_body_layout
        .as_ref()
        .map(|l| l.inset_top_px + l.inset_bottom_px)
        .unwrap_or(0.0);
    let font_scale_mult =
        own_tx.and_then(|own| norm_autofit_font_scale_multiplier(own, inherited_tx));
    let scale_px = |font_px: f64| {
        if let Some(mult) = font_scale_mult {
            scale_font_size_for_autofit(font_px, mult)
        } else {
            scale_norm_autofit_font_size_px(font_px, box_height_px, inset_tb, para_count)
        }
    };
    extras.font_size_px = Some(scale_px(sz));
    if let Some(paras) = &mut extras.text_paint_paragraphs {
        for para in paras.iter_mut() {
            if let Some(level_sz) = para.level_font_size_px {
                para.level_font_size_px = Some(scale_px(level_sz));
            }
            if let Some(sz) = para.empty_line_font_size_px {
                para.empty_line_font_size_px = Some(scale_px(sz));
            }
            for run in &mut para.runs {
                if let Some(sz) = run.run_font_size_px {
                    run.run_font_size_px = Some(scale_px(sz));
                }
            }
        }
    }
}

fn parse_line_end_arrow(ln: &Element, local: &str) -> Option<String> {
    let end = child_local(ln, local, ns::A)?;
    let ty = attr(end, "type")?.trim().to_ascii_lowercase();
    if ty.is_empty() || ty == "none" {
        return None;
    }
    Some(ty)
}

fn first_run_text_paint(
    tx: &Element,
    theme: SchemeTheme<'_>,
    ctx: FirstRunTextPaintCtx<'_>,
) -> FirstRunTextPaint {
    let mut color = None;
    let mut size = None;
    let mut font_family = None;
    let mut bold = false;
    let mut italic = false;
    let mut stroke_width_px = None;
    let mut stroke_color = None;
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
            if font_family.is_none() {
                font_family =
                    resolve_run_font_family(def, ctx.shape, ctx.inherited_shape, ctx.font_scheme);
            }
            if !bold {
                bold = def.map(rpr_bold).unwrap_or(false);
            }
            if !italic {
                italic = def.map(rpr_italic).unwrap_or(false);
            }
            if stroke_width_px.is_none() {
                if let Some(def) = def {
                    let (w, c) = rpr_text_stroke(def, theme);
                    stroke_width_px = w;
                    stroke_color = c;
                }
            }
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
        if font_family.is_none() {
            font_family = resolve_run_font_family(
                rpr.or(def),
                ctx.shape,
                ctx.inherited_shape,
                ctx.font_scheme,
            );
        }
        if !bold {
            bold = rpr
                .map(rpr_bold)
                .unwrap_or_else(|| def.map(rpr_bold).unwrap_or(false));
        }
        if !italic {
            italic = rpr
                .map(rpr_italic)
                .unwrap_or_else(|| def.map(rpr_italic).unwrap_or(false));
        }
        if stroke_width_px.is_none() {
            let stroke_rpr = rpr.or(def);
            if let Some(r) = stroke_rpr {
                let (w, c) = rpr_text_stroke(r, theme);
                stroke_width_px = w;
                stroke_color = c;
            }
        }
        break;
    }
    if color.is_none() {
        color = font_ref_text_color(ctx.shape, ctx.inherited_shape, theme);
    }
    if let Some(style) = ctx.level_fallback {
        if color.is_none() {
            color = level_style_text_color(style, theme);
        }
        if size.is_none() {
            size = style.font_size_px;
        }
        if font_family.is_none() {
            if let Some(face) = style.default_latin_typeface.as_deref() {
                font_family = resolve_theme_latin_typeface(face, ctx.font_scheme);
            }
        }
        if !bold {
            bold = style.font_bold.unwrap_or(false);
        }
        if !italic {
            italic = style.font_italic.unwrap_or(false);
        }
    }
    if font_family.is_none() {
        font_family =
            presentation_default_latin_typeface(ctx.presentation_level_styles, ctx.font_scheme);
    }
    if size.is_none() {
        size =
            non_placeholder_font_size_fallback(ctx.presentation_level_styles, ctx.master_tx_styles);
    }
    FirstRunTextPaint {
        color,
        size,
        font_family,
        bold,
        italic,
        stroke_width_px,
        stroke_color,
    }
}

fn non_placeholder_font_size_fallback(
    presentation_level_styles: Option<&LevelStylesMap>,
    master_tx_styles: Option<&MasterTxStyles>,
) -> Option<f64> {
    presentation_level_styles
        .and_then(|map| level_style_for(map, 0))
        .and_then(|s| s.font_size_px)
        .or_else(|| {
            master_tx_styles
                .and_then(|tx| level_style_for(&tx.other_style, 0))
                .and_then(|s| s.font_size_px)
        })
}

fn resolve_run_font_family(
    rpr: Option<&Element>,
    shape: &Element,
    inherited_shape: Option<&Element>,
    font_scheme: Option<&ThemeFontScheme>,
) -> Option<String> {
    use crate::theme::{resolve_theme_typeface, ThemeTypefaceScript};
    if let Some(r) = rpr {
        if let Some(face) = latin_typeface(r) {
            if let Some(resolved) =
                resolve_theme_typeface(&face, font_scheme, ThemeTypefaceScript::Latin)
            {
                return Some(resolved);
            }
        }
        if let Some(ea) = child_local(r, "ea", ns::A).and_then(|el| attr(el, "typeface")) {
            if let Some(resolved) =
                resolve_theme_typeface(ea, font_scheme, ThemeTypefaceScript::EastAsia)
            {
                return Some(resolved);
            }
        }
    }
    font_ref_family(shape, inherited_shape, font_scheme)
}

fn latin_typeface(rpr: &Element) -> Option<String> {
    child_local(rpr, "latin", ns::A)
        .and_then(|latin| attr(latin, "typeface"))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn rpr_bold(rpr: &Element) -> bool {
    matches!(attr(rpr, "b"), Some("1") | Some("true") | Some("on"))
}

fn rpr_italic(rpr: &Element) -> bool {
    matches!(attr(rpr, "i"), Some("1") | Some("true") | Some("on"))
}

fn rpr_text_stroke(rpr: &Element, theme: SchemeTheme<'_>) -> (Option<f64>, Option<String>) {
    let Some(ln) = child_local(rpr, "ln", ns::A) else {
        return (None, None);
    };
    let Some(line) = parse_drawing_line(Some(ln)) else {
        return (None, None);
    };
    let width = line.width_emu.filter(|w| *w > 0).map(emu_to_pixels);
    let color = match &line.fill {
        Some(DrawingFill::Solid { color }) => css_color(color, theme, false),
        // gradFill / other non-solid: paint uses `currentColor` (matches upstream text-outline).
        _ => None,
    };
    (width, color)
}

fn font_ref_text_color(
    shape: &Element,
    inherited_shape: Option<&Element>,
    theme: SchemeTheme<'_>,
) -> Option<String> {
    let ref_el = style_ref_node(shape, inherited_shape, "fontRef")?;
    let color = parse_drawing_color_in(Some(ref_el))?;
    css_color(&color, theme, false)
}

fn font_ref_family(
    shape: &Element,
    inherited_shape: Option<&Element>,
    font_scheme: Option<&ThemeFontScheme>,
) -> Option<String> {
    let scheme = font_scheme?;
    let ref_el = style_ref_node(shape, inherited_shape, "fontRef")?;
    let idx = attr(ref_el, "idx").unwrap_or("minor");
    if idx == "none" {
        return None;
    }
    theme_font_latin(scheme, idx)
}

fn presentation_default_latin_typeface(
    styles: Option<&LevelStylesMap>,
    font_scheme: Option<&ThemeFontScheme>,
) -> Option<String> {
    let styles = styles?;
    for key in [0i8, -1i8] {
        if let Some(face) = styles
            .get(&key)
            .and_then(|s| s.default_latin_typeface.as_deref())
        {
            if let Some(resolved) = resolve_theme_latin_typeface(face, font_scheme) {
                return Some(resolved);
            }
        }
    }
    None
}

fn resolve_theme_latin_typeface(
    typeface: &str,
    font_scheme: Option<&ThemeFontScheme>,
) -> Option<String> {
    let t = typeface.trim();
    if t.is_empty() {
        return None;
    }
    if t.eq_ignore_ascii_case("+mn-lt") {
        return font_scheme.and_then(|s| theme_font_latin(s, "minor"));
    }
    if t.eq_ignore_ascii_case("+mj-lt") {
        return font_scheme.and_then(|s| theme_font_latin(s, "major"));
    }
    if t.starts_with('+') {
        return None;
    }
    Some(t.to_owned())
}

fn theme_font_latin(scheme: &ThemeFontScheme, idx: &str) -> Option<String> {
    let fonts = match idx {
        "major" => scheme.major_font.as_ref(),
        "minor" => scheme.minor_font.as_ref(),
        _ => None,
    }?;
    fonts
        .latin
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn first_run_text_shadow(
    tx: &Element,
    theme: SchemeTheme<'_>,
    rotation_deg: Option<f64>,
) -> Option<String> {
    let para = children_local(tx, "p", ns::A).into_iter().next()?;
    let def = child_path(para, &[("pPr", ns::A), ("defRPr", ns::A)]);
    let runs: Vec<&Element> = para
        .children
        .iter()
        .filter_map(|n| match n {
            Node::Element(el) if el.local == "r" || el.local == "fld" => Some(el),
            _ => None,
        })
        .collect();
    let rpr = if runs.is_empty() {
        def
    } else {
        child_local(runs[0], "rPr", ns::A).or(def)
    };
    let rpr = rpr?;
    let effect_lst = child_local(rpr, "effectLst", ns::A)?;
    crate::outer_shadow::text_shadow_from_effect_lst(effect_lst, &theme, rotation_deg)
}

fn level_style_text_color(
    style: &PlaceholderTextLevelStyle,
    theme: SchemeTheme<'_>,
) -> Option<String> {
    if let Some(hex) = &style.text_color_hex {
        return Some(hex.clone());
    }
    style
        .text_color
        .as_ref()
        .and_then(|color| css_color(color, theme, false))
}

fn apply_rpr(
    rpr: Option<&Element>,
    color: &mut Option<String>,
    size: &mut Option<f64>,
    theme: SchemeTheme<'_>,
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

fn solid_css(rpr: &Element, theme: SchemeTheme<'_>) -> Option<String> {
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

fn css_color(color: &DrawingColor, theme: SchemeTheme<'_>, blend_white: bool) -> Option<String> {
    css_color_in(color, &theme, blend_white)
}

fn css_color_in(
    color: &DrawingColor,
    theme: &dyn DrawingColorTheme,
    blend_white: bool,
) -> Option<String> {
    let resolved = resolve_drawing_color(color, Some(theme), None)?;
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

fn linear_gradient_css(fill: &DrawingFill, theme: SchemeTheme<'_>) -> Option<String> {
    linear_gradient_css_in(fill, &theme)
}

fn linear_gradient_css_in(fill: &DrawingFill, theme: &dyn DrawingColorTheme) -> Option<String> {
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
        let color = css_color_in(&stop.color, theme, false)?;
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
            color_map: None,
            format_scheme: None,
            font_scheme: None,
            table_style_map: None,
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
        let paint = first_paintable_background(&[Some(&doc.root)], &theme(), None, None).unwrap();
        assert_eq!(paint.color.as_deref(), Some("#FF0000"));
        assert_eq!(
            paint.gradient.as_deref(),
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
        let paint = first_paintable_background(
            &[Some(&slide.root), Some(&layout.root)],
            &theme(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(paint.color.as_deref(), Some("#00FF00"));
        assert!(paint.gradient.is_none());
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

    // Catches: master titleStyle defRPr white ignored so title paints black on brown gradient.
    #[test]
    fn master_title_style_def_rpr_paints_white() {
        let master = parse_xml_str(
            r#"<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <p:txStyles>
                <p:titleStyle>
                  <a:lvl1pPr>
                    <a:buNone/>
                    <a:defRPr sz="4400" b="1" kern="1200"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:defRPr>
                  </a:lvl1pPr>
                </p:titleStyle>
              </p:txStyles>
            </p:sldMaster>"#,
        )
        .unwrap();
        let styles = crate::level_style::parse_master_tx_styles(&master.root);
        let layout_lst = parse_xml_str(
            r#"<a:lstStyle xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <a:lvl1pPr algn="ctr"><a:defRPr sz="6000"/></a:lvl1pPr>
            </a:lstStyle>"#,
        )
        .unwrap();
        let mut layout_levels = PlaceholderDefaultsCache::new();
        layout_levels.insert(
            "title".to_owned(),
            crate::level_style::parse_lst_style_levels(&layout_lst.root),
        );
        let shape = parse_xml_str(
            r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <p:nvSpPr><p:cNvPr id="2" name="Title 1"/><p:cNvSpPr/><p:nvPr><p:ph type="ctrTitle"/></p:nvPr></p:nvSpPr>
              <p:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
              </p:spPr>
              <p:txBody>
                <a:bodyPr/>
                <a:lstStyle/>
                <a:p><a:r><a:rPr lang="en-US"/><a:t>Title</a:t></a:r></a:p>
              </p:txBody>
            </p:sp>"#,
        )
        .unwrap();
        let colors = theme();
        let ctx = ParseCtx {
            master_tx_styles: Some(&styles),
            layout_placeholder_levels: Some(&layout_levels),
            ..ctx(&colors)
        };
        let el = parse_shape(&shape.root, "s".into(), &ctx, false).unwrap();
        assert_eq!(el.text_color.as_deref(), Some("#FFFFFF"));
        assert!((el.font_size_px.unwrap() - 60.0 * 96.0 / 72.0).abs() < 0.05);
        assert!(el.font_bold);
        let para = el.text_paint_paragraphs.as_ref().unwrap().first().unwrap();
        assert_eq!(para.level_font_kern_hundredths, Some(1200));
        assert_eq!(para.level_font_bold, Some(true));
    }

    // Catches: body placeholder `bodyStyle` defRPr sz left at 24px host default instead of master 28pt.
    #[test]
    fn body_placeholder_applies_merged_tx_style_font_size() {
        let master = parse_xml_str(
            r#"<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <p:txStyles>
                <p:bodyStyle>
                  <a:lvl1pPr><a:defRPr sz="2800"/></a:lvl1pPr>
                </p:bodyStyle>
              </p:txStyles>
            </p:sldMaster>"#,
        )
        .unwrap();
        let styles = crate::level_style::parse_master_tx_styles(&master.root);
        let shape = parse_xml_str(
            r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr>
              <p:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
              </p:spPr>
              <p:txBody>
                <a:bodyPr/>
                <a:p><a:r><a:rPr lang="en-US"/><a:t>Body</a:t></a:r></a:p>
              </p:txBody>
            </p:sp>"#,
        )
        .unwrap();
        let colors = theme();
        let ctx = ParseCtx {
            master_tx_styles: Some(&styles),
            ..ctx(&colors)
        };
        let el = parse_shape(&shape.root, "s".into(), &ctx, false).unwrap();
        assert!((el.font_size_px.unwrap() - 28.0 * 96.0 / 72.0).abs() < 0.05);
    }

    // Catches: `p:defaultTextStyle` +mn-lt winning over merged `bodyStyle` Verdana on placeholders.
    #[test]
    fn body_placeholder_applies_merged_tx_style_typeface() {
        let master = parse_xml_str(
            r#"<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <p:txStyles>
                <p:bodyStyle>
                  <a:lvl1pPr><a:defRPr sz="2600"><a:latin typeface="Verdana"/></a:defRPr></a:lvl1pPr>
                </p:bodyStyle>
              </p:txStyles>
            </p:sldMaster>"#,
        )
        .unwrap();
        let styles = crate::level_style::parse_master_tx_styles(&master.root);
        let shape = parse_xml_str(
            r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
              <p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr>
              <p:spPr>
                <a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm>
                <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
              </p:spPr>
              <p:txBody>
                <a:bodyPr/>
                <a:p><a:r><a:rPr lang="en-US"/><a:t>Body</a:t></a:r></a:p>
              </p:txBody>
            </p:sp>"#,
        )
        .unwrap();
        let pres_default = crate::level_style::parse_tx_style_category(
            &parse_xml_str(
                r#"<p:defaultTextStyle xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
                  <a:lvl1pPr><a:defRPr><a:latin typeface="+mn-lt"/></a:defRPr></a:lvl1pPr>
                </p:defaultTextStyle>"#,
            )
            .unwrap()
            .root,
        );
        let colors = theme();
        let ctx = ParseCtx {
            master_tx_styles: Some(&styles),
            presentation_level_styles: Some(&pres_default),
            ..ctx(&colors)
        };
        let el = parse_shape(&shape.root, "s".into(), &ctx, false).unwrap();
        assert_eq!(el.font_family.as_deref(), Some("Verdana"));
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

    fn office_matrix() -> nitro_xml_opc::drawingml::ThemeFormatScheme {
        let xml = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <a:themeElements>
            <a:clrScheme name="Office"/>
            <a:fontScheme name="Office"><a:majorFont/><a:minorFont/></a:fontScheme>
            <a:fmtScheme name="Office">
              <a:fillStyleLst>
                <a:solidFill><a:srgbClr val="112233"/></a:solidFill>
                <a:gradFill>
                  <a:gsLst>
                    <a:gs pos="0"><a:schemeClr val="phClr"/></a:gs>
                    <a:gs pos="100000"><a:srgbClr val="00FF00"/></a:gs>
                  </a:gsLst>
                  <a:lin ang="0"/>
                </a:gradFill>
              </a:fillStyleLst>
              <a:lnStyleLst>
                <a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
                <a:ln w="25400"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
              </a:lnStyleLst>
              <a:bgFillStyleLst>
                <a:solidFill><a:srgbClr val="ABCDEF"/></a:solidFill>
                <a:gradFill>
                  <a:gsLst>
                    <a:gs pos="0"><a:schemeClr val="phClr"/></a:gs>
                    <a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs>
                  </a:gsLst>
                  <a:lin ang="0"/>
                </a:gradFill>
              </a:bgFillStyleLst>
              <a:effectStyleLst>
                <a:effectStyle>
                  <a:effectLst>
                    <a:outerShdw blurRad="19050" dist="38100" dir="2700000" rotWithShape="0">
                      <a:schemeClr val="phClr"><a:alpha val="45000"/></a:schemeClr>
                    </a:outerShdw>
                  </a:effectLst>
                </a:effectStyle>
              </a:effectStyleLst>
            </a:fmtScheme>
          </a:themeElements>
        </a:theme>"#;
        nitro_xml_opc::drawingml::parse_theme_xml(xml, "t")
            .format_scheme
            .expect("fmtScheme")
    }

    fn ctx_with_matrix<'a>(
        theme: &'a BTreeMap<String, String>,
        scheme: &'a nitro_xml_opc::drawingml::ThemeFormatScheme,
    ) -> ParseCtx<'a> {
        ParseCtx {
            format_scheme: Some(scheme),
            ..ctx(theme)
        }
    }

    // Catches: fillRef idx reading only the ref schemeClr, ignoring fmtScheme fills and bgFillStyleLst.
    #[test]
    fn fill_ref_uses_format_scheme_matrix() {
        let colors = theme();
        let scheme = office_matrix();
        let solid = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="1" name="S"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
          <p:style><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef></p:style>
        </p:sp>"#;
        let el = parse_shape(
            &parse_xml_str(solid).unwrap().root,
            "s".into(),
            &ctx_with_matrix(&colors, &scheme),
            false,
        )
        .unwrap();
        assert_eq!(el.fill_color.as_deref(), Some("#112233"));

        let grad = solid.replace("idx=\"1\"", "idx=\"2\"");
        let el = parse_shape(
            &parse_xml_str(&grad).unwrap().root,
            "g".into(),
            &ctx_with_matrix(&colors, &scheme),
            false,
        )
        .unwrap();
        assert_eq!(el.fill_mode.as_deref(), Some("gradient"));
        assert_eq!(
            el.fill_gradient.as_deref(),
            Some("linear-gradient(90.00deg, #4472C4 0%, #00FF00 100%)")
        );

        let bg = solid.replace("idx=\"1\"", "idx=\"1001\"");
        let el = parse_shape(
            &parse_xml_str(&bg).unwrap().root,
            "b".into(),
            &ctx_with_matrix(&colors, &scheme),
            false,
        )
        .unwrap();
        assert_eq!(el.fill_color.as_deref(), Some("#ABCDEF"));
    }

    // Catches: keyframes title run outerShdw never becomes text-shadow so the pink title floats without TS drop shadow.
    #[test]
    fn first_run_outer_shdw_becomes_text_shadow() {
        let colors = theme();
        let xml = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="16" name="Title"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>
          <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r>
            <a:rPr lang="en-US" sz="5400"><a:solidFill><a:schemeClr val="accent2"/></a:solidFill>
              <a:effectLst><a:outerShdw blurRad="50800" dist="40000" dir="5400000" algn="tl" rotWithShape="0">
                <a:srgbClr val="000000"><a:alpha val="33000"/></a:srgbClr>
              </a:outerShdw></a:effectLst>
            </a:rPr><a:t>Selamat Belajar Biologi</a:t>
          </a:r></a:p></p:txBody>
        </p:sp>"#;
        let el = parse_shape(
            &parse_xml_str(xml).unwrap().root,
            "title".into(),
            &ctx(&colors),
            false,
        )
        .unwrap();
        let shadow = el
            .text_shadow
            .as_deref()
            .expect("text_shadow from rPr effectLst");
        assert!(
            shadow.contains("0px 4px"),
            "dir 90° dist≈4px → vertical offset, got {shadow}"
        );
        assert!(
            shadow.contains("5px"),
            "blurRad 50800 → ~5px blur (TS ≈5.3), got {shadow}"
        );
        assert!(
            shadow.contains("rgba(0, 0, 0, 0.33)"),
            "alpha 33000 → 0.33, got {shadow}"
        );
        assert!(
            el.box_shadow.is_none(),
            "noFill spPr must not set box-shadow"
        );
    }

    // Catches: effectRef idx 1 outerShdw never becomes box-shadow so keyframes menu buttons float without shadow.
    #[test]
    fn effect_ref_resolves_theme_outer_shadow() {
        let colors = theme();
        let scheme = office_matrix();
        let xml = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="1" name="S"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
          <p:style><a:effectRef idx="1"><a:schemeClr val="accent1"/></a:effectRef></p:style>
        </p:sp>"#;
        let el = parse_shape(
            &parse_xml_str(xml).unwrap().root,
            "s".into(),
            &ctx_with_matrix(&colors, &scheme),
            false,
        )
        .unwrap();
        let shadow = el.box_shadow.as_deref().expect("box_shadow from effectRef");
        assert!(
            shadow.contains("rgba(68, 114, 196,"),
            "phClr from accent1, got {shadow}"
        );
        assert!(
            shadow.contains("3px 3px"),
            "45° dist≈4px → ~3px offsets, got {shadow}"
        );
    }

    // Catches: effectRef idx=2 theme metal sp3d never yields inset specular on the shape.
    #[test]
    fn effect_ref_resolves_theme_metal_material() {
        let colors = theme();
        let xml = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <a:themeElements>
            <a:clrScheme name="Office"/>
            <a:fontScheme name="Office"><a:majorFont/><a:minorFont/></a:fontScheme>
            <a:fmtScheme name="Office">
              <a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>
              <a:lnStyleLst><a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>
              <a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst>
              <a:effectStyleLst>
                <a:effectStyle><a:effectLst/></a:effectStyle>
                <a:effectStyle>
                  <a:effectLst>
                    <a:outerShdw blurRad="19050" dist="38100" dir="2700000" rotWithShape="0">
                      <a:schemeClr val="phClr"><a:alpha val="45000"/></a:schemeClr>
                    </a:outerShdw>
                  </a:effectLst>
                  <a:sp3d prstMaterial="metal"><a:bevelT w="10000" h="10000"/></a:sp3d>
                </a:effectStyle>
              </a:effectStyleLst>
            </a:fmtScheme>
          </a:themeElements>
        </a:theme>"#;
        let scheme = nitro_xml_opc::drawingml::parse_theme_xml(xml, "t")
            .format_scheme
            .expect("fmtScheme");
        let shape_xml = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="1" name="S"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
          <p:style><a:effectRef idx="2"><a:schemeClr val="accent1"/></a:effectRef></p:style>
        </p:sp>"#;
        let el = parse_shape(
            &parse_xml_str(shape_xml).unwrap().root,
            "s".into(),
            &ctx_with_matrix(&colors, &scheme),
            false,
        )
        .unwrap();
        let inset = el
            .material_box_shadow
            .as_deref()
            .expect("material_box_shadow");
        assert!(
            inset.contains("inset -1px -1px 3px rgba(255,255,255,"),
            "default circle bevel on 10000 EMU, got {inset}"
        );
        assert!(
            inset.contains("inset -3px -3px 8px"),
            "metal inset specular, got {inset}"
        );
        let overlay = el.material_overlay.as_deref().expect("material_overlay");
        assert!(
            overlay.starts_with("linear-gradient(135deg"),
            "metal gradient overlay, got {overlay}"
        );
        assert!(el.box_shadow.is_some(), "outer shadow from effectLst");
    }

    // Catches: title-slide ctrTitle using master 44pt instead of layout 60pt / missing bold+kern from txStyle.
    #[test]
    fn upstream_title_slide_ctr_title_tx_style() {
        use crate::presentation::Presentation;
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/upstream/pptx/1abd9b12cb19_master-layout-inheritance-fills.pptx"
        ))
        .expect("fixture");
        let mut opened = Presentation::open(&bytes).expect("open");
        let slide = opened.slide(0).expect("slide0");
        let title = slide
            .elements
            .iter()
            .find(|e| {
                e.text
                    .as_deref()
                    .is_some_and(|t| t.contains("Master & Layout"))
            })
            .expect("title");
        let sz60 = 60.0 * 96.0 / 72.0;
        let para = title
            .text_paint_paragraphs
            .as_ref()
            .unwrap()
            .first()
            .unwrap();
        assert!(
            (para.level_font_size_px.unwrap() - sz60).abs() < 0.5,
            "level sz {:?}",
            para.level_font_size_px
        );
        assert_eq!(para.level_font_bold, Some(true));
        assert_eq!(para.level_font_kern_hundredths, Some(1200));
        assert!(title.font_bold);
        assert_eq!(
            title.font_family.as_deref(),
            Some("Aptos Display"),
            "title uses theme major latin"
        );
        let layout = title.text_body_layout.as_ref().expect("layout");
        assert_eq!(layout.v_align, crate::text_layout::TextBodyVAlign::Bottom);
        assert_eq!(
            para.line_height_css.as_deref(),
            Some("1.08"),
            "titleStyle lnSpc 0.9 × default 1.2"
        );
    }

    // Catches: CJK line-breaking fixture red runs losing `a:rPr/a:solidFill` at parse time.
    #[test]
    fn upstream_cjk_line_breaking_run_solid_fill_parsed() {
        use crate::presentation::Presentation;
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/upstream/pptx/e3dab827b836_cjk-line-breaking.pptx"
        ))
        .expect("fixture");
        let mut opened = Presentation::open(&bytes).expect("open");
        let slide = opened.slide(0).expect("slide0");
        let shape = slide
            .elements
            .iter()
            .find(|e| {
                e.text
                    .as_deref()
                    .map(|t| t.contains("まみむめ"))
                    .unwrap_or(false)
            })
            .expect("shape");
        let paras = shape.text_paint_paragraphs.as_ref().expect("paras");
        let red = paras[0]
            .runs
            .iter()
            .find(|r| r.text.contains('」'))
            .and_then(|r| r.run_text_color.as_deref());
        assert_eq!(red, Some("#C00000"));
    }

    // Catches: CJK hanging-punct paragraphs losing per-paragraph `eastAsianBreaks` at parse time.
    #[test]
    fn upstream_cjk_line_breaking_hanging_punct_parsed() {
        use crate::presentation::Presentation;
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/upstream/pptx/e3dab827b836_cjk-line-breaking.pptx"
        ))
        .expect("fixture");
        let mut opened = Presentation::open(&bytes).expect("open");
        let slide = opened.slide(0).expect("slide0");
        let hang = slide
            .elements
            .iter()
            .find(|e| e.text.as_deref().map(|t| t.contains('あ')).unwrap_or(false))
            .expect("hang shape");
        let paras = hang.text_paint_paragraphs.as_ref().expect("paras");
        let opts = paras[0]
            .east_asian_breaks
            .expect("hangingPunct=1 paragraph options");
        assert!(opts.hanging_punctuation);
        assert!(!opts.break_anywhere);
    }

    // Catches: body lvl2 inheriting titleStyle lvl1 italic via shape-level first-run fallback.
    #[test]
    fn upstream_master_text_styles_body_level_two_not_italic() {
        use crate::presentation::Presentation;
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/upstream/pptx/4647c93fad68_master-text-styles.pptx"
        ))
        .expect("fixture");
        let mut opened = Presentation::open(&bytes).expect("open");
        let slide = opened.slide(0).expect("slide0");
        let body = slide
            .elements
            .iter()
            .find(|e| e.placeholder_type.as_deref() == Some("body"))
            .expect("body");
        let paras = body.text_paint_paragraphs.as_ref().expect("paras");
        assert_eq!(paras[0].level_font_italic, Some(true));
        assert_eq!(paras[1].level_font_italic, Some(false));
        assert_eq!(
            paras[0].bullet_marker.as_deref(),
            Some("•"),
            "bodyStyle lvl1 bullet"
        );
    }

    // Catches: modify-password body placeholder losing layout font size (GDI drift vs TS).
    #[test]
    fn modify_password_body_placeholder_font_size() {
        use crate::presentation::Presentation;
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/upstream/pptx/6fbe45df1cb3_modify-password.pptx"
        ))
        .expect("fixture");
        let mut opened = Presentation::open(&bytes).expect("open");
        let slide = opened.slide(0).expect("slide0");
        let body = slide
            .elements
            .iter()
            .find(|e| e.name.as_deref() == Some("Text Placeholder 2"))
            .expect("body placeholder");
        let paras = body.text_paint_paragraphs.as_ref().expect("paras");
        assert_eq!(paras.len(), 1);
        assert!(body.font_size_px.unwrap() > 0.0);
        assert!(paras[0].runs[0].run_font_family.is_some() || body.font_family.is_some());
    }

    // Catches: keyframes Profil effectRef idx=2 metal never gets inset specular so the green button stays flat.
    #[test]
    fn keyframes_profil_effectref_metal_material() {
        use crate::presentation::Presentation;
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../fixtures/real/pptx/poi-keyframes.pptx"
        ))
        .expect("fixture");
        let mut opened = Presentation::open(&bytes).expect("open");
        let slide = opened.slide(0).expect("slide0");
        let profil: Vec<_> = slide
            .elements
            .iter()
            .filter(|e| e.text.as_deref() == Some("Profil"))
            .collect();
        assert!(!profil.is_empty(), "no Profil elements");
        for el in &profil {
            let inset = el
                .material_box_shadow
                .as_deref()
                .expect("Profil effectRef idx=2 metal inset");
            assert!(
                inset.contains("inset -3px -3px 8px"),
                "metal inset specular on Profil, got {inset:?}"
            );
            assert!(
                inset.contains("rgba(255,255,255,") && inset.matches("inset ").count() >= 3,
                "Profil metal+bevelT should stack bevel and material inset layers, got {inset:?}"
            );
            assert!(
                el.material_overlay
                    .as_deref()
                    .is_some_and(|o| o.contains("linear-gradient")),
                "expected material gradient overlay on Profil"
            );
            assert!(
                el.effect_filter
                    .as_deref()
                    .is_some_and(|f| f.contains("brightness(0.88)")),
                "metal+bevel legacy compensation until SVG bevel lighting, got {:?}",
                el.effect_filter
            );
        }
    }

    // Catches: master/layout picture a:glow on spPr never becomes a drop-shadow filter.
    #[test]
    fn picture_sp_pr_glow_sets_effect_filter() {
        let colors = theme();
        let xml = r#"<p:pic xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
          <p:nvPicPr><p:cNvPr id="56" name="Picture 5"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>
          <p:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
          <p:spPr>
            <a:xfrm><a:off x="0" y="0"/><a:ext cx="9144000" cy="6858000"/></a:xfrm>
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
            <a:effectLst><a:glow rad="228600"><a:schemeClr val="accent2"><a:alpha val="40000"/></a:schemeClr></a:glow></a:effectLst>
          </p:spPr>
        </p:pic>"#;
        let el =
            parse_picture(&parse_xml_str(xml).unwrap().root, "p".into(), &ctx(&colors)).unwrap();
        let filter = el.effect_filter.as_deref().expect("picture glow filter");
        assert!(
            filter.contains("drop-shadow(0 0 12px"),
            "poi-keyframes Picture 5 halo radius, got {filter}"
        );
    }

    // Catches: lnRef idx=2 painted at 1px instead of the theme line width.
    #[test]
    fn ln_ref_uses_theme_line_width() {
        let colors = theme();
        let scheme = office_matrix();
        let xml = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="1" name="L"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
          <p:style><a:lnRef idx="2"><a:schemeClr val="accent1"/></a:lnRef></p:style>
        </p:sp>"#;
        let el = parse_shape(
            &parse_xml_str(xml).unwrap().root,
            "l".into(),
            &ctx_with_matrix(&colors, &scheme),
            false,
        )
        .unwrap();
        assert_eq!(el.stroke_color.as_deref(), Some("#4472C4"));
        assert!((el.stroke_width_px.unwrap() - 25400.0 / 9525.0).abs() < 0.01);
    }

    // Catches: p:bg blipFill ignored so picture slide backgrounds never resolve an rId.
    #[test]
    fn background_blip_keeps_embed_id() {
        let xml = r#"<p:cSld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
          <p:bg><p:bgPr><a:blipFill><a:blip r:embed="rId9"/><a:stretch><a:fillRect/></a:stretch></a:blipFill></p:bgPr></p:bg>
        </p:cSld>"#;
        let doc = parse_xml_str(xml).unwrap();
        let paint = first_paintable_background(&[Some(&doc.root)], &theme(), None, None).unwrap();
        assert_eq!(paint.image_rel_id.as_deref(), Some("rId9"));
        assert!(paint.color.is_none());
    }

    // Catches: p:pic a:blip r:embed dropped so the painter has no image relationship.
    #[test]
    fn picture_records_embed_id() {
        let xml = r#"<p:pic xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
          <p:nvPicPr><p:cNvPr id="4" name="Pic"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>
          <p:blipFill><a:blip r:embed="rId3"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
          <p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
        </p:pic>"#;
        let doc = parse_xml_str(xml).unwrap();
        let colors = theme();
        let el = parse_picture(&doc.root, "p".into(), &ctx(&colors)).unwrap();
        assert_eq!(el.image_rel_id.as_deref(), Some("rId3"));
        assert!(el.image_src.is_none());
    }

    // Catches: bgRef idx 1002 flattened to the ref colour instead of the background gradient.
    #[test]
    fn bg_ref_idx_uses_background_fill_list() {
        let scheme = office_matrix();
        let xml = r#"<p:cSld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:bg><p:bgRef idx="1002"><a:schemeClr val="accent1"/></p:bgRef></p:bg>
        </p:cSld>"#;
        let doc = parse_xml_str(xml).unwrap();
        let paint =
            first_paintable_background(&[Some(&doc.root)], &theme(), Some(&scheme), None).unwrap();
        assert_eq!(
            paint.gradient.as_deref(),
            Some("linear-gradient(90.00deg, #4472C4 0%, #0000FF 100%)")
        );
    }

    #[test]
    fn master_clr_map_routes_bg1_to_dk2() {
        // Catches: master clrMap bg1="dk2" ignored so slide chrome paints yellow lt1 instead of blue.
        use nitro_xml_opc::drawingml::ThemeColorSlot;
        let mut colors = BTreeMap::new();
        colors.insert("lt1".to_owned(), "#FFFF00".to_owned());
        colors.insert("dk1".to_owned(), "#000000".to_owned());
        colors.insert("dk2".to_owned(), "#0066CC".to_owned());
        let mut map = ThemeColorMap::new();
        map.insert("bg1".to_owned(), ThemeColorSlot::Dk2);
        let shape = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="1" name="Chrome"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr>
            <a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm>
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
            <a:solidFill><a:schemeClr val="bg1"/></a:solidFill>
          </p:spPr>
        </p:sp>"#;
        let doc = parse_xml_str(shape).unwrap();
        let mut parse = ctx(&colors);
        parse.color_map = Some(&map);
        let el = parse_shape(&doc.root, "s".into(), &parse, false).unwrap();
        assert_eq!(el.fill_color.as_deref(), Some("#0066CC"));

        let bg = r#"<p:cSld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:bg><p:bgPr><a:solidFill><a:schemeClr val="bg1"/></a:solidFill></p:bgPr></p:bg>
        </p:cSld>"#;
        let bg_doc = parse_xml_str(bg).unwrap();
        let paint =
            first_paintable_background(&[Some(&bg_doc.root)], &colors, None, Some(&map)).unwrap();
        assert_eq!(paint.color.as_deref(), Some("#0066CC"));

        let unmapped = shape.replace("val=\"bg1\"", "val=\"tx1\"");
        let el = parse_shape(
            &parse_xml_str(&unmapped).unwrap().root,
            "t".into(),
            &parse,
            false,
        )
        .unwrap();
        assert_eq!(el.fill_color.as_deref(), Some("#000000"));
    }

    #[test]
    fn media_pic_keeps_poster_embed() {
        // Catches: video/audio pic skipped so movie poster never paints.
        let xml = r#"<p:spTree xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main">
          <p:pic>
            <p:nvPicPr>
              <p:cNvPr id="2" name="just-two-mice.mp4"/>
              <p:cNvPicPr/>
              <p:nvPr>
                <a:videoFile r:link="rId2"/>
                <p:extLst>
                  <p:ext uri="{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}">
                    <p14:media r:embed="rId1"/>
                  </p:ext>
                </p:extLst>
              </p:nvPr>
            </p:nvPicPr>
            <p:blipFill><a:blip r:embed="rId4"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
            <p:spPr>
              <a:xfrm><a:off x="2590800" y="571500"/><a:ext cx="3962400" cy="5715000"/></a:xfrm>
              <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
            </p:spPr>
          </p:pic>
        </p:spTree>"#;
        let doc = parse_xml_str(xml).unwrap();
        let colors = theme();
        let parsed = parse_sp_tree(
            &doc.root,
            Some(xml),
            "",
            &ctx(&colors),
            &SpTreeOptions::default(),
        );
        let pic = parsed
            .elements
            .iter()
            .find(|el| el.type_ == "picture")
            .expect("poster picture");
        assert_eq!(pic.image_rel_id.as_deref(), Some("rId4"));
        assert!(pic.width > 0.0 && pic.height > 0.0);
        let (_, kinds) = issue24_dump_parts(&parsed.elements, &parsed.other_kind_seq);
        assert_eq!(kinds, vec!["media".to_owned()]);
        assert!(parsed.other_kinds.is_empty());
    }

    #[test]
    fn ole_frame_takes_vml_preview_rel() {
        // Catches: OLE graphicFrame stays Other(ole) so VML preview image never paints.
        let xml = r#"<p:spTree xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
          <p:graphicFrame>
            <p:nvGraphicFramePr><p:cNvPr id="584709" name="Object 5"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>
            <p:xfrm><a:off x="4038600" y="3733800"/><a:ext cx="1066800" cy="1295400"/></p:xfrm>
            <a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/presentationml/2006/ole">
              <p:oleObj spid="_x0000_s584709" name="Photo Editor Photo" r:id="rId4"><p:embed/></p:oleObj>
            </a:graphicData></a:graphic>
          </p:graphicFrame>
        </p:spTree>"#;
        let vml = r#"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office">
          <v:shape id="_x0000_s584709"><v:imagedata o:relid="rId1" o:title=""/></v:shape>
        </xml>"#;
        let doc = parse_xml_str(xml).unwrap();
        let colors = theme();
        let mut parsed = parse_sp_tree(
            &doc.root,
            Some(xml),
            "",
            &ctx(&colors),
            &SpTreeOptions::default(),
        );
        assert!(parsed.other_kinds.iter().all(|k| k != "ole"));
        let vml_doc = parse_xml_str(vml).unwrap();
        for el in &mut parsed.elements {
            assign_vml_ole_preview(el, "ppt/drawings/vmlDrawing1.vml", &vml_doc.root);
        }
        let ole = parsed
            .elements
            .iter()
            .find(|el| el.type_ == "ole")
            .expect("ole element");
        assert_eq!(ole.ole_spid.as_deref(), Some("_x0000_s584709"));
        assert_eq!(ole.image_rel_id.as_deref(), Some("rId1"));
        assert_eq!(
            ole.image_rel_owner.as_deref(),
            Some("ppt/drawings/vmlDrawing1.vml")
        );
        assert!(ole.width > 0.0 && ole.height > 0.0);
        let (_, kinds) = issue24_dump_parts(&parsed.elements, &parsed.other_kind_seq);
        assert_eq!(kinds, vec!["ole".to_owned()]);
    }

    fn corbel_minor_scheme() -> crate::theme::ThemeFontScheme {
        crate::theme::ThemeFontScheme {
            major_font: None,
            minor_font: Some(crate::theme::ThemeFonts {
                latin: Some("Corbel".to_owned()),
                east_asia: None,
                complex_script: None,
            }),
        }
    }

    // Catches: Selamat-like gradFill ln must not flatten first stop into teal text-stroke.
    #[test]
    fn gradfill_text_outline_leaves_stroke_color_unset_for_current_color() {
        let xml = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="1" name="T"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr>
            <a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm>
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          </p:spPr>
          <p:txBody>
            <a:bodyPr/>
            <a:p><a:r><a:rPr sz="1800" lang="en-US">
              <a:ln w="12700">
                <a:gradFill>
                  <a:gsLst>
                    <a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs>
                    <a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs>
                  </a:gsLst>
                  <a:lin ang="0" scaled="0"/>
                </a:gradFill>
              </a:ln>
            </a:rPr><a:t>Grad</a:t></a:r></a:p>
          </p:txBody>
        </p:sp>"#;
        let doc = parse_xml_str(xml).unwrap();
        let colors = theme();
        let el = parse_shape(&doc.root, "s".into(), &ctx(&colors), false).unwrap();
        assert!(el.text_stroke_width_px.unwrap() > 0.0);
        assert_eq!(el.text_stroke_color, None);
    }

    // Catches: Informal Roman bold + black text outline from first-run a:rPr left unpainted.
    #[test]
    fn first_run_informal_roman_bold_and_text_stroke() {
        let xml = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="1" name="T"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr>
            <a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm>
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          </p:spPr>
          <p:txBody>
            <a:bodyPr/>
            <a:p><a:r><a:rPr sz="5400" b="1" lang="en-US">
              <a:latin typeface="Informal Roman"/>
              <a:ln w="31550"><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:ln>
            </a:rPr><a:t>Selamat</a:t></a:r></a:p>
          </p:txBody>
        </p:sp>"#;
        let doc = parse_xml_str(xml).unwrap();
        let colors = theme();
        let el = parse_shape(&doc.root, "s".into(), &ctx(&colors), false).unwrap();
        assert_eq!(el.font_family.as_deref(), Some("Informal Roman"));
        assert!(el.font_bold);
        assert!((el.font_size_px.unwrap() - 72.0).abs() < 0.05);
        assert!((el.text_stroke_width_px.unwrap() - 31550.0 / 9525.0).abs() < 0.02);
        assert_eq!(el.text_stroke_color.as_deref(), Some("#000000"));
    }

    // Catches: p:style fontRef minor ignored when a:latin is absent so menu labels stay Calibri.
    #[test]
    fn font_ref_minor_resolves_corbel_without_latin() {
        let xml = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="2" name="Btn"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr>
            <a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm>
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          </p:spPr>
          <p:style><a:fontRef idx="minor"><a:schemeClr val="dk1"/></a:fontRef></p:style>
          <p:txBody>
            <a:bodyPr/>
            <a:p><a:r><a:rPr lang="en-US"/><a:t>Menu</a:t></a:r></a:p>
          </p:txBody>
        </p:sp>"#;
        let doc = parse_xml_str(xml).unwrap();
        let colors = theme();
        let scheme = corbel_minor_scheme();
        let parse_ctx = ParseCtx {
            font_scheme: Some(&scheme),
            ..ctx(&colors)
        };
        let el = parse_shape(&doc.root, "s".into(), &parse_ctx, false).unwrap();
        assert_eq!(el.font_family.as_deref(), Some("Corbel"));
    }

    // Catches: p:style fontRef lt1 ignored so accent buttons render black-on-teal instead of white.
    #[test]
    fn font_ref_scheme_clr_resolves_text_color() {
        let xml = r#"<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <p:nvSpPr><p:cNvPr id="2" name="Btn"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
          <p:spPr>
            <a:xfrm><a:off x="0" y="0"/><a:ext cx="952500" cy="952500"/></a:xfrm>
            <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          </p:spPr>
          <p:style><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style>
          <p:txBody>
            <a:bodyPr/>
            <a:p><a:r><a:rPr lang="en-US"/><a:t>BM1</a:t></a:r></a:p>
          </p:txBody>
        </p:sp>"#;
        let doc = parse_xml_str(xml).unwrap();
        let colors = theme();
        let el = parse_shape(&doc.root, "s".into(), &ctx(&colors), false).unwrap();
        assert_eq!(el.text_color.as_deref(), Some("#FFFFFF"));
    }

    // Catches: VML / blip deep searches recursing once per nesting level and overflowing the WASM
    // stack at MAX_XML_DEPTH (4096).
    #[test]
    fn vml_deep_walks_do_not_overflow_stack() {
        let xml = format!("{}{}", "<a>".repeat(4096), "</a>".repeat(4096));
        let root = parse_xml_str(&xml).expect("parses").root;
        assert!(find_local_deep(&root, "missing").is_none());
        assert!(deep_blip_embed(&root).is_none());
        assert!(vml_shape_image_rel(&root, "nope").is_none());
    }
}
