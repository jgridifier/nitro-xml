// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/docx/parse-warnings.ts and parse.ts warning
// assembly; changes listed in PROVENANCE.md.

use nitro_xml_opc::xml::Document;

use crate::breaks::{has_special_break, is_modeled_break};
use crate::xml_util::{by_name_w, children_w, get_w, text_content};

pub fn warnings_for(document: &Document) -> Vec<String> {
    let mut warnings = Vec::new();
    let features: [(&[&str], &str); 7] = [
        (
            &["object"],
            "Embedded OLE objects (w:object) are preserved but rendered as a placeholder.",
        ),
        (
            &["altChunk"],
            "Embedded alternate-format content is not represented in the document model.",
        ),
        (
            &["fldSimple", "instrText", "fldChar"],
            "Fields show the result Word last saved while editing; Print Layout recalculates PAGE, NUMPAGES, SECTIONPAGES, DATE and TIME, and tables of contents update from the References tab.",
        ),
        (
            &["moveFrom", "moveTo"],
            "Moved text is linked by its move name: accepting or rejecting either side resolves both. Moves are shown and saved with their range markers.",
        ),
        (
            &["rPrChange"],
            "Formatting-change revisions retain their prior run-properties snapshots and support acceptance and rejection; recording new formatting changes and showing prior formatting in Original display are not supported yet.",
        ),
        (
            &["pPrChange"],
            "Paragraph-formatting-change revisions retain their prior properties snapshots and support acceptance and rejection; recording new paragraph formatting changes and showing prior formatting in Original display are not supported yet.",
        ),
        (
            &["tblPrChange", "trPrChange", "tcPrChange"],
            "Table-structure tracked changes are preserved in the source XML but are not represented in the document model.",
        ),
    ];
    for (names, message) in features {
        if names
            .iter()
            .any(|name| !by_name_w(&document.root, name).is_empty())
        {
            warnings.push(message.to_string());
        }
    }
    for el in by_name_w(&document.root, "instrText") {
        if text_content(el).to_uppercase().contains("HYPERLINK") {
            warnings.push(
                "Complex HYPERLINK field codes (w:fldChar/w:instrText) are preserved but not parsed as links; their result text edits as plain text.".to_string(),
            );
            break;
        }
    }
    let special_break = by_name_w(&document.root, "br")
        .iter()
        .any(|br| has_special_break(br) && !is_modeled_break(br))
        || by_name_w(&document.root, "cr")
            .iter()
            .any(|cr| !cr.attrs.is_empty());
    if special_break {
        warnings.push(
            "Some non-line breaks other than page and column breaks are not distinguished from line breaks in the document model; edits to paragraphs containing them are rejected to preserve the original XML.".to_string(),
        );
    }
    warnings
}

pub fn for_each_paragraph(blocks: &[serde_json::Value], visit: &mut dyn FnMut(&serde_json::Value)) {
    for block in blocks {
        if block.get("type").and_then(|v| v.as_str()) == Some("paragraph") {
            visit(block);
        } else if let Some(rows) = block.get("rows").and_then(|v| v.as_array()) {
            for row in rows {
                if let Some(cells) = row.as_array() {
                    for cell in cells {
                        if let Some(paragraphs) = cell.get("paragraphs").and_then(|v| v.as_array())
                        {
                            for p in paragraphs {
                                visit(p);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn image_and_bookmark_warnings(blocks: &[serde_json::Value]) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut has_picture = false;
    let mut has_anchored = false;
    let mut unsupported_kinds = std::collections::BTreeSet::new();
    let mut has_bookmarks = false;
    for_each_paragraph(blocks, &mut |paragraph| {
        if paragraph.get("bookmarks").is_some() {
            has_bookmarks = true;
        }
        if let Some(runs) = paragraph.get("runs").and_then(|v| v.as_array()) {
            for run in runs {
                if let Some(image) = run.get("image") {
                    if let Some(u) = image.get("unsupported").and_then(|v| v.as_str()) {
                        let diagram_rendering = image
                            .get("diagram")
                            .and_then(|d| d.get("rendering"))
                            .and_then(|v| v.as_str());
                        if diagram_rendering != Some("cached-drawing") {
                            unsupported_kinds.insert(u.to_string());
                        }
                    } else {
                        has_picture = true;
                    }
                    if image.get("anchored").and_then(|v| v.as_bool()) == Some(true) {
                        has_anchored = true;
                    }
                }
            }
        }
    });
    if has_picture {
        warnings.push(
            "Inline pictures render from their original bytes and can be deleted or newly inserted; crop, artistic effects, and non-inline text wrapping are not reproduced.".to_string(),
        );
    }
    if has_anchored {
        warnings.push(
            "Floating pictures follow their text wrapping approximately (floated left or right, or on their own line); exact positions, vertical offsets and in-front/behind layering are not reproduced.".to_string(),
        );
    }
    if !unsupported_kinds.is_empty() {
        warnings.push(format!(
            "{} content is rendered as a labeled placeholder and is not editable.",
            unsupported_kinds.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    if has_bookmarks {
        warnings.push(
            "Bookmark names are read-only; editing a paragraph that contains bookmarks preserves them but moves their start/end to the paragraph boundaries.".to_string(),
        );
    }
    warnings
}

pub fn paragraph_bookmark_names(node: &nitro_xml_opc::xml::Element) -> Vec<String> {
    children_w(node, "bookmarkStart")
        .iter()
        .filter_map(|el| get_w(Some(el), "name"))
        .collect()
}
