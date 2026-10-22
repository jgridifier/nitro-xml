// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/write-paragraph.ts; changes listed in PROVENANCE.md.

use serde_json::Value;

use nitro_xml_opc::xml::{Element, Node};

use super::element::{first_w_mut, make_w, remove_w_children, set_w_attr};
use super::inline::{collect_inline_slots, replaceable_inline_indices};
use super::run::create_run;
use crate::paragraph_alignment::paragraph_justification_for_write;
pub fn write_paragraph(
    paragraph: &Value,
    node: &Element,
    base: Option<&Value>,
) -> Result<Element, String> {
    if base.is_some_and(|b| paragraph == b) {
        return Ok(node.clone());
    }
    let slots = collect_inline_slots(node).ok_or_else(|| {
        format!(
            "Cannot edit paragraph {}: unsupported inline content",
            paragraph.get("id").and_then(|v| v.as_str()).unwrap_or("?")
        )
    })?;
    let mut out = node.clone();

    if base.is_none()
        || base.and_then(|b| b.get("align")) != paragraph.get("align")
        || base.and_then(|b| b.get("justification")) != paragraph.get("justification")
        || base.and_then(|b| b.get("direction")) != paragraph.get("direction")
    {
        if first_w_mut(&mut out, "pPr").is_none() {
            out.children.insert(0, Node::Element(make_w("pPr")));
        }
        let p_pr = first_w_mut(&mut out, "pPr").expect("pPr");
        remove_w_children(p_pr, "jc");
        if let Some(jc) = paragraph_justification_for_write(paragraph) {
            let mut align_el = make_w("jc");
            set_w_attr(&mut align_el, "val", &jc);
            p_pr.children.push(Node::Element(align_el));
        }
    }

    if indent_differs(paragraph, base) {
        sync_indent(&mut out, paragraph);
    }
    if spacing_differs(paragraph, base) {
        sync_spacing(&mut out, paragraph);
    }
    if field_differs(paragraph, base, "shadingFill") {
        sync_shading(&mut out, paragraph);
    }
    if field_differs(paragraph, base, "borders") {
        sync_borders(&mut out, paragraph);
    }

    let runs = paragraph
        .get("runs")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "paragraph missing runs".to_string())?;
    let base_runs = base.and_then(|b| b.get("runs").and_then(|r| r.as_array()));

    let mut new_runs: Vec<Element> = Vec::new();
    for (i, run) in runs.iter().enumerate() {
        let base_run = base_runs.and_then(|br| br.get(i));
        let old = slots
            .get(i)
            .and_then(|s| s.element.as_ref())
            .or_else(|| slots.first().and_then(|s| s.element.as_ref()));
        new_runs.push(create_run(run, base_run, old));
    }

    let indices = replaceable_inline_indices(&out);
    for (child_idx, run_el) in indices.iter().zip(new_runs.iter()) {
        if *child_idx < out.children.len() {
            out.children[*child_idx] = Node::Element(run_el.clone());
        }
    }
    if new_runs.len() > indices.len() {
        let insert_at = out
            .children
            .iter()
            .position(|n| matches!(n, Node::Element(e) if e.local == "pPr"))
            .map(|i| i + 1)
            .unwrap_or(0)
            + indices.len();
        for run in new_runs.iter().skip(indices.len()) {
            out.children.insert(insert_at, Node::Element(run.clone()));
        }
    } else if new_runs.len() < indices.len() {
        for extra in indices.iter().skip(new_runs.len()) {
            if *extra < out.children.len() {
                out.children.remove(*extra);
            }
        }
    }

    let style_changed = match base {
        None => paragraph
            .get("style")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty()),
        Some(b) => b.get("style") != paragraph.get("style"),
    };
    if style_changed {
        sync_p_style(&mut out, paragraph);
    }

    sync_num_pr(&mut out, paragraph);
    if let Some(p_pr) = first_w_mut(&mut out, "pPr") {
        if p_pr.children.is_empty() {
            out.children
                .retain(|n| !matches!(n, Node::Element(e) if e.local == "pPr"));
        }
    }

    Ok(out)
}

const INDENT_ATTRS: &[(&str, &str)] = &[
    ("indentLeftTwips", "left"),
    ("indentRightTwips", "right"),
    ("indentStartTwips", "start"),
    ("indentEndTwips", "end"),
    ("firstLineTwips", "firstLine"),
    ("hangingTwips", "hanging"),
];

