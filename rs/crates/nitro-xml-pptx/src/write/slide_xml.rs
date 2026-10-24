// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec
// src/core/pptx/core/core/runtime/PptxHandlerRuntimeSaveElementWriter.ts (processSlideElement),
// src/core/pptx/core/core/builders/PptxElementTransformUpdater.ts (applyTransform),
// src/core/pptx/core/builders/factories/TextShapeXmlFactory.ts (createXmlElement),
// src/core/pptx/core/core/runtime/PptxHandlerRuntimeSaveRunProperties.ts (sz/b/i),
// src/core/pptx/core/core/builders/PptxShapeIdValidator.ts (repairShapeIds, two passes
// because a shared rawXml node is repaired again after the first renumber);
// changes listed in PROVENANCE.md.

//! Rewrite one `ppt/slides/slideN.xml` from the editable host.
//!
//! Untouched axes go through [`resolve_xfrm_emu`]. The first run's underline
//! (`a:rPr/@u`), the first paragraph's align (`a:pPr/@algn`), and a list marker
//! (`a:buChar` / `a:buAutoNum` / `a:buNone`) are written.
//! Rich multi-run splits, autofit and notes parts are TODO(#85) / TODO(#86).
//! An authored table (`table_authored`) is written as `p:graphicFrame` / `a:tbl`.
//! A picture inserted this session (`image_png`) is a fresh `p:pic` whose
//! `a:blip/@r:embed` is the id in `pic_rels`.

use std::collections::{HashMap, HashSet};

use nitro_xml_opc::drawingml::{resolve_xfrm_emu, EMU_PER_PIXEL};
use nitro_xml_opc::xml::ns;
use nitro_xml_opc::xml::{parse_xml_str, Attr, Document, Element, Node};

use crate::edit::host::{EditableElement, EditableSlide};
use crate::edit::shapes::{BulletInfoSnap, TextStyleSnap};

const XML_HEADER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>";

const SHAPE_LOCALS: &[&str] = &["sp", "pic", "cxnSp", "graphicFrame", "grpSp", "contentPart"];

/// Largest numeric `@_id` in a slide part (upstream `findMaxElementId` over slide raw XML).
pub fn max_numeric_id(xml: &str) -> Result<i64, String> {
    let doc = parse_xml_str(xml).map_err(|e| e.to_string())?;
    let mut max = 0i64;
    walk(&doc.root, &mut |el| {
        for attr in &el.attrs {
            if attr.local == "id" {
                if let Some(id) = parse_shape_id(&attr.value) {
                    max = max.max(id);
                }
            }
        }
    });
    Ok(max)
}

/// Apply `slide` onto `xml` and return the rewritten part, including the XML declaration.
/// Rewrite `slide` onto `xml`. `pic_rels` maps an inserted picture's element id to its `rId`.
pub fn rewrite_slide_with_pics(
    xml: &str,
    slide: &EditableSlide,
    next_id: &mut i64,
    pic_rels: &HashMap<String, String>,
) -> Result<String, String> {
    let mut doc = parse_xml_str(xml).map_err(|e| e.to_string())?;
    rewrite_document(&mut doc, slide, next_id, pic_rels)?;
    Ok(format!("{XML_HEADER}{}", write_xml(&doc.root)))
}

fn rewrite_document(
    doc: &mut Document,
    slide: &EditableSlide,
    next_id: &mut i64,
    pic_rels: &HashMap<String, String>,
) -> Result<(), String> {
    let sp_tree = sp_tree_mut(&mut doc.root).ok_or_else(|| "p:spTree missing".to_string())?;
    let (prefix, mut nodes, suffix) = split_shapes(sp_tree);
    // Group cNvPr stays on the tree so id repair can renumber it.
    sp_tree.children = prefix;
    let mut id_to_node: HashMap<String, usize> = HashMap::new();
    for el in &slide.elements {
        index_source_nodes(el, nodes.len(), &mut id_to_node);
    }

    let mut order: Vec<usize> = Vec::new();
    for el in &slide.elements {
        if el.type_ == "group" {
            let id = *next_id;
            *next_id += 1;
            let group = new_group_xml(&nodes, &id_to_node, el, id, next_id, pic_rels);
            nodes.push(group);
            order.push(nodes.len() - 1);
            continue;
        }
        if el.clone_xml {
            if let Some(share) = el.xml_share_of.as_deref() {
                if let Some(&idx) = id_to_node.get(share) {
                    if idx < nodes.len() {
                        let mut shape = nodes[idx].clone();
                        ensure_own_xfrm(&mut shape);
                        apply_element(&mut shape, el);
                        nodes.push(shape);
                        order.push(nodes.len() - 1);
                        continue;
                    }
                }
            }
            push_fresh(&mut nodes, &mut order, el, next_id, pic_rels);
            continue;
        }
        if let Some(share) = el.xml_share_of.as_deref() {
            let Some(&idx) = id_to_node.get(share) else {
                push_fresh(&mut nodes, &mut order, el, next_id, pic_rels);
                continue;
            };
            apply_element(&mut nodes[idx], el);
            order.push(idx);
            continue;
        }
        if let Some(idx) = el.source_sp_index {
            if idx < nodes.len() {
                apply_element(&mut nodes[idx], el);
                order.push(idx);
                continue;
            }
        }
        push_fresh(&mut nodes, &mut order, el, next_id, pic_rels);
    }

    // Two passes live inside `repair_shape_ids_mut`: a shared rawXml node is one
    // object, so the first renumber updates every alias, and the structural-id
    // gate renumbers that new duplicate again (upstream save of `duplicateShape`).
    repair_shape_ids_mut(sp_tree, &mut nodes, &order);

    for idx in order {
        sp_tree.children.push(Node::Element(nodes[idx].clone()));
    }
    sp_tree.children.extend(suffix);
    Ok(())
}

fn index_source_nodes(el: &EditableElement, nodes_len: usize, map: &mut HashMap<String, usize>) {
    if let Some(idx) = el.source_sp_index {
        if idx < nodes_len {
            map.insert(el.id.clone(), idx);
        }
    }
    for child in &el.children {
        index_source_nodes(child, nodes_len, map);
    }
}

