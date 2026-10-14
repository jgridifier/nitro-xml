// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/pptx/core/core/runtime/
// PptxHandlerRuntimeShapeParagraphContentParsing.ts, PptxHandlerRuntimeBulletParsing.ts and
// ruby-element-parsing.ts; changes in PROVENANCE.md.

use nitro_xml_opc::drawingml::emu_to_pixels;
use nitro_xml_opc::xml::{ns, Element, Node};

use crate::auto_number::{
    break_auto_number_run, format_auto_number_bullet, next_auto_number, AutoNumberSequence,
};
use crate::inheritance::{attr, child_local, children_local, PlaceholderInfo};
use crate::level_style::{
    level_style_for, parse_level_bullet_from_ppr, LevelStylesMap, PlaceholderTextLevelStyle,
};

pub struct TextExtractContext<'a> {
    pub tx_body: Option<&'a Element>,
    pub inherited_tx_body: Option<&'a Element>,
    pub is_body_placeholder: bool,
    pub effective_level_styles: Option<&'a LevelStylesMap>,
    pub presentation_level_styles: Option<&'a LevelStylesMap>,
}

pub(crate) struct BulletInfo {
    pub(crate) none: bool,
    pub(crate) char: Option<String>,
    pub(crate) auto_num_type: Option<String>,
    pub(crate) auto_num_start_at: u32,
    picture: bool,
}

/// Hanging-indent layout in px (upstream `resolveParagraphIndent`).
pub(crate) struct ParagraphIndentLayout {
    pub margin_left_px: Option<f64>,
    pub text_indent_px: Option<f64>,
}

const INDENT_PX_PER_LEVEL: f64 = 18.0;

pub fn extract_element_text(ctx: &TextExtractContext<'_>, tx_body: &Element) -> String {
    let paras = children_local(tx_body, "p", ns::A);
    if paras.is_empty() {
        return tx_body.text_content();
    }
    let mut auto_seq = AutoNumberSequence::default();
    let mut parts = Vec::new();
    for (p_idx, p) in paras.iter().enumerate() {
        collect_paragraph_content(p, p_idx, paras.len(), ctx, &mut auto_seq, &mut parts);
    }
    parts.join("")
}

pub fn text_body_has_renderable_content(tx_body: &Element, ctx: &TextExtractContext<'_>) -> bool {
    let paras = children_local(tx_body, "p", ns::A);
    if paras.is_empty() {
        return scan_text(tx_body);
    }
    for p in paras.iter() {
        if paragraph_has_renderable_content(p) {
            return true;
        }
        let bullet = resolve_paragraph_bullet_info(
            p,
            ctx.tx_body,
            ctx.inherited_tx_body,
            ctx.is_body_placeholder,
            ctx.effective_level_styles,
            ctx.presentation_level_styles,
        );
        if bullet.is_some_and(|b| !b.none && (b.char.is_some() || b.auto_num_type.is_some())) {
            return true;
        }
    }
    false
}

fn collect_paragraph_content(
    p: &Element,
    p_idx: usize,
    para_count: usize,
    ctx: &TextExtractContext<'_>,
    auto_seq: &mut AutoNumberSequence,
    parts: &mut Vec<String>,
) {
    let has_renderable = paragraph_has_renderable_content(p);
    let bullet = resolve_paragraph_bullet_info(
        p,
        ctx.tx_body,
        ctx.inherited_tx_body,
        ctx.is_body_placeholder,
        ctx.effective_level_styles,
        ctx.presentation_level_styles,
    );

    let auto_num_scheme = bullet.as_ref().and_then(|b| {
        if !b.none && b.char.is_none() && !b.picture {
            b.auto_num_type.clone()
        } else {
            None
        }
    });

    let mut auto_num_ordinal: Option<u32> = None;
    let level = paragraph_level(p);
    if let Some(scheme) = &auto_num_scheme {
        if has_renderable {
            let start = bullet.as_ref().map(|b| b.auto_num_start_at).unwrap_or(1);
            auto_num_ordinal = Some(next_auto_number(auto_seq, level, scheme, start));
        }
    } else {
        break_auto_number_run(auto_seq, level);
    }

    let show_bullet_marker = has_renderable || auto_num_scheme.is_none();
    if let Some(ref info) = bullet {
        if !info.none && show_bullet_marker {
            let bullet_text = if let Some(ch) = &info.char {
                format!("{} ", ch)
            } else if let (Some(scheme), Some(ord)) = (&info.auto_num_type, auto_num_ordinal) {
                format_auto_number_bullet(scheme, ord)
            } else if info.picture {
                String::new()
            } else {
                "• ".to_owned()
            };
            if !bullet_text.is_empty() {
                parts.push(bullet_text);
            }
        }
    }

    walk_paragraph_children(p, parts);

    if p_idx + 1 < para_count {
        parts.push("\n".to_owned());
    }
}