const SPACING_KEYS: &[&str] = &[
    "spacingBeforeTwips",
    "spacingAfterTwips",
    "lineSpacingTwips",
    "lineSpacingRule",
];

fn field_differs(paragraph: &Value, base: Option<&Value>, key: &str) -> bool {
    match base {
        None => paragraph.get(key).is_some(),
        Some(b) => paragraph.get(key) != b.get(key),
    }
}

fn indent_differs(paragraph: &Value, base: Option<&Value>) -> bool {
    INDENT_ATTRS
        .iter()
        .any(|(key, _)| field_differs(paragraph, base, key))
}

fn spacing_differs(paragraph: &Value, base: Option<&Value>) -> bool {
    SPACING_KEYS
        .iter()
        .any(|key| field_differs(paragraph, base, key))
}

fn json_int(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|n| {
        n.as_i64()
            .or_else(|| n.as_u64().map(|u| u as i64))
            .or_else(|| n.as_f64().map(|f| f.round() as i64))
    })
}

/// `w:ind` follows indent fields. A paragraph with none of them drops a stale `w:ind`.
fn sync_indent(out: &mut Element, paragraph: &Value) {
    let any = INDENT_ATTRS
        .iter()
        .any(|(key, _)| paragraph.get(*key).is_some());
    let has = first_w_mut(out, "pPr")
        .and_then(|p| first_w_mut(p, "ind"))
        .is_some();
    if !any && !has {
        return;
    }
    if first_w_mut(out, "pPr").is_none() {
        out.children.insert(0, Node::Element(make_w("pPr")));
    }
    let p_pr = first_w_mut(out, "pPr").expect("pPr");
    remove_w_children(p_pr, "ind");
    if !any {
        return;
    }
    let mut ind = make_w("ind");
    for (key, xml) in INDENT_ATTRS {
        if let Some(n) = json_int(paragraph, key) {
            set_w_attr(&mut ind, xml, &n.to_string());
        }
    }
    if !ind.attrs.is_empty() {
        p_pr.children.push(Node::Element(ind));
    }
}

/// `w:spacing` follows spacing fields, including `w:line` / `w:lineRule`.
fn sync_spacing(out: &mut Element, paragraph: &Value) {
    let before = json_int(paragraph, "spacingBeforeTwips");
    let after = json_int(paragraph, "spacingAfterTwips");
    let line = json_int(paragraph, "lineSpacingTwips");
    let rule = paragraph
        .get("lineSpacingRule")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let any = before.is_some() || after.is_some() || line.is_some() || rule.is_some();
    let has = first_w_mut(out, "pPr")
        .and_then(|p| first_w_mut(p, "spacing"))
        .is_some();
    if !any && !has {
        return;
    }
    if first_w_mut(out, "pPr").is_none() {
        out.children.insert(0, Node::Element(make_w("pPr")));
    }
    let p_pr = first_w_mut(out, "pPr").expect("pPr");
    remove_w_children(p_pr, "spacing");
    if !any {
        return;
    }
    let mut spacing = make_w("spacing");
    if let Some(n) = before {
        set_w_attr(&mut spacing, "before", &n.to_string());
    }
    if let Some(n) = after {
        set_w_attr(&mut spacing, "after", &n.to_string());
    }
    if let Some(n) = line {
        set_w_attr(&mut spacing, "line", &n.to_string());
        set_w_attr(&mut spacing, "lineRule", rule.unwrap_or("auto"));
    } else if let Some(rule) = rule {
        set_w_attr(&mut spacing, "lineRule", rule);
    }
    if !spacing.attrs.is_empty() {
        p_pr.children.push(Node::Element(spacing));
    }
}