/// `p:grpSp` for a grouped shape. Child coordinates are group-local: `chOff` is
/// the origin and `chExt` matches the group box, which is how `group_elements` stores them.
fn new_group_xml(
    nodes: &[Element],
    id_to_node: &HashMap<String, usize>,
    el: &EditableElement,
    id: i64,
    next_id: &mut i64,
    pic_rels: &HashMap<String, String>,
) -> Element {
    let emu = EMU_PER_PIXEL as f64;
    let x = resolve_xfrm_emu(el.x, el.x_emu, emu);
    let y = resolve_xfrm_emu(el.y, el.y_emu, emu);
    let cx = resolve_xfrm_emu(el.width, el.width_emu, emu).max(0);
    let cy = resolve_xfrm_emu(el.height, el.height_emu, emu).max(0);

    let mut c_nv_pr = elem(ns::P, "cNvPr");
    set_attr(&mut c_nv_pr, "id", &id.to_string());
    set_attr(&mut c_nv_pr, "name", &format!("Group {id}"));
    let mut nv = elem(ns::P, "nvGrpSpPr");
    nv.children.push(Node::Element(c_nv_pr));
    nv.children.push(Node::Element(elem(ns::P, "cNvGrpSpPr")));
    nv.children.push(Node::Element(elem(ns::P, "nvPr")));

    let mut off = elem(ns::A, "off");
    set_attr(&mut off, "x", &x.to_string());
    set_attr(&mut off, "y", &y.to_string());
    let mut ext = elem(ns::A, "ext");
    set_attr(&mut ext, "cx", &cx.to_string());
    set_attr(&mut ext, "cy", &cy.to_string());
    let mut ch_off = elem(ns::A, "chOff");
    set_attr(&mut ch_off, "x", "0");
    set_attr(&mut ch_off, "y", "0");
    let mut ch_ext = elem(ns::A, "chExt");
    set_attr(&mut ch_ext, "cx", &cx.to_string());
    set_attr(&mut ch_ext, "cy", &cy.to_string());
    let mut xfrm = elem(ns::A, "xfrm");
    xfrm.children.push(Node::Element(off));
    xfrm.children.push(Node::Element(ext));
    xfrm.children.push(Node::Element(ch_off));
    xfrm.children.push(Node::Element(ch_ext));
    let mut grp_sp_pr = elem(ns::P, "grpSpPr");
    grp_sp_pr.children.push(Node::Element(xfrm));

    let mut grp = elem(ns::P, "grpSp");
    grp.children.push(Node::Element(nv));
    grp.children.push(Node::Element(grp_sp_pr));
    for child in &el.children {
        grp.children.push(Node::Element(grouped_child(
            nodes, id_to_node, child, next_id, pic_rels,
        )));
    }
    grp
}

fn grouped_child(
    nodes: &[Element],
    id_to_node: &HashMap<String, usize>,
    child: &EditableElement,
    next_id: &mut i64,
    pic_rels: &HashMap<String, String>,
) -> Element {
    if child.type_ == "group" {
        let id = *next_id;
        *next_id += 1;
        return new_group_xml(nodes, id_to_node, child, id, next_id, pic_rels);
    }
    let idx = child.source_sp_index.or_else(|| {
        child
            .xml_share_of
            .as_deref()
            .and_then(|share| id_to_node.get(share).copied())
    });
    if let Some(idx) = idx {
        if idx < nodes.len() && nodes[idx].local != "grpSp" {
            let mut shape = nodes[idx].clone();
            apply_element(&mut shape, child);
            return shape;
        }
    }
    let id = *next_id;
    *next_id += 1;
    new_shape_xml(child, id, pic_rels)
}

fn push_fresh(
    nodes: &mut Vec<Element>,
    order: &mut Vec<usize>,
    el: &EditableElement,
    next_id: &mut i64,
    pic_rels: &HashMap<String, String>,
) {
    let id = *next_id;
    *next_id += 1;
    let mut shape = new_shape_xml(el, id, pic_rels);
    apply_element(&mut shape, el);
    nodes.push(shape);
    order.push(nodes.len() - 1);
}

fn sp_tree_mut(root: &mut Element) -> Option<&mut Element> {
    let sld = if root.local == "sld" {
        root
    } else {
        child_mut(root, "sld", ns::P)?
    };
    let c_sld = child_mut(sld, "cSld", ns::P)?;
    child_mut(c_sld, "spTree", ns::P)
}

/// Split spTree children into non-shapes before the first shape, shape elements,
/// and non-shapes that followed (for example `p:extLst`).
fn split_shapes(sp_tree: &mut Element) -> (Vec<Node>, Vec<Element>, Vec<Node>) {
    let mut prefix = Vec::new();
    let mut nodes = Vec::new();
    let mut suffix = Vec::new();
    let mut seen_shape = false;
    for child in std::mem::take(&mut sp_tree.children) {
        match child {
            Node::Element(el) if is_shape(&el) => {
                seen_shape = true;
                nodes.push(el);
            }
            other if seen_shape => suffix.push(other),
            other => prefix.push(other),
        }
    }
    (prefix, nodes, suffix)
}

fn is_shape(el: &Element) -> bool {
    el.ns.as_deref() == Some(ns::P) && SHAPE_LOCALS.contains(&el.local.as_str())
}

fn apply_element(shape: &mut Element, el: &EditableElement) {
    apply_xfrm(shape, el);
    if let Some(mode) = el.fill_mode.as_deref() {
        apply_fill(shape, mode, el.fill_color.as_deref());
    }
    if el.stroke_color.is_some() || el.stroke_width.is_some() || el.stroke_compound.is_some() {
        apply_line(
            shape,
            el.stroke_color.as_deref(),
            el.stroke_width,
            el.stroke_compound.as_deref(),
        );
    }
    let authored = el.text_style.as_ref().is_some_and(style_authored);
    if el.text.is_some() || authored {
        update_text(shape, el.text.as_deref(), el.text_style.as_ref());
    }
    apply_paragraph_align(shape, el);
    apply_paragraph_bullet(shape, el);
    apply_paragraph_metrics(shape, el);
    apply_table(shape, el);
}

