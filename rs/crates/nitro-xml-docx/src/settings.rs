// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/document-parts.ts (parseSettings) and
// page-setup-model.ts; changes listed in PROVENANCE.md.

use serde_json::{json, Value};

use nitro_xml_opc::xml::{parse_xml_str, Element};

use crate::simple_types::on_off_element;
use crate::xml_util::{first_w, get_w};

pub fn parse_settings(xml: Option<&str>) -> Value {
    if xml.is_none() {
        return json!({
            "evenAndOddHeaders": false,
            "autoHyphenation": false
        });
    }
    let document = parse_xml_str(xml.unwrap()).expect("settings xml");
    let root = &document.root;
    json!({
        "evenAndOddHeaders": on_off_element(first_w(Some(root), "evenAndOddHeaders")) == Some(true),
        "autoHyphenation": on_off_element(first_w(Some(root), "autoHyphenation")) == Some(true),
    })
}

const TWIPS_PER_INCH: f64 = 1440.0;
const MAX_PAGE_INCHES: f64 = 22.0;
const MIN_TEXT_INCHES: f64 = 0.5;

pub struct PageSetupValues {
    pub top_in: f64,
    pub bottom_in: f64,
    pub left_in: f64,
    pub right_in: f64,
    pub gutter_in: f64,
    pub header_in: f64,
    pub footer_in: f64,
    pub orientation: &'static str,
    pub width_in: f64,
    pub height_in: f64,
}

pub struct SectionProps {
    pub page_width_twips: i64,
    pub page_height_twips: i64,
    pub margin_top_twips: i64,
    pub margin_bottom_twips: i64,
    pub margin_left_twips: i64,
    pub margin_right_twips: i64,
    pub gutter_twips: i64,
    pub header_distance_twips: i64,
    pub footer_distance_twips: i64,
    pub orientation: &'static str,
}

fn inches(value: i64) -> f64 {
    (value as f64 / TWIPS_PER_INCH * 1000.0).round() / 1000.0
}

pub fn read_page_setup(section: &SectionProps) -> PageSetupValues {
    let short = section.page_width_twips.min(section.page_height_twips);
    let long = section.page_width_twips.max(section.page_height_twips);
    PageSetupValues {
        top_in: inches(section.margin_top_twips),
        bottom_in: inches(section.margin_bottom_twips),
        left_in: inches(section.margin_left_twips),
        right_in: inches(section.margin_right_twips),
        gutter_in: inches(section.gutter_twips),
        header_in: inches(section.header_distance_twips),
        footer_in: inches(section.footer_distance_twips),
        orientation: section.orientation,
        width_in: inches(short),
        height_in: inches(long),
    }
}

pub fn validate_page_setup(values: &PageSetupValues) -> Option<&'static str> {
    let margins = [
        values.top_in,
        values.bottom_in,
        values.left_in,
        values.right_in,
        values.gutter_in,
    ];
    let measures: Vec<f64> = margins
        .into_iter()
        .chain([values.header_in, values.footer_in])
        .collect();
    if measures
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0 || *v > MAX_PAGE_INCHES)
    {
        return Some("range");
    }
    if ![values.width_in, values.height_in]
        .iter()
        .all(|v| *v > 0.0 && *v <= MAX_PAGE_INCHES)
    {
        return Some("range");
    }
    let short = values.width_in.min(values.height_in);
    let long = values.width_in.max(values.height_in);
    let (width, height) = if values.orientation == "landscape" {
        (long, short)
    } else {
        (short, long)
    };
    if width - values.left_in - values.right_in - values.gutter_in < MIN_TEXT_INCHES {
        return Some("width");
    }
    if height - values.top_in - values.bottom_in < MIN_TEXT_INCHES {
        return Some("height");
    }
    None
}

#[allow(dead_code)]
pub fn section_from_body_sect_pr(sect_pr: Option<&Element>) -> SectionProps {
    let Some(sect_pr) = sect_pr else {
        return SectionProps {
            page_width_twips: 12240,
            page_height_twips: 15840,
            margin_top_twips: 1440,
            margin_bottom_twips: 1440,
            margin_left_twips: 1440,
            margin_right_twips: 1440,
            gutter_twips: 0,
            header_distance_twips: 720,
            footer_distance_twips: 720,
            orientation: "portrait",
        };
    };
    let size = first_w(Some(sect_pr), "pgSz");
    let margins = first_w(Some(sect_pr), "pgMar");
    let w = get_w(size, "w")
        .and_then(|s| s.parse().ok())
        .unwrap_or(12240);
    let h = get_w(size, "h")
        .and_then(|s| s.parse().ok())
        .unwrap_or(15840);
    let orientation = if get_w(size, "orient").as_deref() == Some("landscape") {
        "landscape"
    } else {
        "portrait"
    };
    SectionProps {
        page_width_twips: w,
        page_height_twips: h,
        margin_top_twips: get_w(margins, "top")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1440),
        margin_bottom_twips: get_w(margins, "bottom")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1440),
        margin_left_twips: get_w(margins, "left")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1440),
        margin_right_twips: get_w(margins, "right")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1440),
        gutter_twips: get_w(margins, "gutter")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        header_distance_twips: get_w(margins, "header")
            .and_then(|s| s.parse().ok())
            .unwrap_or(720),
        footer_distance_twips: get_w(margins, "footer")
            .and_then(|s| s.parse().ok())
            .unwrap_or(720),
        orientation,
    }
}
