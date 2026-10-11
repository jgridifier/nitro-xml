// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/breaks.ts; changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::{ns, Element};

use crate::xml_util::{get_w, named_w};

pub fn is_cached_page_break(element: &Element) -> bool {
    named_w(element, "lastRenderedPageBreak")
        && element.children.is_empty()
        && element
            .attrs
            .iter()
            .all(|a| a.ns.as_deref() == Some("http://www.w3.org/2000/xmlns/"))
}

pub fn classify_break(element: &Element) -> BreakKind {
    match get_w(Some(element), "type").as_deref() {
        Some("page") => BreakKind::Page,
        Some("column") => BreakKind::Column,
        _ => BreakKind::TextWrapping,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakKind {
    Page,
    Column,
    TextWrapping,
}

pub fn has_special_break(element: &Element) -> bool {
    for attr in &element.attrs {
        if attr.ns.as_deref() == Some("http://www.w3.org/2000/xmlns/") {
            continue;
        }
        if attr.ns.as_deref() != Some(ns::W) {
            return true;
        }
        if attr.local == "type" {
            if attr.value != "textWrapping" {
                return true;
            }
        } else if attr.local == "clear" {
            if attr.value != "none" {
                return true;
            }
        } else {
            return true;
        }
    }
    false
}

pub fn is_modeled_break(element: &Element) -> bool {
    if !matches!(classify_break(element), BreakKind::Page | BreakKind::Column) {
        return false;
    }
    for attr in &element.attrs {
        if attr.ns.as_deref() == Some("http://www.w3.org/2000/xmlns/") {
            continue;
        }
        if attr.ns.as_deref() != Some(ns::W) || attr.local != "type" {
            return false;
        }
    }
    true
}