/// Write `a:off` / `a:ext` with [`resolve_xfrm_emu`]. No-op when the shape has
/// no own transform (upstream `materializeInheritedShapeTransform` also bails
/// without an inherited baseline — these generated slides have neither).
fn apply_xfrm(shape: &mut Element, el: &EditableElement) {
    let Some(sp_pr) = child_mut(shape, "spPr", ns::P) else {
        return;
    };
    let Some(xfrm) = child_mut(sp_pr, "xfrm", ns::A) else {
        return;
    };
    let emu = EMU_PER_PIXEL as f64;
    let x = resolve_xfrm_emu(el.x, el.x_emu, emu);
    let y = resolve_xfrm_emu(el.y, el.y_emu, emu);
    let cx = resolve_xfrm_emu(el.width, el.width_emu, emu);
    let cy = resolve_xfrm_emu(el.height, el.height_emu, emu);
    let rot = (el.rotation * 60_000.0).round() as i64;
    if axis_is(xfrm, "off", "x", x)
        && axis_is(xfrm, "off", "y", y)
        && axis_is(xfrm, "ext", "cx", cx)
        && axis_is(xfrm, "ext", "cy", cy)
        && rot_is(xfrm, rot)
    {
        return;
    }
    set_axis(xfrm, "off", "x", x);
    set_axis(xfrm, "off", "y", y);
    set_axis(xfrm, "ext", "cx", cx);
    set_axis(xfrm, "ext", "cy", cy);
    set_attr(xfrm, "rot", &rot.to_string());
}

fn axis_is(xfrm: &Element, child: &str, attr_name: &str, want: i64) -> bool {
    child_ref(xfrm, child, ns::A)
        .and_then(|el| attr(el, attr_name))
        .and_then(|v| v.parse::<i64>().ok())
        == Some(want)
}

fn set_axis(xfrm: &mut Element, child: &str, attr_name: &str, value: i64) {
    let node = ensure_child(xfrm, child, ns::A);
    set_attr(node, attr_name, &value.to_string());
}

fn rot_is(xfrm: &Element, rot: i64) -> bool {
    match attr(xfrm, "rot") {
        None => rot == 0,
        Some(v) => v == rot.to_string(),
    }
}

fn apply_fill(shape: &mut Element, mode: &str, color: Option<&str>) {
    let sp_pr = ensure_child(shape, "spPr", ns::P);
    remove_children(
        sp_pr,
        ns::A,
        &[
            "solidFill",
            "noFill",
            "gradFill",
            "pattFill",
            "blipFill",
            "grpFill",
        ],
    );
    let node = if mode == "none" {
        elem(ns::A, "noFill")
    } else {
        let mut fill = elem(ns::A, "solidFill");
        let mut clr = elem(ns::A, "srgbClr");
        if let Some(color) = color {
            set_attr(&mut clr, "val", &hex_val(color));
        }
        fill.children.push(Node::Element(clr));
        fill
    };
    insert_before_line(sp_pr, node);
}

fn apply_line(
    shape: &mut Element,
    color: Option<&str>,
    width: Option<f64>,
    compound: Option<&str>,
) {
    let sp_pr = ensure_child(shape, "spPr", ns::P);
    remove_children(sp_pr, ns::A, &["ln"]);
    let mut ln = elem(ns::A, "ln");
    if let Some(width) = width {
        let w = (width * EMU_PER_PIXEL as f64).round() as i64;
        set_attr(&mut ln, "w", &w.to_string());
    }
    if let Some(compound) = compound.filter(|token| {
        let token = token.trim();
        !token.is_empty() && !token.eq_ignore_ascii_case("sng")
    }) {
        set_attr(&mut ln, "cmpd", compound.trim());
    }
    if let Some(color) = color {
        let mut fill = elem(ns::A, "solidFill");
        let mut clr = elem(ns::A, "srgbClr");
        set_attr(&mut clr, "val", &hex_val(color));
        fill.children.push(Node::Element(clr));
        ln.children.push(Node::Element(fill));
    }
    sp_pr.children.push(Node::Element(ln));
}

/// First paragraph `a:pPr/@algn`. Editor tokens (`center`) become OOXML (`ctr`).
fn apply_paragraph_align(shape: &mut Element, el: &EditableElement) {
    let Some(align) = el.text_segments.iter().find_map(|s| s.align.as_deref()) else {
        return;
    };
    let Some(token) = ooxml_algn_token(align) else {
        return;
    };
    let Some(tx) = child_mut(shape, "txBody", ns::P) else {
        return;
    };
    let Some(p) = child_mut(tx, "p", ns::A) else {
        return;
    };
    let p_pr = ensure_child(p, "pPr", ns::A);
    set_attr(p_pr, "algn", token);
}

/// First paragraph list marker. Skipped when the model has no bullet info, so an
/// unrelated edit does not strip a `buFont` the parser did not load.
fn apply_paragraph_bullet(shape: &mut Element, el: &EditableElement) {
    let Some(info) = el.text_segments.iter().find_map(|s| s.bullet_info.as_ref()) else {
        return;
    };
    let Some(tx) = child_mut(shape, "txBody", ns::P) else {
        return;
    };
    let Some(p) = child_mut(tx, "p", ns::A) else {
        return;
    };
    let p_pr = ensure_child(p, "pPr", ns::A);
    remove_children(p_pr, ns::A, &["buNone", "buChar", "buAutoNum"]);
    p_pr.children.insert(0, Node::Element(bullet_marker(info)));
}

