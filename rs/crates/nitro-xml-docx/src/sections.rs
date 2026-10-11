// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/sections.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::Element;

use crate::simple_types::{
    on_off_element, parse_on_off, parse_signed_twips, parse_twips, parse_unsigned_integer,
};
use crate::xml_util::{children_w, first_w, get_w, named_w, ElementExt};

pub struct RawHeaderFooterRef {
    pub slot: String,
    pub r_id: String,
}

pub struct RawSection {
    pub props: Value,
    pub header_refs: Vec<RawHeaderFooterRef>,
    pub footer_refs: Vec<RawHeaderFooterRef>,
}

pub fn parse_raw_sections(body: &Element, blocks: &[Value]) -> Vec<RawSection> {
    let mut sections = Vec::new();
    let mut position = 0usize;
    for node in body.element_children() {
        if named_w(node, "p") {
            let sect_pr = first_w(first_w(Some(node), "pPr"), "sectPr");
            if let Some(sp) = sect_pr {
                if let Some(block_id) = blocks
                    .get(position)
                    .and_then(|b| b.get("id"))
                    .and_then(|v| v.as_str())
                {
                    sections.push(parse_one_section(sp, block_id));
                }
            }
            position += 1;
        } else if named_w(node, "tbl") {
            position += 1;
        }
    }
    let final_sect: Vec<_> = children_w(body, "sectPr");
    if let Some(final_sp) = final_sect.last() {
        if let Some(block_id) = blocks
            .last()
            .and_then(|b| b.get("id"))
            .and_then(|v| v.as_str())
        {
            sections.push(parse_one_section(final_sp, block_id));
        }
    }
    sections
}

fn parse_one_section(section: &Element, ends_at_block_id: &str) -> RawSection {
    let size = first_w(Some(section), "pgSz");
    let margins = first_w(Some(section), "pgMar");
    let type_value = get_w(first_w(Some(section), "type"), "val");
    let pg_num = first_w(Some(section), "pgNumType");
    let v_align = get_w(first_w(Some(section), "vAlign"), "val");
    let header_distance = parse_twips(get_w(margins, "header").as_deref());
    let footer_distance = parse_twips(get_w(margins, "footer").as_deref());
    let gutter = parse_twips(get_w(margins, "gutter").as_deref());
    let mut props = serde_json::Map::new();
    props.insert("endsAtBlockId".into(), json!(ends_at_block_id));
    props.insert(
        "type".into(),
        json!(type_value.unwrap_or_else(|| "nextPage".to_string())),
    );
    props.insert(
        "pageWidthTwips".into(),
        json!(parse_twips(get_w(size, "w").as_deref()).unwrap_or(12240)),
    );
    props.insert(
        "pageHeightTwips".into(),
        json!(parse_twips(get_w(size, "h").as_deref()).unwrap_or(15840)),
    );
    props.insert(
        "orientation".into(),
        json!(if get_w(size, "orient").as_deref() == Some("landscape") {
            "landscape"
        } else {
            "portrait"
        }),
    );
    props.insert(
        "marginTopTwips".into(),
        json!(parse_signed_twips(get_w(margins, "top").as_deref()).unwrap_or(1440)),
    );
    props.insert(
        "marginRightTwips".into(),
        json!(parse_twips(get_w(margins, "right").as_deref()).unwrap_or(1440)),
    );
    props.insert(
        "marginBottomTwips".into(),
        json!(parse_signed_twips(get_w(margins, "bottom").as_deref()).unwrap_or(1440)),
    );
    props.insert(
        "marginLeftTwips".into(),
        json!(parse_twips(get_w(margins, "left").as_deref()).unwrap_or(1440)),
    );
    if let Some(h) = header_distance {
        props.insert("headerDistanceTwips".into(), json!(h));
    }
    if let Some(f) = footer_distance {
        props.insert("footerDistanceTwips".into(), json!(f));
    }
    if let Some(g) = gutter {
        props.insert("gutterTwips".into(), json!(g));
    }
    props.insert("columns".into(), parse_columns(section));
    if on_off_element(first_w(Some(section), "titlePg")) == Some(true) {
        props.insert("titlePage".into(), json!(true));
    }
    if matches!(
        v_align.as_deref(),
        Some("top") | Some("center") | Some("bottom")
    ) {
        props.insert("verticalAlign".into(), json!(v_align.unwrap()));
    }
    if pg_num.is_some() {
        let start = parse_unsigned_integer(get_w(pg_num, "start").as_deref());
        let format = get_w(pg_num, "fmt");
        let mut pn = serde_json::Map::new();
        if let Some(s) = start {
            pn.insert("start".into(), json!(s));
        }
        if let Some(f) = format {
            pn.insert("format".into(), json!(f));
        }
        if !pn.is_empty() {
            props.insert("pageNumbering".into(), Value::Object(pn));
        }
    }
    RawSection {
        props: Value::Object(props),
        header_refs: parse_header_footer_refs(section, "headerReference"),
        footer_refs: parse_header_footer_refs(section, "footerReference"),
    }
}

fn parse_columns(section: &Element) -> Value {
    let cols = first_w(Some(section), "cols");
    if cols.is_none() {
        return json!({ "count": 1, "equalWidth": true });
    }
    let cols = cols.unwrap();
    let count = parse_unsigned_integer(get_w(Some(cols), "num").as_deref()).unwrap_or(1);
    let spacing = parse_twips(get_w(Some(cols), "space").as_deref());
    let equal_width_value = get_w(Some(cols), "equalWidth");
    let equal_width =
        equal_width_value.is_none() || parse_on_off(equal_width_value.as_deref()) != Some(false);
    let separator = parse_on_off(get_w(Some(cols), "sep").as_deref()) == Some(true);
    let col_children = children_w(cols, "col");
    let widths = if !col_children.is_empty() {
        Some(
            col_children
                .iter()
                .map(|column| {
                    let space = parse_twips(get_w(Some(column), "space").as_deref());
                    let mut obj = serde_json::Map::new();
                    obj.insert(
                        "widthTwips".into(),
                        json!(parse_twips(get_w(Some(column), "w").as_deref()).unwrap_or(0)),
                    );
                    if let Some(s) = space {
                        obj.insert("spacingTwips".into(), json!(s));
                    }
                    Value::Object(obj)
                })
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };
    let mut out = serde_json::Map::new();
    out.insert("count".into(), json!(count));
    if let Some(s) = spacing {
        out.insert("spacingTwips".into(), json!(s));
    }
    out.insert("equalWidth".into(), json!(equal_width));
    if separator {
        out.insert("separator".into(), json!(true));
    }
    if let Some(w) = widths {
        out.insert("widths".into(), json!(w));
    }
    Value::Object(out)
}

fn parse_header_footer_refs(section: &Element, local_name: &str) -> Vec<RawHeaderFooterRef> {
    let mut refs = Vec::new();
    for element in children_w(section, local_name) {
        let slot_value = get_w(Some(element), "type");
        let slot = match slot_value.as_deref() {
            Some("even") => "even",
            Some("first") => "first",
            _ => "default",
        };
        if let Some(r_id) = get_r_id(element) {
            refs.push(RawHeaderFooterRef {
                slot: slot.to_string(),
                r_id,
            });
        }
    }
    refs
}

fn get_r_id(element: &Element) -> Option<String> {
    use nitro_xml_opc::xml::ns;
    element
        .attrs
        .iter()
        .find(|a| a.local == "id" && a.ns.as_deref() == Some(ns::R))
        .map(|a| a.value.clone())
}