/// `w:shd` follows `shadingFill`. An absent field drops a stale shade. An empty string writes
/// `w:fill="auto"` so a styled shade does not stick.
fn sync_shading(out: &mut Element, paragraph: &Value) {
    let fill = paragraph.get("shadingFill").and_then(|v| v.as_str());
    let has = first_w_mut(out, "pPr")
        .and_then(|p| first_w_mut(p, "shd"))
        .is_some();
    if fill.is_none() && !has {
        return;
    }
    if first_w_mut(out, "pPr").is_none() {
        out.children.insert(0, Node::Element(make_w("pPr")));
    }
    let p_pr = first_w_mut(out, "pPr").expect("pPr");
    remove_w_children(p_pr, "shd");
    let Some(fill) = fill else {
        return;
    };
    let mut shd = make_w("shd");
    set_w_attr(&mut shd, "val", "clear");
    set_w_attr(&mut shd, "color", "auto");
    let hex = fill.trim_start_matches('#');
    let fill_attr = if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        hex
    } else {
        "auto"
    };
    set_w_attr(&mut shd, "fill", fill_attr);
    p_pr.children.push(Node::Element(shd));
}

const BORDER_SIDES: &[&str] = &["top", "left", "bottom", "right", "between"];

/// `w:pBdr` follows `borders`. An absent field drops a stale border. `style: "nil"` writes
/// `w:val="nil"` so a styled rule does not stick. Other sides are kept.
fn sync_borders(out: &mut Element, paragraph: &Value) {
    let borders = paragraph.get("borders").filter(|v| v.is_object());
    let has = first_w_mut(out, "pPr")
        .and_then(|p| first_w_mut(p, "pBdr"))
        .is_some();
    if borders.is_none() && !has {
        return;
    }
    if first_w_mut(out, "pPr").is_none() {
        out.children.insert(0, Node::Element(make_w("pPr")));
    }
    let p_pr = first_w_mut(out, "pPr").expect("pPr");
    remove_w_children(p_pr, "pBdr");
    let Some(borders) = borders else {
        return;
    };
    let mut p_bdr = make_w("pBdr");
    for side in BORDER_SIDES {
        if let Some(value) = borders.get(*side) {
            p_bdr
                .children
                .push(Node::Element(border_side_el(side, value)));
        }
    }
    if !p_bdr.children.is_empty() {
        p_pr.children.push(Node::Element(p_bdr));
    }
}

fn border_side_el(name: &str, side: &Value) -> Element {
    let mut el = make_w(name);
    let style = side
        .get("style")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("single");
    set_w_attr(&mut el, "val", style);
    if style == "nil" || style == "none" {
        return el;
    }
    if let Some(n) = json_int(side, "sizeEighthPoints") {
        set_w_attr(&mut el, "sz", &n.to_string());
    }
    if let Some(n) = json_int(side, "spacePoints") {
        set_w_attr(&mut el, "space", &n.to_string());
    }
    if let Some(color) = side
        .get("color")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        set_w_attr(&mut el, "color", color.trim_start_matches('#'));
    }
    el
}

/// `w:pStyle` follows the block's `style` field. An empty style drops a stale `pStyle`.
fn sync_p_style(out: &mut Element, paragraph: &Value) {
    if first_w_mut(out, "pPr").is_none() {
        out.children.insert(0, Node::Element(make_w("pPr")));
    }
    let p_pr = first_w_mut(out, "pPr").expect("pPr");
    remove_w_children(p_pr, "pStyle");
    let Some(style) = paragraph
        .get("style")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    let mut el = make_w("pStyle");
    set_w_attr(&mut el, "val", style);
    p_pr.children.insert(0, Node::Element(el));
}

/// `w:numPr` follows the block's `numbering` field. Absent numbering drops a stale `numPr`.
fn sync_num_pr(out: &mut Element, paragraph: &Value) {
    let numbering = paragraph.get("numbering").filter(|n| !n.is_null());
    let has_num_pr = first_w_mut(out, "pPr")
        .and_then(|p| first_w_mut(p, "numPr"))
        .is_some();
    if numbering.is_none() && !has_num_pr {
        return;
    }
    if first_w_mut(out, "pPr").is_none() {
        out.children.insert(0, Node::Element(make_w("pPr")));
    }
    let p_pr = first_w_mut(out, "pPr").expect("pPr");
    remove_w_children(p_pr, "numPr");
    let Some(numbering) = numbering else {
        return;
    };
    let num_id = numbering
        .get("numId")
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n as u64)))
        .unwrap_or(1);
    let level = numbering
        .get("level")
        .and_then(|v| v.as_u64().or_else(|| v.as_i64().map(|n| n as u64)))
        .unwrap_or(0);
    let mut ilvl = make_w("ilvl");
    set_w_attr(&mut ilvl, "val", &level.to_string());
    let mut num_id_el = make_w("numId");
    set_w_attr(&mut num_id_el, "val", &num_id.to_string());
    let mut num_pr = make_w("numPr");
    num_pr.children.push(Node::Element(ilvl));
    num_pr.children.push(Node::Element(num_id_el));
    p_pr.children.insert(0, Node::Element(num_pr));
}