/// First paragraph `marL` and `a:lnSpc/a:spcPct`. Absent fields leave the file's values alone.
fn apply_paragraph_metrics(shape: &mut Element, el: &EditableElement) {
    let first = el
        .text_segments
        .iter()
        .find(|s| !s.is_paragraph_break && !s.is_line_break);
    let indent = first.and_then(|s| s.indent_left_emu);
    let spacing = first.and_then(|s| s.line_spacing_pct);
    if indent.is_none() && spacing.is_none() {
        return;
    }
    let Some(tx) = child_mut(shape, "txBody", ns::P) else {
        return;
    };
    let Some(p) = child_mut(tx, "p", ns::A) else {
        return;
    };
    let p_pr = ensure_child(p, "pPr", ns::A);
    if let Some(emu) = indent {
        if emu <= 0 {
            remove_attr(p_pr, "marL");
        } else {
            set_attr(p_pr, "marL", &emu.to_string());
        }
    }
    if let Some(pct) = spacing {
        remove_children(p_pr, ns::A, &["lnSpc"]);
        let mut spc_pct = elem(ns::A, "spcPct");
        set_attr(&mut spc_pct, "val", &pct.to_string());
        let mut ln = elem(ns::A, "lnSpc");
        ln.children.push(Node::Element(spc_pct));
        p_pr.children.push(Node::Element(ln));
    }
}

fn bullet_marker(info: &BulletInfoSnap) -> Element {
    if info.none {
        return elem(ns::A, "buNone");
    }
    if let Some(kind) = info.auto_num_type.as_deref() {
        let mut bu = elem(ns::A, "buAutoNum");
        set_attr(&mut bu, "type", kind);
        let start = info.auto_num_start_at.unwrap_or(1);
        set_attr(&mut bu, "startAt", &start.to_string());
        return bu;
    }
    let mut bu = elem(ns::A, "buChar");
    set_attr(&mut bu, "char", info.char.as_deref().unwrap_or("•"));
    bu
}

fn ooxml_algn_token(align: &str) -> Option<&'static str> {
    Some(match align {
        "left" | "l" => "l",
        "center" | "ctr" => "ctr",
        "right" | "r" => "r",
        "justify" | "just" => "just",
        "justLow" => "justLow",
        "dist" => "dist",
        "thaiDist" => "thaiDist",
        _ => return None,
    })
}

fn insert_before_line(sp_pr: &mut Element, node: Element) {
    let idx = sp_pr.children.iter().position(|c| match c {
        Node::Element(el) => el.local == "ln" && el.ns.as_deref() == Some(ns::A),
        _ => false,
    });
    match idx {
        Some(i) => sp_pr.children.insert(i, Node::Element(node)),
        None => sp_pr.children.push(Node::Element(node)),
    }
}

fn update_text(shape: &mut Element, text: Option<&str>, style: Option<&TextStyleSnap>) -> bool {
    if shape.local == "r" && shape.ns.as_deref() == Some(ns::A) && has_child(shape, "t", ns::A) {
        if let Some(text) = text {
            if let Some(t) = child_mut(shape, "t", ns::A) {
                set_text(t, text);
            }
        }
        if let Some(style) = style {
            if style_authored(style) {
                upsert_rpr(shape, style);
            }
        }
        return true;
    }
    for child in &mut shape.children {
        if let Node::Element(el) = child {
            if update_text(el, text, style) {
                return true;
            }
        }
    }
    false
}

fn style_authored(style: &TextStyleSnap) -> bool {
    style.bold
        || style.italic
        || style.underline.is_some()
        || style.strike.is_some()
        || style.double_strike.is_some()
        || style.small_caps.is_some()
        || style.all_caps.is_some()
        || style.baseline.is_some()
        || style.typeface.is_some()
        || style.font_size.is_some()
        || style.color.is_some()
}

fn upsert_rpr(run: &mut Element, style: &TextStyleSnap) {
    let previous_u = child_ref(run, "rPr", ns::A)
        .and_then(|rpr| attr(rpr, "u"))
        .map(str::to_owned);
    let previous_strike = child_ref(run, "rPr", ns::A)
        .and_then(|rpr| attr(rpr, "strike"))
        .map(str::to_owned);
    let previous_cap = child_ref(run, "rPr", ns::A)
        .and_then(|rpr| attr(rpr, "cap"))
        .map(str::to_owned);
    let previous_baseline = child_ref(run, "rPr", ns::A)
        .and_then(|rpr| attr(rpr, "baseline"))
        .map(str::to_owned);
    let previous_latin = child_ref(run, "rPr", ns::A)
        .and_then(|rpr| child_ref(rpr, "latin", ns::A))
        .cloned();
    let previous_fill = child_ref(run, "rPr", ns::A)
        .and_then(|rpr| child_ref(rpr, "solidFill", ns::A))
        .cloned();
    remove_children(run, ns::A, &["rPr"]);
    let mut rpr = elem(ns::A, "rPr");
    if let Some(size) = style.font_size {
        // CSS px → hundredths of a point. `Math.round(fontSize * (72/96) * 100)`.
        let sz = (size * (72.0 / 96.0) * 100.0).round() as i64;
        set_attr(&mut rpr, "sz", &sz.to_string());
    }
    if style.bold {
        set_attr(&mut rpr, "b", "1");
    }
    if style.italic {
        set_attr(&mut rpr, "i", "1");
    }
    match style.underline {
        Some(true) => set_attr(&mut rpr, "u", "sng"),
        Some(false) => {}
        None => {
            if let Some(u) = previous_u {
                set_attr(&mut rpr, "u", &u);
            }
        }
    }
    // One `@strike`. Double strikethrough wins when both are authored on;
    // either `Some(false)` drops the attribute unless the other flag is `Some(true)`.
    match (style.double_strike, style.strike) {
        (Some(true), _) => set_attr(&mut rpr, "strike", "dblStrike"),
        (_, Some(true)) => set_attr(&mut rpr, "strike", "sngStrike"),
        (Some(false), _) | (_, Some(false)) => {}
        (None, None) => {
            if let Some(strike) = previous_strike {
                set_attr(&mut rpr, "strike", &strike);
            }
        }
    }
    // One `@cap`. All caps wins when both are authored on; either `Some(false)`
    // drops the attribute unless the other flag is `Some(true)`.
    match (style.all_caps, style.small_caps) {
        (Some(true), _) => set_attr(&mut rpr, "cap", "all"),
        (_, Some(true)) => set_attr(&mut rpr, "cap", "small"),
        (Some(false), _) | (_, Some(false)) => {}
        (None, None) => {
            if let Some(cap) = previous_cap {
                set_attr(&mut rpr, "cap", &cap);
            }
        }
    }
    match style.baseline {
        Some(0) => {}
        Some(n) => set_attr(&mut rpr, "baseline", &n.to_string()),
        None => {
            if let Some(baseline) = previous_baseline {
                set_attr(&mut rpr, "baseline", &baseline);
            }
        }
    }
    if let Some(face) = style.typeface.as_deref().filter(|s| !s.is_empty()) {
        let mut latin = elem(ns::A, "latin");
        set_attr(&mut latin, "typeface", face);
        rpr.children.push(Node::Element(latin));
    } else if let Some(latin) = previous_latin {
        rpr.children.push(Node::Element(latin));
    }
    if let Some(color) = style.color.as_deref() {
        let mut fill = elem(ns::A, "solidFill");
        let mut clr = elem(ns::A, "srgbClr");
        set_attr(&mut clr, "val", &hex_val(color));
        fill.children.push(Node::Element(clr));
        rpr.children.push(Node::Element(fill));
    } else if let Some(fill) = previous_fill {
        rpr.children.push(Node::Element(fill));
    }
    let at = run.children.iter().position(|c| match c {
        Node::Element(el) => el.local == "t" && el.ns.as_deref() == Some(ns::A),
        _ => false,
    });
    match at {
        Some(i) => run.children.insert(i, Node::Element(rpr)),
        None => run.children.insert(0, Node::Element(rpr)),
    }
}