/// Upstream inserts synthetic breaks between runs when fast-xml-parser groups
/// `a:r*` before `a:br*` (`insertCollapsedBreaks`). Match that dump string.
fn walk_paragraph_children(p: &Element, parts: &mut Vec<String>) {
    let content: Vec<&Element> = p
        .children
        .iter()
        .filter_map(|c| match c {
            Node::Element(el) if is_paragraph_content_tag(el) => Some(el),
            _ => None,
        })
        .collect();
    let run_count = content
        .iter()
        .filter(|el| el.local == "r" && el.ns.as_deref() == Some(ns::A))
        .count();
    let break_count = content
        .iter()
        .filter(|el| el.local == "br" && el.ns.as_deref() == Some(ns::A))
        .count();
    let grouped = content_is_grouped_by_tag(&content);
    let insert_collapsed = grouped && run_count > 1 && break_count > 0;
    let mut run_index = 0usize;

    for el in content {
        if el.ns.as_deref() == Some(ns::A) {
            match el.local.as_str() {
                "r" => {
                    process_run(el, parts);
                    if insert_collapsed && run_index < run_count.saturating_sub(1).min(break_count)
                    {
                        parts.push("\n".to_owned());
                    }
                    run_index += 1;
                }
                "fld" => process_field(el, parts),
                "t" => parts.push(el.text_content()),
                "br" => parts.push("\n".to_owned()),
                _ => {}
            }
        } else if el.ns.as_deref() == Some(ns::M)
            && matches!(el.local.as_str(), "oMath" | "oMathPara")
        {
            parts.push("[Equation]".to_owned());
        } else if el.ns.as_deref() == Some(ns::MC) && el.local == "AlternateContent" {
            process_alternate_content(el, parts);
        } else if el.local == "m"
            && el
                .ns
                .as_deref()
                .map(|u| u.contains("drawing"))
                .unwrap_or(false)
        {
            parts.push("[Equation]".to_owned());
        }
    }
}

fn is_paragraph_content_tag(el: &Element) -> bool {
    if el.ns.as_deref() == Some(ns::A) {
        return matches!(el.local.as_str(), "r" | "fld" | "t" | "br");
    }
    if el.ns.as_deref() == Some(ns::M) {
        return matches!(el.local.as_str(), "oMath" | "oMathPara");
    }
    if el.ns.as_deref() == Some(ns::MC) && el.local == "AlternateContent" {
        return true;
    }
    el.local == "m"
        && el
            .ns
            .as_deref()
            .map(|u| u.contains("drawing"))
            .unwrap_or(false)
}

fn content_is_grouped_by_tag(content: &[&Element]) -> bool {
    // True when tag runs are contiguous by local name (parser-grouped shape).
    let mut last: Option<&str> = None;
    let mut seen: Vec<&str> = Vec::new();
    for el in content {
        let tag = el.local.as_str();
        if last != Some(tag) {
            if seen.contains(&tag) {
                return false;
            }
            seen.push(tag);
            last = Some(tag);
        }
    }
    true
}

fn process_alternate_content(ac: &Element, parts: &mut Vec<String>) {
    let branch = children_local(ac, "Choice", ns::MC)
        .into_iter()
        .next()
        .or_else(|| children_local(ac, "Fallback", ns::MC).into_iter().next());
    let Some(branch) = branch else {
        return;
    };
    if branch_has_math(branch) {
        parts.push("[Equation]".to_owned());
        return;
    }
    walk_paragraph_children(branch, parts);
}