pub fn create_paragraph(paragraph: &Value) -> Result<Element, String> {
    let node = make_w("p");
    write_paragraph(paragraph, &node, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn shd_fill(el: &Element) -> Option<&str> {
        let props = el.children.iter().find_map(|child| match child {
            Node::Element(e) if e.local == "pPr" => Some(e),
            _ => None,
        })?;
        let shd = props.children.iter().find_map(|child| match child {
            Node::Element(e) if e.local == "shd" => Some(e),
            _ => None,
        })?;
        shd.attrs
            .iter()
            .find(|a| a.local == "fill")
            .map(|a| a.value.as_str())
    }

    fn side_attr<'a>(el: &'a Element, side: &str, attr: &str) -> Option<&'a str> {
        let props = el.children.iter().find_map(|child| match child {
            Node::Element(e) if e.local == "pPr" => Some(e),
            _ => None,
        })?;
        let bdr = props.children.iter().find_map(|child| match child {
            Node::Element(e) if e.local == "pBdr" => Some(e),
            _ => None,
        })?;
        let edge = bdr.children.iter().find_map(|child| match child {
            Node::Element(e) if e.local == side => Some(e),
            _ => None,
        })?;
        edge.attrs
            .iter()
            .find(|a| a.local == attr)
            .map(|a| a.value.as_str())
    }

    #[test]
    fn catches_shade_omitting_w_shd() {
        // Catches: a yellow paragraph saving without w:shd, or a cleared shade leaving the old fill.
        let base = json!({"id": "p0", "runs": [{"text": "Hi"}]});
        let shaded = json!({"id": "p0", "runs": [{"text": "Hi"}], "shadingFill": "#FFFF00"});
        let on = write_paragraph(&shaded, &make_w("p"), Some(&base)).unwrap();
        assert_eq!(shd_fill(&on), Some("FFFF00"));
        let cleared = write_paragraph(&base, &on, Some(&shaded)).unwrap();
        assert_eq!(shd_fill(&cleared), None);
        let auto = write_paragraph(
            &json!({"id": "p0", "runs": [{"text": "Hi"}], "shadingFill": ""}),
            &on,
            Some(&shaded),
        )
        .unwrap();
        assert_eq!(shd_fill(&auto), Some("auto"));
    }

    #[test]
    fn catches_bottom_border_omitting_w_pbdr() {
        // Catches: a bottom border saving without w:pBdr, a cleared border leaving the old rule, or nil omitted.
        let base = json!({"id": "p0", "runs": [{"text": "Hi"}]});
        let bordered = json!({
            "id": "p0",
            "runs": [{"text": "Hi"}],
            "borders": {
                "bottom": {"style": "single", "sizeEighthPoints": 12, "spacePoints": 1, "color": "000000"},
                "left": {"style": "single", "sizeEighthPoints": 4, "color": "FF0000"}
            }
        });
        let on = write_paragraph(&bordered, &make_w("p"), Some(&base)).unwrap();
        assert_eq!(side_attr(&on, "bottom", "val"), Some("single"));
        assert_eq!(side_attr(&on, "bottom", "sz"), Some("12"));
        assert_eq!(side_attr(&on, "bottom", "space"), Some("1"));
        assert_eq!(side_attr(&on, "bottom", "color"), Some("000000"));
        assert_eq!(side_attr(&on, "left", "val"), Some("single"));
        let cleared = write_paragraph(&base, &on, Some(&bordered)).unwrap();
        assert_eq!(side_attr(&cleared, "bottom", "val"), None);
        let nil =
            json!({"id": "p0", "runs": [{"text": "Hi"}], "borders": {"bottom": {"style": "nil"}}});
        let off = write_paragraph(&nil, &on, Some(&bordered)).unwrap();
        assert_eq!(side_attr(&off, "bottom", "val"), Some("nil"));
        assert_eq!(side_attr(&off, "bottom", "sz"), None);
    }
}