fn new_shape_xml(el: &EditableElement, id: i64, pic_rels: &HashMap<String, String>) -> Element {
    if el.type_ == "picture" {
        let rel = pic_rels.get(&el.id).map(String::as_str).unwrap_or("");
        let mut pic = new_picture_xml(el, id, rel);
        apply_element(&mut pic, el);
        return pic;
    }
    if el.type_ == "table" {
        let mut frame = new_table_frame(el, id);
        apply_element(&mut frame, el);
        return frame;
    }
    let emu = EMU_PER_PIXEL as f64;
    let x = resolve_xfrm_emu(el.x, el.x_emu, emu);
    let y = resolve_xfrm_emu(el.y, el.y_emu, emu);
    let cx = resolve_xfrm_emu(el.width, el.width_emu, emu);
    let cy = resolve_xfrm_emu(el.height, el.height_emu, emu);
    let rot = (el.rotation * 60_000.0).round() as i64;
    let prst = el
        .shape_type
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("rect");
    let tx_box = if el.type_ == "text" { "1" } else { "0" };

    let mut c_nv_pr = elem(ns::P, "cNvPr");
    set_attr(&mut c_nv_pr, "id", &id.to_string());
    set_attr(&mut c_nv_pr, "name", &format!("Rectangle {id}"));
    let mut c_nv_sp_pr = elem(ns::P, "cNvSpPr");
    set_attr(&mut c_nv_sp_pr, "txBox", tx_box);
    let mut nv = elem(ns::P, "nvSpPr");
    nv.children.push(Node::Element(c_nv_pr));
    nv.children.push(Node::Element(c_nv_sp_pr));
    nv.children.push(Node::Element(elem(ns::P, "nvPr")));

    let mut off = elem(ns::A, "off");
    set_attr(&mut off, "x", &x.to_string());
    set_attr(&mut off, "y", &y.to_string());
    let mut ext = elem(ns::A, "ext");
    set_attr(&mut ext, "cx", &cx.to_string());
    set_attr(&mut ext, "cy", &cy.to_string());
    let mut xfrm = elem(ns::A, "xfrm");
    set_attr(&mut xfrm, "rot", &rot.to_string());
    xfrm.children.push(Node::Element(off));
    xfrm.children.push(Node::Element(ext));

    let mut geom = elem(ns::A, "prstGeom");
    set_attr(&mut geom, "prst", prst);
    geom.children.push(Node::Element(elem(ns::A, "avLst")));

    let mut sp_pr = elem(ns::P, "spPr");
    sp_pr.children.push(Node::Element(xfrm));
    sp_pr.children.push(Node::Element(geom));
    // Fill and line are applied by `apply_element` so solid vs none share one path.

    let mut body_pr = elem(ns::A, "bodyPr");
    set_attr(&mut body_pr, "wrap", "square");
    let mut rpr = elem(ns::A, "rPr");
    set_attr(&mut rpr, "lang", "en-US");
    let mut t = elem(ns::A, "t");
    if let Some(text) = el.text.as_deref() {
        if !text.is_empty() {
            set_text(&mut t, text);
        }
    }
    let mut r = elem(ns::A, "r");
    r.children.push(Node::Element(rpr));
    r.children.push(Node::Element(t));
    let mut p = elem(ns::A, "p");
    p.children.push(Node::Element(r));
    let mut tx = elem(ns::P, "txBody");
    tx.children.push(Node::Element(body_pr));
    tx.children.push(Node::Element(elem(ns::A, "lstStyle")));
    tx.children.push(Node::Element(p));

    let mut sp = elem(ns::P, "sp");
    sp.children.push(Node::Element(nv));
    sp.children.push(Node::Element(sp_pr));
    sp.children.push(Node::Element(tx));
    apply_element(&mut sp, el);
    // apply_element may rewrite xfrm rot again; new shapes already carry it.
    // A second apply_fill runs when fill_mode is set — new shapes from addShape are.
    sp
}

