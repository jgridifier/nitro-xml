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
}
