// SPDX-License-Identifier: MIT
//! Section and page-setup edits (#79).
//!
//! Mirrors the model fields mutated by upstream `section-edit.ts`. No corpus
//! ops in `edits/schema.json` yet — host API + undo capture via `DocxHost`.

use nitro_xml_opc::edit::EditError;
use serde_json::{json, Value};

use super::host::DocxHost;

fn section_mut(host: &mut DocxHost, index: usize) -> Result<&mut Value, EditError> {
    let sections = host.sections.get_or_insert_with(|| json!([]));
    let arr = sections.as_array_mut().ok_or_else(|| EditError::Other {
        message: "sections not array".into(),
    })?;
    while arr.len() <= index {
        arr.push(json!({
            "page": {},
            "headers": {},
            "footers": {},
        }));
    }
    Ok(&mut arr[index])
}

/// Set page margins on a section (twips), syncing `host.page` from section 0 when present.
pub fn set_section_margins(
    host: &mut DocxHost,
    section_index: usize,
    top: i64,
    right: i64,
    bottom: i64,
    left: i64,
) -> Result<(), EditError> {
    let section = section_mut(host, section_index)?;
    let page = section
        .as_object_mut()
        .ok_or_else(|| EditError::Other {
            message: "section not object".into(),
        })?
        .entry("page")
        .or_insert_with(|| json!({}));
    if let Some(obj) = page.as_object_mut() {
        obj.insert(
            "margins".into(),
            json!({ "top": top, "right": right, "bottom": bottom, "left": left }),
        );
    }
    if section_index == 0 {
        host.page = Some(page.clone());
    }
    Ok(())
}

/// Set page orientation (`portrait` | `landscape`).
pub fn set_section_orientation(
    host: &mut DocxHost,
    section_index: usize,
    orientation: &str,
) -> Result<(), EditError> {
    if orientation != "portrait" && orientation != "landscape" {
        return Err(EditError::Other {
            message: format!("bad orientation {orientation}"),
        });
    }
    let section = section_mut(host, section_index)?;
    if let Some(obj) = section.as_object_mut() {
        let page = obj.entry("page").or_insert_with(|| json!({}));
        if let Some(p) = page.as_object_mut() {
            p.insert("orientation".into(), json!(orientation));
        }
    }
    if section_index == 0 {
        host.page = section.get("page").cloned();
    }
    Ok(())
}

/// Flip section 0 between portrait and landscape.
///
/// Print layout reads `pageWidthTwips` / `pageHeightTwips`. The writer reads
/// `host.page` width and height in CSS pixels and emits `w:orient="landscape"`
/// when that object carries `orientation`. Swapping both stores keeps a second
/// click equal to the open snapshot, so save takes the clean-host path.
pub fn toggle_page_landscape(host: &mut DocxHost) -> Result<(), EditError> {
    let width_px = host
        .page
        .as_ref()
        .and_then(|page| page.get("width"))
        .cloned()
        .unwrap_or_else(|| json!(816.0));
    let height_px = host
        .page
        .as_ref()
        .and_then(|page| page.get("height"))
        .cloned()
        .unwrap_or_else(|| json!(1056.0));
    let next = {
        let section = section_mut(host, 0)?;
        let width_twips = section
            .get("pageWidthTwips")
            .cloned()
            .unwrap_or_else(|| json!(12240));
        let height_twips = section
            .get("pageHeightTwips")
            .cloned()
            .unwrap_or_else(|| json!(15840));
        let landscape = section.get("orientation").and_then(|v| v.as_str()) == Some("landscape");
        let next = if landscape { "portrait" } else { "landscape" };
        if let Some(obj) = section.as_object_mut() {
            obj.insert("orientation".into(), json!(next));
            obj.insert("pageWidthTwips".into(), height_twips);
            obj.insert("pageHeightTwips".into(), width_twips);
        }
        next
    };
    let page = host.page.get_or_insert_with(|| json!({}));
    if let Some(obj) = page.as_object_mut() {
        obj.insert("width".into(), height_px);
        obj.insert("height".into(), width_px);
        if next == "landscape" {
            obj.insert("orientation".into(), json!("landscape"));
        } else {
            obj.remove("orientation");
        }
    }
    Ok(())
}

const NARROW_MARGIN_TWIPS: i64 = 720;
const NARROW_MARGIN_PX: f64 = 48.0;

const SECTION_MARGIN_KEYS: [&str; 4] = [
    "marginTopTwips",
    "marginRightTwips",
    "marginBottomTwips",
    "marginLeftTwips",
];

const PAGE_MARGIN_KEYS: [&str; 4] = ["marginTop", "marginRight", "marginBottom", "marginLeft"];

fn margin_twips(section: &Value, key: &str) -> i64 {
    section
        .get(key)
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|n| n as i64)))
        .unwrap_or(1440)
}