fn new_picture_xml(el: &EditableElement, id: i64, rel_id: &str) -> Element {
    let emu = EMU_PER_PIXEL as f64;
    let x = resolve_xfrm_emu(el.x, el.x_emu, emu);
    let y = resolve_xfrm_emu(el.y, el.y_emu, emu);
    let cx = resolve_xfrm_emu(el.width, el.width_emu, emu).max(1);
    let cy = resolve_xfrm_emu(el.height, el.height_emu, emu).max(1);

    let mut c_nv_pr = elem(ns::P, "cNvPr");
    set_attr(&mut c_nv_pr, "id", &id.to_string());
    set_attr(&mut c_nv_pr, "name", &format!("Picture {id}"));
    let mut nv = elem(ns::P, "nvPicPr");
    nv.children.push(Node::Element(c_nv_pr));
    nv.children.push(Node::Element(elem(ns::P, "cNvPicPr")));
    nv.children.push(Node::Element(elem(ns::P, "nvPr")));

    let mut blip = elem(ns::A, "blip");
    blip.attrs.push(Attr {
        ns: Some(ns::R.into()),
        local: "embed".to_owned(),
        value: rel_id.to_owned(),
    });
    let mut stretch = elem(ns::A, "stretch");
    stretch
        .children
        .push(Node::Element(elem(ns::A, "fillRect")));
    let mut blip_fill = elem(ns::P, "blipFill");
    blip_fill.children.push(Node::Element(blip));
    blip_fill.children.push(Node::Element(stretch));

    let mut off = elem(ns::A, "off");
    set_attr(&mut off, "x", &x.to_string());
    set_attr(&mut off, "y", &y.to_string());
    let mut ext = elem(ns::A, "ext");
    set_attr(&mut ext, "cx", &cx.to_string());
    set_attr(&mut ext, "cy", &cy.to_string());
    let mut xfrm = elem(ns::A, "xfrm");
    xfrm.children.push(Node::Element(off));
    xfrm.children.push(Node::Element(ext));
    let mut geom = elem(ns::A, "prstGeom");
    set_attr(&mut geom, "prst", "rect");
    geom.children.push(Node::Element(elem(ns::A, "avLst")));
    let mut sp_pr = elem(ns::P, "spPr");
    sp_pr.children.push(Node::Element(xfrm));
    sp_pr.children.push(Node::Element(geom));

    let mut pic = elem(ns::P, "pic");
    pic.children.push(Node::Element(nv));
    pic.children.push(Node::Element(blip_fill));
    pic.children.push(Node::Element(sp_pr));
    pic
}

const TABLE_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/table";

fn new_table_frame(el: &EditableElement, id: i64) -> Element {
    let emu = EMU_PER_PIXEL as f64;
    let x = resolve_xfrm_emu(el.x, el.x_emu, emu);
    let y = resolve_xfrm_emu(el.y, el.y_emu, emu);
    let cx = resolve_xfrm_emu(el.width, el.width_emu, emu).max(1);
    let cy = resolve_xfrm_emu(el.height, el.height_emu, emu).max(1);

    let mut c_nv_pr = elem(ns::P, "cNvPr");
    set_attr(&mut c_nv_pr, "id", &id.to_string());
    set_attr(&mut c_nv_pr, "name", &format!("Table {id}"));
    let mut locks = elem(ns::A, "graphicFrameLocks");
    set_attr(&mut locks, "noGrp", "1");
    let mut c_nv_gf = elem(ns::P, "cNvGraphicFramePr");
    c_nv_gf.children.push(Node::Element(locks));
    let mut nv = elem(ns::P, "nvGraphicFramePr");
    nv.children.push(Node::Element(c_nv_pr));
    nv.children.push(Node::Element(c_nv_gf));
    nv.children.push(Node::Element(elem(ns::P, "nvPr")));

    let mut off = elem(ns::A, "off");
    set_attr(&mut off, "x", &x.to_string());
    set_attr(&mut off, "y", &y.to_string());
    let mut ext = elem(ns::A, "ext");
    set_attr(&mut ext, "cx", &cx.to_string());
    set_attr(&mut ext, "cy", &cy.to_string());
    let mut xfrm = elem(ns::P, "xfrm");
    xfrm.children.push(Node::Element(off));
    xfrm.children.push(Node::Element(ext));

    let tbl = elem(ns::A, "tbl");
    let mut data = elem(ns::A, "graphicData");
    set_attr(&mut data, "uri", TABLE_URI);
    data.children.push(Node::Element(tbl));
    let mut graphic = elem(ns::A, "graphic");
    graphic.children.push(Node::Element(data));

    let mut frame = elem(ns::P, "graphicFrame");
    frame.children.push(Node::Element(nv));
    frame.children.push(Node::Element(xfrm));
    frame.children.push(Node::Element(graphic));
    frame
}

/// Replace `a:tbl` grid and rows from the edited model. `a:tblPr` already on the
/// frame is kept so an existing table style id survives a row or column edit.
fn apply_table(shape: &mut Element, el: &EditableElement) {
    if !el.table_authored {
        return;
    }
    let Some(model) = el.table.as_ref() else {
        return;
    };
    if shape.local != "graphicFrame" || shape.ns.as_deref() != Some(ns::P) {
        return;
    }
    let Some(data) =
        child_mut(shape, "graphic", ns::A).and_then(|g| child_mut(g, "graphicData", ns::A))
    else {
        return;
    };
    let tbl = ensure_child(data, "tbl", ns::A);
    let kept_pr = child_index(tbl, "tblPr", ns::A).and_then(|i| match &tbl.children[i] {
        Node::Element(pr) => Some(pr.clone()),
        Node::Text(_) => None,
    });
    tbl.children.clear();
    tbl.children
        .push(Node::Element(kept_pr.unwrap_or_else(default_tbl_pr)));
    tbl.children.push(Node::Element(table_grid(el, model)));
    for row in &model.rows {
        tbl.children.push(Node::Element(table_row(row)));
    }
}

fn default_tbl_pr() -> Element {
    let mut pr = elem(ns::A, "tblPr");
    set_attr(&mut pr, "firstRow", "1");
    set_attr(&mut pr, "bandRow", "1");
    pr
}

fn table_grid(el: &EditableElement, model: &crate::edit::table::TableModel) -> Element {
    let mut grid = elem(ns::A, "tblGrid");
    for width in column_emus(el, model) {
        let mut col = elem(ns::A, "gridCol");
        set_attr(&mut col, "w", &width.to_string());
        grid.children.push(Node::Element(col));
    }
    grid
}