fn branch_has_math(el: &Element) -> bool {
    let mut stack: Vec<&Element> = vec![el];
    while let Some(node) = stack.pop() {
        for child in node.children.iter().rev() {
            if let Node::Element(e) = child {
                if e.ns.as_deref() == Some(ns::M)
                    && matches!(e.local.as_str(), "oMath" | "oMathPara")
                {
                    return true;
                }
                if e.local == "m" {
                    return true;
                }
                stack.push(e);
            }
        }
    }
    false
}

fn process_run(r: &Element, parts: &mut Vec<String>) {
    if let Some(ruby) = child_local(r, "ruby", ns::A) {
        if let Some(text) = ruby_base_text(ruby) {
            parts.push(text);
            return;
        }
    }
    if let Some(t) = child_local(r, "t", ns::A) {
        parts.push(t.text_content());
    }
}

fn ruby_base_text(ruby: &Element) -> Option<String> {
    let base = child_local(ruby, "rubyBase", ns::A)?;
    let mut out = String::new();
    for child in &base.children {
        if let Node::Element(e) = child {
            if e.local == "r" && e.ns.as_deref() == Some(ns::A) {
                if let Some(t) = child_local(e, "t", ns::A) {
                    out.push_str(&t.text_content());
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn process_field(field: &Element, parts: &mut Vec<String>) {
    if let Some(t) = child_local(field, "t", ns::A) {
        parts.push(t.text_content());
    }
}

#[allow(clippy::single_match)]
pub(crate) fn paragraph_has_renderable_content(p: &Element) -> bool {
    let mut stack: Vec<&Element> = vec![p];
    while let Some(el) = stack.pop() {
        for child in &el.children {
            match child {
                Node::Element(el) => {
                    if el.ns.as_deref() == Some(ns::A)
                        && matches!(el.local.as_str(), "r" | "fld" | "t" | "br")
                    {
                        return true;
                    }
                    if el.ns.as_deref() == Some(ns::M)
                        && matches!(el.local.as_str(), "oMath" | "oMathPara")
                    {
                        return true;
                    }
                    if el.ns.as_deref() == Some(ns::MC) && el.local == "AlternateContent" {
                        if let Some(branch) =
                            children_local(el, "Choice", ns::MC).into_iter().next()
                        {
                            if branch_has_math(branch) {
                                return true;
                            }
                            stack.push(branch);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    false
}

pub(crate) fn paragraph_level(p: &Element) -> i32 {
    let p_pr = child_local(p, "pPr", ns::A);
    let lvl = p_pr
        .and_then(|pp| attr(pp, "lvl"))
        .and_then(|s| s.parse().ok());
    lvl.unwrap_or(0)
}

pub(crate) fn resolve_paragraph_bullet_info(
    paragraph: &Element,
    tx_body: Option<&Element>,
    inherited_tx_body: Option<&Element>,
    is_body_placeholder: bool,
    effective_level_styles: Option<&LevelStylesMap>,
    presentation_level_styles: Option<&LevelStylesMap>,
) -> Option<BulletInfo> {
    let p_pr = child_local(paragraph, "pPr", ns::A);
    if p_pr.is_some_and(|pp| child_local(pp, "buNone", ns::A).is_some()) {
        return Some(BulletInfo {
            none: true,
            char: None,
            auto_num_type: None,
            auto_num_start_at: 1,
            picture: false,
        });
    }

    let level = paragraph_level(paragraph);
    let normalized_level = (level + 1).clamp(1, 9);
    let level_key = format!("lvl{normalized_level}pPr");

    let inherited_level_props = inherited_tx_body
        .and_then(|tb| child_local(tb, "lstStyle", ns::A))
        .and_then(|ls| child_local(ls, &level_key, ns::A));
    let body_level_props = tx_body
        .and_then(|tb| child_local(tb, "lstStyle", ns::A))
        .and_then(|ls| child_local(ls, &level_key, ns::A));
    let default_body_props = tx_body
        .and_then(|tb| child_local(tb, "lstStyle", ns::A))
        .and_then(|ls| child_local(ls, "defPPr", ns::A));
    let inherited_default_body_props = inherited_tx_body
        .and_then(|tb| child_local(tb, "lstStyle", ns::A))
        .and_then(|ls| child_local(ls, "defPPr", ns::A));

    let candidates = [
        p_pr,
        body_level_props,
        inherited_level_props,
        default_body_props,
        inherited_default_body_props,
    ];

    for candidate in candidates.into_iter().flatten() {
        if child_local(candidate, "buNone", ns::A).is_some() {
            return Some(BulletInfo {
                none: true,
                char: None,
                auto_num_type: None,
                auto_num_start_at: 1,
                picture: false,
            });
        }
        if has_bullet_marker(candidate) {
            return bullet_info_from_props(candidate);
        }
    }

    // Body/obj placeholders inherit list bullets from txStyle; title/subtitle do not.
    if !is_body_placeholder {
        return None;
    }
    let mut fallback = effective_level_styles.and_then(|map| {
        level_style_for(map, (normalized_level - 1) as i8).or_else(|| level_style_for(map, -1))
    });
    if fallback.is_none() {
        fallback = presentation_level_styles.and_then(|map| {
            level_style_for(map, (normalized_level - 1) as i8).or_else(|| level_style_for(map, -1))
        });
    }
    fallback.and_then(bullet_info_from_level_style)
}

pub(crate) fn resolve_paragraph_indent_layout(
    paragraph: &Element,
    tx_body: Option<&Element>,
    inherited_tx_body: Option<&Element>,
    is_body_placeholder: bool,
    effective_level_styles: Option<&LevelStylesMap>,
    presentation_level_styles: Option<&LevelStylesMap>,
    has_bullet: bool,
) -> ParagraphIndentLayout {
    let level = paragraph_level(paragraph);
    let normalized_level = (level + 1).clamp(1, 9);
    let level_key = format!("lvl{normalized_level}pPr");

    let p_pr = child_local(paragraph, "pPr", ns::A);
    let inherited_level_props = inherited_tx_body
        .and_then(|tb| child_local(tb, "lstStyle", ns::A))
        .and_then(|ls| child_local(ls, &level_key, ns::A));
    let body_level_props = tx_body
        .and_then(|tb| child_local(tb, "lstStyle", ns::A))
        .and_then(|ls| child_local(ls, &level_key, ns::A));
    let default_body_props = tx_body
        .and_then(|tb| child_local(tb, "lstStyle", ns::A))
        .and_then(|ls| child_local(ls, "defPPr", ns::A));
    let inherited_default_body_props = inherited_tx_body
        .and_then(|tb| child_local(tb, "lstStyle", ns::A))
        .and_then(|ls| child_local(ls, "defPPr", ns::A));

    let candidates = [
        p_pr,
        body_level_props,
        inherited_level_props,
        default_body_props,
        inherited_default_body_props,
    ];

    let mut margin_left = None;
    let mut indent = None;
    for candidate in candidates.into_iter().flatten() {
        let (ml, ind) = paragraph_margin_attrs(candidate);
        if margin_left.is_none() {
            margin_left = ml;
        }
        if indent.is_none() {
            indent = ind;
        }
        if margin_left.is_some() && indent.is_some() {
            break;
        }
    }

    let mut level_style = effective_level_styles.and_then(|map| {
        level_style_for(map, (normalized_level - 1) as i8).or_else(|| level_style_for(map, -1))
    });
    if level_style.is_none() && is_body_placeholder {
        level_style = presentation_level_styles.and_then(|map| {
            level_style_for(map, (normalized_level - 1) as i8).or_else(|| level_style_for(map, -1))
        });
    }
    if let Some(style) = level_style {
        if is_body_placeholder || has_bullet {
            if margin_left.is_none() {
                margin_left = style.margin_left_px;
            }
            if indent.is_none() {
                indent = style.indent_px;
            }
        }
    }

    if margin_left.is_none() && indent.is_none() && level > 0 {
        margin_left = Some(level as f64 * INDENT_PX_PER_LEVEL);
    }

    let margin_left_px = margin_left.filter(|v| *v != 0.0);
    let text_indent_px = indent.and_then(|ind| {
        if ind == 0.0 {
            return None;
        }
        if ind >= 0.0 || margin_left_px.is_some() || has_bullet {
            Some(ind)
        } else {
            None
        }
    });

    ParagraphIndentLayout {
        margin_left_px,
        text_indent_px,
    }
}

fn paragraph_margin_attrs(p_pr: &Element) -> (Option<f64>, Option<f64>) {
    let mar_l = attr(p_pr, "marL")
        .and_then(|s| s.parse::<i64>().ok())
        .map(emu_to_pixels);
    let indent = attr(p_pr, "indent")
        .and_then(|s| s.parse::<i64>().ok())
        .map(emu_to_pixels);
    (mar_l, indent)
}

fn has_bullet_marker(props: &Element) -> bool {
    child_local(props, "buChar", ns::A).is_some()
        || child_local(props, "buAutoNum", ns::A).is_some()
        || child_local(props, "buBlip", ns::A).is_some()
        || child_local(props, "buFontTx", ns::A).is_some()
        || child_local(props, "buClrTx", ns::A).is_some()
        || child_local(props, "buSzTx", ns::A).is_some()
}

fn bullet_info_from_props(props: &Element) -> Option<BulletInfo> {
    let parsed = parse_level_bullet_from_ppr(props);
    if parsed.bullet_none {
        return Some(BulletInfo {
            none: true,
            char: None,
            auto_num_type: None,
            auto_num_start_at: 1,
            picture: false,
        });
    }
    let picture = child_local(props, "buBlip", ns::A).is_some();
    Some(BulletInfo {
        none: false,
        char: parsed.bullet_char,
        auto_num_type: parsed.bullet_auto_num_type,
        auto_num_start_at: parsed.bullet_auto_num_start_at,
        picture,
    })
}

fn bullet_info_from_level_style(style: &PlaceholderTextLevelStyle) -> Option<BulletInfo> {
    if style.bullet_none {
        return Some(BulletInfo {
            none: true,
            char: None,
            auto_num_type: None,
            auto_num_start_at: 1,
            picture: false,
        });
    }
    if style.bullet_char.is_some() || style.bullet_auto_num_type.is_some() {
        return Some(BulletInfo {
            none: false,
            char: style.bullet_char.clone(),
            auto_num_type: style.bullet_auto_num_type.clone(),
            auto_num_start_at: style.bullet_auto_num_start_at,
            picture: false,
        });
    }
    None
}

fn scan_text(node: &Element) -> bool {
    let mut stack: Vec<&Element> = vec![node];
    while let Some(cur) = stack.pop() {
        if cur.local == "t" && cur.ns.as_deref() == Some(ns::A) {
            if !cur.text_content().trim().is_empty() {
                return true;
            }
            continue;
        }
        for c in cur.children.iter().rev() {
            if let Node::Element(e) = c {
                stack.push(e);
            }
        }
    }
    false
}

pub fn placeholder_is_body_type(ph: &PlaceholderInfo) -> bool {
    matches!(ph.type_.as_deref(), Some("body") | Some("obj") | None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nitro_xml_opc::xml::parse_xml_str;

    // Catches: inline Office Math in a paragraph omitted from flattened dump text.
    #[test]
    fn inline_omath_yields_equation_marker() {
        let doc = parse_xml_str(
            r#"<p:txBody xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
 xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
 xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">
 <a:p><a:r><a:t>Eq </a:t></a:r><m:oMath><m:r><m:t>x</m:t></m:r></m:oMath></a:p></p:txBody>"#,
        )
        .unwrap();
        let tx = doc.root.clone();
        let ctx = TextExtractContext {
            tx_body: Some(&tx),
            inherited_tx_body: None,
            is_body_placeholder: false,
            effective_level_styles: None,
            presentation_level_styles: None,
        };
        assert_eq!(extract_element_text(&ctx, &tx), "Eq [Equation]");
    }

    // Catches: `scan_text` / `branch_has_math` recursing once per nesting level and overflowing the
    // WASM stack at MAX_XML_DEPTH (4096).
    #[test]
    fn scan_text_deep_tree_does_not_overflow_stack() {
        let xml = format!("{}{}", "<a>".repeat(4096), "</a>".repeat(4096));
        let root = parse_xml_str(&xml).expect("parses").root;
        assert!(!scan_text(&root));
        assert!(!branch_has_math(&root));
    }
}