/// Set every page margin to half an inch, or restore the margins from open.
///
/// Print layout reads `margin*Twips` on the section. The writer reads the
/// CSS-pixel `marginTop` / `marginRight` / `marginBottom` / `marginLeft` fields
/// on `host.page`. Restoring the open snapshot makes a second click clean, so
/// save returns the original package bytes.
pub fn toggle_narrow_margins(host: &mut DocxHost) -> Result<(), EditError> {
    let already = {
        let section = section_mut(host, 0)?;
        SECTION_MARGIN_KEYS
            .iter()
            .all(|key| margin_twips(section, key) == NARROW_MARGIN_TWIPS)
    };
    if already {
        restore_open_margins(host)
    } else {
        apply_narrow_margins(host)
    }
}

fn apply_narrow_margins(host: &mut DocxHost) -> Result<(), EditError> {
    {
        let section = section_mut(host, 0)?;
        if let Some(obj) = section.as_object_mut() {
            for key in SECTION_MARGIN_KEYS {
                obj.insert(key.into(), json!(NARROW_MARGIN_TWIPS));
            }
        }
    }
    let page = host.page.get_or_insert_with(|| json!({}));
    if let Some(obj) = page.as_object_mut() {
        for key in PAGE_MARGIN_KEYS {
            obj.insert(key.into(), json!(NARROW_MARGIN_PX));
        }
    }
    Ok(())
}

fn restore_open_margins(host: &mut DocxHost) -> Result<(), EditError> {
    let section_vals: Vec<Value> = SECTION_MARGIN_KEYS
        .iter()
        .map(|key| {
            host.base_sections
                .as_ref()
                .and_then(|sections| sections.get(0))
                .and_then(|section| section.get(*key))
                .cloned()
                .unwrap_or_else(|| json!(1440))
        })
        .collect();
    let page_vals: Vec<Value> = PAGE_MARGIN_KEYS
        .iter()
        .map(|key| {
            host.base_page
                .as_ref()
                .and_then(|page| page.get(*key))
                .cloned()
                .unwrap_or_else(|| json!(96.0))
        })
        .collect();
    {
        let section = section_mut(host, 0)?;
        if let Some(obj) = section.as_object_mut() {
            for (key, value) in SECTION_MARGIN_KEYS.iter().zip(section_vals) {
                obj.insert((*key).into(), value);
            }
        }
    }
    let page = host.page.get_or_insert_with(|| json!({}));
    if let Some(obj) = page.as_object_mut() {
        for (key, value) in PAGE_MARGIN_KEYS.iter().zip(page_vals) {
            obj.insert((*key).into(), value);
        }
    }
    Ok(())
}

