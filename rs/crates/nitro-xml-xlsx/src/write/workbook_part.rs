// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/write/workbook-part.ts; changes listed in PROVENANCE.md.
//! `workbook.xml`: sheets, defined names, and `calcPr fullCalcOnLoad="1"` on a dirty save.

use nitro_xml_opc::xml::ns;
use nitro_xml_opc::xml::{parse_xml_str, Element};

use crate::model::{DefinedName, SheetState, WorkbookSlice};

use super::fragments::direct_child_xml;
use super::xml_out::{attr_str, attr_u32, escape_attr, escape_text, XML_HEADER};

const KEPT_BEFORE_SHEETS: &[&str] = &["fileVersion", "fileSharing"];
const KEPT_AFTER_SHEETS: &[&str] = &["functionGroups", "externalReferences"];
const KEPT_TAIL: &[&str] = &[
    "oleSize",
    "customWorkbookViews",
    "pivotCaches",
    "smartTagPr",
    "smartTagTypes",
    "webPublishing",
    "fileRecoveryPr",
    "webPublishObjects",
    "extLst",
];

pub struct SheetRef {
    pub name: String,
    pub sheet_id: u32,
    pub state: SheetState,
    pub rel_id: String,
}

fn child<'a>(root: &'a Element, name: &str) -> Option<&'a Element> {
    root.children.iter().find_map(|node| match node {
        nitro_xml_opc::xml::Node::Element(el) if el.local == name => Some(el),
        _ => None,
    })
}

fn copy_attrs(node: Option<&Element>, skip: &[&str]) -> String {
    let Some(node) = node else {
        return String::new();
    };
    let mut out = String::new();
    for attr in &node.attrs {
        if skip.contains(&attr.local.as_str()) {
            continue;
        }
        if attr.ns.is_some() {
            continue;
        }
        out.push_str(&format!(" {}=\"{}\"", attr.local, escape_attr(&attr.value)));
    }
    out
}

fn raw_named(children: &[(String, String)], names: &[&str]) -> String {
    let mut out = String::new();
    for name in names {
        for (local, xml) in children {
            if local == name {
                out.push_str(xml);
            }
        }
    }
    out
}

/// Builds `workbook.xml`. `full_calc_on_load` is the dirty-save flag (negative control clears it).
pub fn workbook_xml(
    workbook: &WorkbookSlice,
    sheets: &[SheetRef],
    active_tab: u32,
    source_xml: Option<&str>,
    full_calc_on_load: bool,
) -> String {
    let children = source_xml.map(direct_child_xml).unwrap_or_default();
    let parsed = source_xml.and_then(|xml| parse_xml_str(xml).ok());
    let root = parsed.as_ref().map(|doc| &doc.root);
    let mut out = format!(
        "{XML_HEADER}<workbook xmlns=\"{}\" xmlns:r=\"{}\">",
        ns::X,
        ns::R
    );
    let before = raw_named(&children, KEPT_BEFORE_SHEETS);
    if before.is_empty() {
        out.push_str(
            "<fileVersion appName=\"xl\" lastEdited=\"7\" lowestEdited=\"7\" rupBuild=\"27328\"/>",
        );
    } else {
        out.push_str(&before);
    }
    let date = if workbook.date1904 {
        " date1904=\"1\""
    } else {
        ""
    };
    out.push_str(&format!(
        "<workbookPr{}{date}/>",
        copy_attrs(root.and_then(|r| child(r, "workbookPr")), &["date1904"])
    ));
    let view = root.and_then(|r| child(r, "bookViews")).and_then(|views| {
        views.children.iter().find_map(|node| match node {
            nitro_xml_opc::xml::Node::Element(el) if el.local == "workbookView" => Some(el),
            _ => None,
        })
    });
    let active = if active_tab > 0 {
        format!(" activeTab=\"{active_tab}\"")
    } else {
        String::new()
    };
    out.push_str(&format!(
        "<bookViews><workbookView{}{active}/></bookViews>",
        copy_attrs(view, &["activeTab", "firstSheet"])
    ));
    out.push_str("<sheets>");
    for sheet in sheets {
        let state = match sheet.state {
            SheetState::Visible => None,
            other => Some(other.as_str()),
        };
        let rel_id = format!(" r:id=\"{}\"", escape_attr(&sheet.rel_id));
        let attrs = format!(
            "{}{}{}{rel_id}",
            attr_str("name", Some(&sheet.name)),
            attr_u32("sheetId", Some(sheet.sheet_id)),
            attr_str("state", state),
        );
        out.push_str(&format!("<sheet{attrs}/>"));
    }
    out.push_str("</sheets>");
    out.push_str(&raw_named(&children, KEPT_AFTER_SHEETS));
    if !workbook.defined_names.is_empty() {
        out.push_str("<definedNames>");
        for name in &workbook.defined_names {
            out.push_str(&defined_name_xml(name));
        }
        out.push_str("</definedNames>");
    }
    let calc = root.and_then(|r| child(r, "calcPr"));
    let mode = if workbook.calc_mode.as_deref() == Some("manual") {
        " calcMode=\"manual\""
    } else {
        ""
    };
    let iterate = if let Some((count, delta)) = workbook.iterate {
        format!(" iterate=\"1\" iterateCount=\"{count}\" iterateDelta=\"{delta}\"")
    } else {
        String::new()
    };
    let full = if full_calc_on_load {
        " fullCalcOnLoad=\"1\""
    } else {
        ""
    };
    out.push_str(&format!(
        "<calcPr{} calcId=\"191029\"{mode}{iterate}{full}/>",
        copy_attrs(
            calc,
            &[
                "fullCalcOnLoad",
                "calcId",
                "calcMode",
                "iterate",
                "iterateCount",
                "iterateDelta",
            ]
        )
    ));
    out.push_str(&raw_named(&children, KEPT_TAIL));
    out.push_str("</workbook>");
    out
}

fn defined_name_xml(name: &DefinedName) -> String {
    let hidden = if name.hidden == Some(true) {
        Some("1")
    } else {
        None
    };
    let local = name.local_sheet.map(|id| id.to_string());
    format!(
        "<definedName{}{}{}{}>{}</definedName>",
        attr_str("name", Some(&name.name)),
        attr_str("comment", name.comment.as_deref()),
        attr_str("localSheetId", local.as_deref()),
        attr_str("hidden", hidden),
        escape_text(&name.formula),
    )
}