fn column_emus(el: &EditableElement, model: &crate::edit::table::TableModel) -> Vec<i64> {
    let width = resolve_xfrm_emu(el.width, el.width_emu, EMU_PER_PIXEL as f64).max(1);
    let mut fracs = model.column_widths.clone();
    if fracs.is_empty() {
        let n = model
            .rows
            .iter()
            .map(|row| row.cells.len())
            .max()
            .unwrap_or(1)
            .max(1);
        fracs = vec![1.0 / n as f64; n];
    }
    let sum: f64 = fracs.iter().copied().sum();
    let sum = if sum > 0.0 { sum } else { 1.0 };
    let mut used = 0i64;
    let last = fracs.len().saturating_sub(1);
    fracs
        .iter()
        .enumerate()
        .map(|(i, frac)| {
            if i == last {
                (width - used).max(1)
            } else {
                let w = ((frac / sum) * width as f64).round() as i64;
                let w = w.max(1).min(width - used - (last - i) as i64);
                let w = w.max(1);
                used += w;
                w
            }
        })
        .collect()
}

fn table_row(row: &crate::edit::table::TableRowModel) -> Element {
    let px = row.height.filter(|h| *h > 0.0).unwrap_or(40.0);
    let h = (px * EMU_PER_PIXEL as f64).round() as i64;
    let mut tr = elem(ns::A, "tr");
    set_attr(&mut tr, "h", &h.max(1).to_string());
    for cell in &row.cells {
        tr.children.push(Node::Element(table_cell(cell)));
    }
    tr
}

fn table_cell(cell: &crate::edit::table::TableCellModel) -> Element {
    let mut tc = elem(ns::A, "tc");
    if cell.grid_span > 1 {
        set_attr(&mut tc, "gridSpan", &cell.grid_span.to_string());
    }
    if cell.row_span > 1 {
        set_attr(&mut tc, "rowSpan", &cell.row_span.to_string());
    }
    if cell.h_merge {
        set_attr(&mut tc, "hMerge", "1");
    }
    if cell.v_merge {
        set_attr(&mut tc, "vMerge", "1");
    }
    let mut p = elem(ns::A, "p");
    if cell.text.is_empty() {
        let mut end = elem(ns::A, "endParaRPr");
        set_attr(&mut end, "lang", "en-US");
        p.children.push(Node::Element(end));
    } else {
        let mut rpr = elem(ns::A, "rPr");
        set_attr(&mut rpr, "lang", "en-US");
        let mut t = elem(ns::A, "t");
        set_text(&mut t, &cell.text);
        let mut r = elem(ns::A, "r");
        r.children.push(Node::Element(rpr));
        r.children.push(Node::Element(t));
        p.children.push(Node::Element(r));
    }
    let mut body = elem(ns::A, "txBody");
    body.children.push(Node::Element(elem(ns::A, "bodyPr")));
    body.children.push(Node::Element(elem(ns::A, "lstStyle")));
    body.children.push(Node::Element(p));
    tc.children.push(Node::Element(body));
    tc.children.push(Node::Element(elem(ns::A, "tcPr")));
    tc
}

#[derive(Clone, Copy)]
enum IdTarget {
    Group,
    Shape(usize),
}

/// Shape-id repair that can mutate the spTree group id and the extracted shapes.
///
/// Two passes: the first renumber writes through a shared node (every alias sees
/// the new id); the second pass treats that repeated id as a duplicate again.
fn repair_shape_ids_mut(sp_tree: &mut Element, nodes: &mut [Element], order: &[usize]) {
    for _ in 0..2 {
        let reads = collect_ids(sp_tree, nodes, order);
        let assignments = plan_repairs(&reads);
        for (target, id) in assignments {
            let text = id.to_string();
            match target {
                IdTarget::Group => {
                    if let Some(c) = grp_cnv_mut(sp_tree) {
                        set_attr(c, "id", &text);
                    }
                }
                IdTarget::Shape(idx) => {
                    if let Some(c) = cnv_pr_mut(&mut nodes[idx]) {
                        set_attr(c, "id", &text);
                    }
                }
            }
        }
    }
}

struct IdRead {
    target: IdTarget,
    id: Option<i64>,
}

fn collect_ids(sp_tree: &Element, nodes: &[Element], order: &[usize]) -> Vec<IdRead> {
    let mut out = Vec::new();
    let grp = child_ref(sp_tree, "nvGrpSpPr", ns::P)
        .and_then(|nv| child_ref(nv, "cNvPr", ns::P))
        .and_then(|c| attr(c, "id"))
        .and_then(parse_shape_id);
    out.push(IdRead {
        target: IdTarget::Group,
        id: grp,
    });
    for &idx in order {
        let id = cnv_pr_ref(&nodes[idx]).and_then(|c| attr(c, "id").and_then(parse_shape_id));
        out.push(IdRead {
            target: IdTarget::Shape(idx),
            id,
        });
    }
    out
}

fn plan_repairs(entries: &[IdRead]) -> Vec<(IdTarget, i64)> {
    let mut used: HashSet<i64> = HashSet::new();
    let mut max_id = 0i64;
    let mut invalid: Vec<usize> = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        match entry.id {
            Some(id) if id != 0 && used.insert(id) => {
                max_id = max_id.max(id);
            }
            _ => invalid.push(i),
        }
    }
    let mut assigned = Vec::new();
    for i in invalid {
        max_id += 1;
        used.insert(max_id);
        assigned.push((entries[i].target, max_id));
    }
    assigned
}

fn parse_shape_id(value: &str) -> Option<i64> {
    let text = value.trim();
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let id: i64 = text.parse().ok()?;
    if (0..=u32::MAX as i64).contains(&id) {
        Some(id)
    } else {
        None
    }
}

fn cnv_pr_ref(shape: &Element) -> Option<&Element> {
    for nv_local in [
        "nvSpPr",
        "nvPicPr",
        "nvCxnSpPr",
        "nvGraphicFramePr",
        "nvGrpSpPr",
    ] {
        if let Some(nv) = child_ref(shape, nv_local, ns::P) {
            if let Some(c) = child_ref(nv, "cNvPr", ns::P) {
                return Some(c);
            }
        }
    }
    None
}