/// Set column count on a section.
pub fn set_section_columns(
    host: &mut DocxHost,
    section_index: usize,
    count: u32,
) -> Result<(), EditError> {
    if count == 0 {
        return Err(EditError::Other {
            message: "column count must be >= 1".into(),
        });
    }
    let section = section_mut(host, section_index)?;
    if let Some(obj) = section.as_object_mut() {
        let page = obj.entry("page").or_insert_with(|| json!({}));
        if let Some(p) = page.as_object_mut() {
            p.insert("columns".into(), json!(count));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_set_margins_writes_section_page_and_syncs_host_page() {
        // Catches: section margin edit updating only host.page and leaving sections empty (#79).
        let mut host = DocxHost::stub(vec![]);
        set_section_margins(&mut host, 0, 1440, 1440, 1440, 1440).unwrap();
        assert_eq!(
            host.sections.as_ref().unwrap()[0]["page"]["margins"]["top"],
            1440
        );
        assert_eq!(host.page.as_ref().unwrap()["margins"]["top"], 1440);
    }

    #[test]
    fn catches_landscape_leaving_pg_sz_portrait_on_a_letter_page() {
        // Catches: Landscape leaving w:pgSz portrait, so the reopened page stays taller than it is wide.
        let bytes = include_bytes!("../../../../../fixtures/generated/docx/pages_10.docx");
        let mut host = DocxHost::open(bytes.as_slice()).expect("open");
        assert!(!host.is_dirty());
        let width = host.sections.as_ref().unwrap()[0]["pageWidthTwips"].clone();
        let height = host.sections.as_ref().unwrap()[0]["pageHeightTwips"].clone();
        assert!(width.as_f64().unwrap() < height.as_f64().unwrap());
        toggle_page_landscape(&mut host).unwrap();
        assert!(host.is_dirty());
        assert_eq!(host.sections.as_ref().unwrap()[0]["pageWidthTwips"], height);
        assert_eq!(host.sections.as_ref().unwrap()[0]["pageHeightTwips"], width);
        assert_eq!(
            host.sections.as_ref().unwrap()[0]["orientation"],
            "landscape"
        );
        assert_eq!(host.page.as_ref().unwrap()["orientation"], "landscape");
        let saved = crate::save_docx(&host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let mut pkg = nitro_xml_opc::opc::Package::open(&saved).expect("pkg");
        let xml = pkg
            .text("word/document.xml")
            .expect("read")
            .expect("document.xml");
        assert!(
            xml.contains("w:orient=\"landscape\""),
            "saved pgSz dropped w:orient: {xml}"
        );
        assert!(xml.contains("w:w=\"15840\""), "saved width stayed portrait");
        assert!(
            xml.contains("w:h=\"12240\""),
            "saved height stayed portrait"
        );
        toggle_page_landscape(&mut host).unwrap();
        assert!(
            !host.is_dirty(),
            "second landscape click left the host dirty"
        );
        let restored = crate::save_docx(&host, bytes.as_slice()).expect("save restored");
        assert_eq!(restored.as_slice(), bytes.as_slice());
    }

    #[test]
    fn catches_landscape_on_an_already_wide_page_writing_portrait_twips() {
        // Catches: Landscape on a page that is already landscape swapping the twips the wrong way, so the page stays wide.
        let mut host = DocxHost::stub(vec![]);
        host.sections = Some(json!([{
            "orientation": "landscape",
            "pageWidthTwips": 15840,
            "pageHeightTwips": 12240
        }]));
        host.base_sections = host.sections.clone();
        host.page = Some(json!({ "width": 1056.0, "height": 816.0, "orientation": "landscape" }));
        host.base_page = host.page.clone();
        toggle_page_landscape(&mut host).unwrap();
        assert_eq!(
            host.sections.as_ref().unwrap()[0]["orientation"],
            "portrait"
        );
        assert_eq!(host.sections.as_ref().unwrap()[0]["pageWidthTwips"], 12240);
        assert_eq!(host.sections.as_ref().unwrap()[0]["pageHeightTwips"], 15840);
        assert!(host.page.as_ref().unwrap().get("orientation").is_none());
        assert_eq!(host.page.as_ref().unwrap()["width"], 816.0);
        assert_eq!(host.page.as_ref().unwrap()["height"], 1056.0);
    }
    #[test]
    fn catches_narrow_margins_leaving_w_pg_mar_at_one_inch() {
        // Catches: Narrow margins leaving w:pgMar at one inch, so the reopened page keeps the wide inset.
        let bytes = include_bytes!("../../../../../fixtures/generated/docx/pages_10.docx");
        let mut host = DocxHost::open(bytes.as_slice()).expect("open");
        assert!(!host.is_dirty());
        toggle_narrow_margins(&mut host).unwrap();
        assert!(host.is_dirty());
        let section = &host.sections.as_ref().unwrap()[0];
        for key in [
            "marginTopTwips",
            "marginRightTwips",
            "marginBottomTwips",
            "marginLeftTwips",
        ] {
            assert_eq!(section[key], 720, "{key}");
        }
        let page = host.page.as_ref().unwrap();
        for key in ["marginTop", "marginRight", "marginBottom", "marginLeft"] {
            assert_eq!(page[key], 48.0, "{key}");
        }
        let saved = crate::save_docx(&host, bytes.as_slice()).expect("save");
        assert_ne!(saved.as_slice(), bytes.as_slice());
        let mut pkg = nitro_xml_opc::opc::Package::open(&saved).expect("pkg");
        let xml = pkg
            .text("word/document.xml")
            .expect("read")
            .expect("document.xml");
        assert!(
            xml.contains("w:top=\"720\""),
            "top margin stayed wide: {xml}"
        );
        assert!(xml.contains("w:left=\"720\""), "left margin stayed wide");
        assert!(xml.contains("w:right=\"720\""), "right margin stayed wide");
        assert!(
            xml.contains("w:bottom=\"720\""),
            "bottom margin stayed wide"
        );
        toggle_narrow_margins(&mut host).unwrap();
        assert!(
            !host.is_dirty(),
            "second narrow-margins click left the host dirty"
        );
        let restored = crate::save_docx(&host, bytes.as_slice()).expect("save restored");
        assert_eq!(restored.as_slice(), bytes.as_slice());
    }

    #[test]
    fn catches_narrow_margins_on_a_wide_left_margin_restoring_that_inset() {
        // Catches: Narrow margins restoring every side to one inch, so a 1.25 inch left margin comes back as 1 inch.
        let mut host = DocxHost::stub(vec![]);
        host.sections = Some(json!([{
            "marginTopTwips": 1440,
            "marginRightTwips": 1800,
            "marginBottomTwips": 1440,
            "marginLeftTwips": 1800
        }]));
        host.base_sections = host.sections.clone();
        host.page = Some(json!({
            "marginTop": 96.0,
            "marginRight": 120.0,
            "marginBottom": 96.0,
            "marginLeft": 120.0
        }));
        host.base_page = host.page.clone();
        toggle_narrow_margins(&mut host).unwrap();
        assert_eq!(host.sections.as_ref().unwrap()[0]["marginLeftTwips"], 720);
        toggle_narrow_margins(&mut host).unwrap();
        assert_eq!(host.sections.as_ref().unwrap()[0]["marginLeftTwips"], 1800);
        assert_eq!(host.sections.as_ref().unwrap()[0]["marginRightTwips"], 1800);
        assert_eq!(host.page.as_ref().unwrap()["marginLeft"], 120.0);
        assert!(!host.is_dirty());
    }
}
