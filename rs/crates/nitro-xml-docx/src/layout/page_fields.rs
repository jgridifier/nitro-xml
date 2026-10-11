// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/layout/page-fields.ts; changes listed in PROVENANCE.md.

use crate::body::DocumentModel;
use crate::fields::{date_field_result, field_name, is_field_locked};
use crate::note_format::format_note_number;
use super::result::LayoutPageBox;

pub fn page_numbers(model: &DocumentModel, pages: &[LayoutPageBox]) -> Vec<String> {
    page_number_values(model, pages)
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let section_index = pages[index].section_index;
            let format = model
                .sections
                .as_ref()
                .and_then(|s| s.get(section_index))
                .and_then(|sec| sec.get("pageNumbering"))
                .and_then(|pn| pn.get("format"))
                .and_then(|v| v.as_str())
                .unwrap_or("decimal");
            format_note_number(value, Some(format))
        })
        .collect()
}

pub fn page_number_values(model: &DocumentModel, pages: &[LayoutPageBox]) -> Vec<i32> {
    let mut previous = 0i32;
    pages
        .iter()
        .map(|page| {
            let start = model
                .sections
                .as_ref()
                .and_then(|s| s.get(page.section_index))
                .and_then(|sec| sec.get("pageNumbering"))
                .and_then(|pn| pn.get("start"))
                .and_then(|v| v.as_i64());
            let value = if start.is_none() {
                previous + 1
            } else {
                start.unwrap() as i32 + page.page_in_section as i32
            };
            previous = value;
            value
        })
        .collect()
}

pub fn field_display_text(run: &serde_json::Value, values: &PageFieldContext) -> String {
    if run.get("field").is_none() || is_field_locked(run) {
        return run.get("text").and_then(|v| v.as_str()).unwrap_or("").into();
    }
    let instr = run.get("field").and_then(|f| f.get("instr")).and_then(|v| v.as_str()).unwrap_or("");
    let name = field_name(instr);
    if name == "PAGE" {
        return values.page.clone();
    }
    if name == "NUMPAGES" {
        return values.num_pages.clone();
    }
    if name == "SECTIONPAGES" {
        return values.section_pages.clone();
    }
    if name == "DATE" || name == "TIME" {
        return date_field_result(&name, instr, values.year, values.month, values.day, values.hour, values.minute);
    }
    run.get("text").and_then(|v| v.as_str()).unwrap_or("").into()
}

pub struct PageFieldContext {
    pub page: String,
    pub num_pages: String,
    pub section_pages: String,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}