fn cnv_pr_mut(shape: &mut Element) -> Option<&mut Element> {
    let nv_local = [
        "nvSpPr",
        "nvPicPr",
        "nvCxnSpPr",
        "nvGraphicFramePr",
        "nvGrpSpPr",
    ]
    .into_iter()
    .find(|local| child_index(shape, local, ns::P).is_some())?;
    let nv_i = child_index(shape, nv_local, ns::P)?;
    let nv = match &mut shape.children[nv_i] {
        Node::Element(el) => el,
        Node::Text(_) => return None,
    };
    let c_i = child_index(nv, "cNvPr", ns::P)?;
    match &mut nv.children[c_i] {
        Node::Element(el) => Some(el),
        Node::Text(_) => None,
    }
}

fn grp_cnv_mut(sp_tree: &mut Element) -> Option<&mut Element> {
    let nv = child_mut(sp_tree, "nvGrpSpPr", ns::P)?;
    child_mut(nv, "cNvPr", ns::P)
}

fn elem(uri: &str, local: &str) -> Element {
    Element {
        ns: Some(uri.into()),
        local: local.to_owned(),
        attrs: Vec::new(),
        children: Vec::new(),
    }
}

fn child_index(el: &Element, local: &str, uri: &str) -> Option<usize> {
    el.children.iter().position(|n| match n {
        Node::Element(e) => e.local == local && e.ns.as_deref() == Some(uri),
        _ => false,
    })
}

fn child_ref<'a>(el: &'a Element, local: &str, uri: &str) -> Option<&'a Element> {
    let i = child_index(el, local, uri)?;
    match &el.children[i] {
        Node::Element(e) => Some(e),
        _ => None,
    }
}

fn child_mut<'a>(el: &'a mut Element, local: &str, uri: &str) -> Option<&'a mut Element> {
    let i = child_index(el, local, uri)?;
    match &mut el.children[i] {
        Node::Element(e) => Some(e),
        _ => None,
    }
}

/// A placeholder often has no `a:xfrm`. Paste needs one so the copy's box is written
/// without changing the source shape.
fn ensure_own_xfrm(shape: &mut Element) {
    ensure_child(shape, "spPr", ns::P);
    {
        let sp_pr = child_mut(shape, "spPr", ns::P).expect("spPr");
        ensure_child(sp_pr, "xfrm", ns::A);
    }
    let sp_pr = child_mut(shape, "spPr", ns::P).expect("spPr");
    let xfrm = child_mut(sp_pr, "xfrm", ns::A).expect("xfrm");
    let _ = ensure_child(xfrm, "off", ns::A);
    let _ = ensure_child(xfrm, "ext", ns::A);
}

fn ensure_child<'a>(el: &'a mut Element, local: &str, uri: &str) -> &'a mut Element {
    if child_index(el, local, uri).is_none() {
        el.children.push(Node::Element(elem(uri, local)));
    }
    child_mut(el, local, uri).expect("child just inserted")
}

fn has_child(el: &Element, local: &str, uri: &str) -> bool {
    child_index(el, local, uri).is_some()
}

fn remove_children(el: &mut Element, uri: &str, locals: &[&str]) {
    el.children.retain(|n| match n {
        Node::Element(e) => !(e.ns.as_deref() == Some(uri) && locals.contains(&e.local.as_str())),
        _ => true,
    });
}

fn attr<'a>(el: &'a Element, local: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|a| a.ns.is_none() && a.local == local)
        .map(|a| a.value.as_str())
}

fn remove_attr(el: &mut Element, local: &str) {
    el.attrs.retain(|a| !(a.ns.is_none() && a.local == local));
}

fn set_attr(el: &mut Element, local: &str, value: &str) {
    if let Some(a) = el
        .attrs
        .iter_mut()
        .find(|a| a.ns.is_none() && a.local == local)
    {
        a.value = value.to_owned();
        return;
    }
    el.attrs.push(Attr {
        ns: None,
        local: local.to_owned(),
        value: value.to_owned(),
    });
}

fn set_text(el: &mut Element, text: &str) {
    el.children.retain(|n| !matches!(n, Node::Text(_)));
    if !text.is_empty() {
        el.children.push(Node::Text(text.to_owned()));
    }
}

fn hex_val(color: &str) -> String {
    color.trim().trim_start_matches('#').to_owned()
}

fn walk(el: &Element, f: &mut impl FnMut(&Element)) {
    f(el);
    for child in &el.children {
        if let Node::Element(e) = child {
            walk(e, f);
        }
    }
}

fn write_xml(root: &Element) -> String {
    let mut out = String::new();
    write_el(root, &mut out, true);
    out
}

fn write_el(el: &Element, out: &mut String, is_root: bool) {
    let pfx = prefix(el.ns.as_deref());
    out.push('<');
    out.push_str(pfx);
    out.push_str(&el.local);
    if is_root {
        out.push_str(" xmlns:a=\"");
        out.push_str(ns::A);
        out.push_str("\" xmlns:r=\"");
        out.push_str(ns::R);
        out.push_str("\" xmlns:p=\"");
        out.push_str(ns::P);
        out.push('"');
    }
    for attr in &el.attrs {
        if attr.local.starts_with("xmlns") {
            continue;
        }
        out.push(' ');
        out.push_str(prefix(attr.ns.as_deref()));
        out.push_str(&attr.local);
        out.push_str("=\"");
        push_escaped(&attr.value, out, true);
        out.push('"');
    }
    let has_child = el.children.iter().any(|c| match c {
        Node::Text(t) => !t.is_empty(),
        Node::Element(_) => true,
    });
    if !has_child {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for child in &el.children {
        match child {
            Node::Text(t) => push_escaped(t, out, false),
            Node::Element(e) => write_el(e, out, false),
        }
    }
    out.push_str("</");
    out.push_str(pfx);
    out.push_str(&el.local);
    out.push('>');
}

fn prefix(uri: Option<&str>) -> &'static str {
    match uri {
        Some(ns::P) => "p:",
        Some(ns::A) => "a:",
        Some(ns::R) => "r:",
        Some(ns::MC) => "mc:",
        _ => "",
    }
}

fn push_escaped(text: &str, out: &mut String, attr: bool) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}
