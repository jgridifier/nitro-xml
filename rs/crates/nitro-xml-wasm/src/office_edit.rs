// SPDX-License-Identifier: MIT
//! Shared edit-effect and ribbon HTML for the pptx and docx WASM sessions.

/// Result of a chrome / headless command on a non-xlsx editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficeEffect {
    /// Host should download the current package (`shellAction` `save` flag).
    Save,
    /// Model changed (or an undo/redo restored it). The viewer should reload from saved bytes.
    Applied,
}

pub struct RibbonItem {
    pub id: &'static str,
    pub label: &'static str,
    pub enabled: bool,
}

/// Compact command strip. Buttons use `data-a="cmd"` so the loader and embedded
/// toolbar dispatch them through `shellAction("cmd", id)` with no eager-JS change.
pub fn command_ribbon(aria: &str, items: &[RibbonItem]) -> String {
    let mut buttons = String::new();
    for item in items {
        let disabled = if item.enabled { "" } else { " disabled" };
        buttons.push_str(&format!(
            "<button type=\"button\" data-a=\"cmd\" data-g=\"{id}\" aria-label=\"{label}\"{disabled}>{label}</button>",
            id = esc(item.id),
            label = esc(item.label),
            disabled = disabled,
        ));
    }
    format!(
        r#"<div class="xve-ribbon oxe" role="toolbar" aria-label="{aria}"><style>.oxe{{display:flex;flex-wrap:wrap;gap:.25rem;align-items:center;padding:.2rem .35rem;border-bottom:1px solid #ddd;font:12px/1.25 system-ui,sans-serif}}.oxe button{{font:inherit;padding:.12rem .4rem}}.oxe button:disabled{{opacity:.45}}</style>{buttons}</div>"#,
        aria = esc(aria),
        buttons = buttons,
    )
}

pub(crate) fn is_text_key(key: &str) -> bool {
    matches!(key, "Backspace" | "Delete") || is_printable(key)
}

fn is_printable(key: &str) -> bool {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => !c.is_control(),
        _ => false,
    }
}

/// Insert or delete one character. `offset` is a byte index. `None` means the key is a no-op.
pub(crate) fn edit_plain(text: &str, offset: usize, key: &str) -> Option<String> {
    let mut chars: Vec<char> = text.chars().collect();
    let mut byte_off = offset.min(text.len());
    while byte_off > 0 && !text.is_char_boundary(byte_off) {
        byte_off -= 1;
    }
    let char_idx = text[..byte_off].chars().count();
    match key {
        "Backspace" => {
            if char_idx == 0 {
                return None;
            }
            chars.remove(char_idx - 1);
        }
        "Delete" => {
            if char_idx >= chars.len() {
                return None;
            }
            chars.remove(char_idx);
        }
        _ if is_printable(key) => {
            chars.insert(char_idx, key.chars().next()?);
        }
        _ => return None,
    }
    Some(chars.into_iter().collect())
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::handle::Handle;

    const PPTX: &[u8] = include_bytes!("../../../../fixtures/generated/pptx/slides_10.pptx");
    const DOCX: &[u8] = include_bytes!("../../../../fixtures/generated/docx/pages_10.docx");
    const MARK: &str = "Nitro edit roundtrip";

    fn open_editable(bytes: &[u8], name: &str) -> Handle {
        let mut h = Handle::new(bytes.to_vec());
        h.open_document(name).expect("open");
        h.load_editor_chrome();
        h.set_read_only(false);
        h
    }

    #[test]
    fn catches_pptx_edit_missing_from_saved_bytes_after_reopen() {
        // Catches: pptx applyEdit/save still rejected, or a save that drops the new shape text.
        let mut h = open_editable(PPTX, "slides.pptx");
        h.apply_edit(&format!(
            r#"{{"command":"setShapeText","slideIndex":0,"elementIndex":0,"text":"{MARK}"}}"#
        ))
        .expect("edit");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, PPTX);
        let mut again = Handle::new(saved);
        again.open_document("slides.pptx").expect("reopen");
        again.shell_action("find-query", MARK).expect("find");
        assert!(
            !again.shell.find.matches.is_empty(),
            "reopened deck does not show the edited text"
        );
    }

    #[test]
    fn catches_pptx_undo_save_keeping_the_edit() {
        // Catches: undo restoring the model but save still emitting the edited package.
        let mut h = open_editable(PPTX, "slides.pptx");
        h.apply_edit(&format!(
            r#"{{"command":"setShapeText","slideIndex":0,"elementIndex":0,"text":"{MARK}"}}"#
        ))
        .expect("edit");
        h.apply_edit(r#"{"command":"edit.undo"}"#).expect("undo");
        let saved = h.save_document().expect("save");
        assert_eq!(saved, PPTX);
        h.apply_edit(r#"{"command":"edit.redo"}"#).expect("redo");
        let redone = h.save_document().expect("redo save");
        assert_ne!(redone, PPTX);
    }

    #[test]
    fn catches_pptx_shell_cmd_not_routed_to_the_editor() {
        // Catches: shellAction("cmd") still hitting the xlsx session (no_document) for a deck.
        let mut h = open_editable(PPTX, "slides.pptx");
        let ui = h.shell_action("ui", "").expect("ui");
        assert!(
            ui.contains("home.bold"),
            "editable pptx chrome is missing the bold command"
        );
        let before = h.save_document().expect("before");
        h.shell_action("cmd", "home.bold").expect("bold");
        let after = h.save_document().expect("after");
        assert_ne!(before, after);
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), PPTX);
    }

    #[test]
    fn catches_docx_bold_missing_after_save_and_reopen() {
        // Catches: docx save/applyEdit rejected, or bold not written into word/document.xml.
        let mut h = open_editable(DOCX, "pages.docx");
        let block = h
            .docx
            .as_ref()
            .and_then(|d| d.edit.as_ref())
            .expect("session")
            .session
            .host
            .blocks
            .iter()
            .position(|b| b.get("type").and_then(|t| t.as_str()) == Some("paragraph"))
            .expect("paragraph");
        h.apply_edit(&format!(
            r#"{{"command":"setRunFormatting","blockIndex":{block},"runIndex":0,"bold":true}}"#
        ))
        .expect("bold");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, DOCX);
        let again = open_editable(&saved, "pages.docx");
        let run = &again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks[block]["runs"][0];
        assert_eq!(run["bold"], true, "reopened document lost bold: {run}");
        h.apply_edit(r#"{"command":"edit.undo"}"#).expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);
    }

    #[test]
    fn catches_pptx_underline_align_and_typing_missing_after_reopen() {
        // Catches: underline, center, and a typed character never reaching the saved slide XML.
        let mut h = open_editable(PPTX, "slides.pptx");
        h.shell_action("cmd", "home.underline").expect("underline");
        h.shell_action("cmd", "home.align-center").expect("align");
        let typed = h
            .editor_key("Ω", false, false, false, false, false)
            .expect("type");
        assert!(typed.contains("\"handled\":true"), "{typed}");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen");
        let el = again
            .session
            .host
            .slides
            .iter()
            .flat_map(|s| s.elements.iter())
            .find(|e| e.text_segments.iter().any(|s| s.text.contains('Ω')))
            .expect("typed character missing after reopen");
        assert!(
            el.text_segments.iter().any(|s| s.underline),
            "underline missing after reopen"
        );
        assert!(
            el.text_segments
                .iter()
                .any(|s| s.align.as_deref() == Some("center")),
            "center align missing after reopen"
        );
        h.apply_edit(r#"{"command":"edit.undo"}"#)
            .expect("undo type");
        h.apply_edit(r#"{"command":"edit.undo"}"#)
            .expect("undo align");
        h.apply_edit(r#"{"command":"edit.undo"}"#)
            .expect("undo underline");
        assert_eq!(h.save_document().expect("undone"), PPTX);
    }

    #[test]
    fn catches_pptx_justify_bullets_and_numbering_missing_after_reopen() {
        // Catches: justify, bullets, or numbering applied from the ribbon but absent after save.
        let mut h = open_editable(PPTX, "slides.pptx");
        h.shell_action("cmd", "home.align-justify")
            .expect("justify");
        h.shell_action("cmd", "home.bullets").expect("bullets");
        let saved = h.save_document().expect("save bullets");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen");
        let el = &again.session.host.slides[0].elements[0];
        let info = el
            .text_segments
            .iter()
            .find_map(|s| s.bullet_info.as_ref())
            .expect("bullet missing after reopen");
        assert_eq!(info.char.as_deref(), Some("•"));
        assert!(
            el.text_segments
                .iter()
                .any(|s| s.align.as_deref() == Some("justify")),
            "justify missing after reopen"
        );
        h.apply_edit(r#"{"command":"edit.undo"}"#)
            .expect("undo bullets");
        h.apply_edit(r#"{"command":"edit.undo"}"#)
            .expect("undo justify");
        assert_eq!(h.save_document().expect("undone"), PPTX);

        h.shell_action("cmd", "home.numbering").expect("numbering");
        let saved = h.save_document().expect("save numbering");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen numbering");
        let info = again.session.host.slides[0].elements[0]
            .text_segments
            .iter()
            .find_map(|s| s.bullet_info.as_ref())
            .expect("numbering missing after reopen");
        assert_eq!(info.auto_num_type.as_deref(), Some("arabicPeriod"));
        h.apply_edit(r#"{"command":"edit.undo"}"#)
            .expect("undo numbering");
        assert_eq!(h.save_document().expect("undone numbering"), PPTX);
    }

    #[test]
    fn catches_docx_justify_missing_after_reopen() {
        // Catches: Word Justify never writing w:jc w:val="both", so reopen loses the alignment.
        let mut h = open_editable(DOCX, "pages.docx");
        h.shell_action("cmd", "home.align-justify")
            .expect("justify");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, DOCX);
        let again = open_editable(&saved, "pages.docx");
        let block = again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .iter()
            .find(|b| b.get("align").and_then(|a| a.as_str()) == Some("justify"))
            .expect("justify missing after reopen");
        assert_eq!(block["align"], "justify");
        h.apply_edit(r#"{"command":"edit.undo"}"#).expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);
    }

    #[test]
    fn catches_docx_numbering_missing_after_reopen() {
        // Catches: Word numbering leaving the saved package without w:numPr, or undo keeping the list.
        let mut h = open_editable(DOCX, "pages.docx");
        h.shell_action("cmd", "home.numbering").expect("numbering");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, DOCX);
        let again = open_editable(&saved, "pages.docx");
        let numbered = again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .iter()
            .any(|b| b.get("numbering").is_some());
        assert!(numbered, "numbering missing after reopen");
        h.apply_edit(r#"{"command":"edit.undo"}"#).expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);
    }

    #[test]
    fn catches_docx_align_and_typing_missing_after_reopen() {
        // Catches: paragraph align and a typed character never reaching word/document.xml.
        let mut h = open_editable(DOCX, "pages.docx");
        h.shell_action("cmd", "home.align-center").expect("align");
        h.shell_action("cmd", "home.underline").expect("underline");
        h.editor_key("Ω", false, false, false, false, false)
            .expect("type");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, DOCX);
        let again = open_editable(&saved, "pages.docx");
        let block = again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .iter()
            .find(|b| {
                b.get("runs").and_then(|r| r.as_array()).is_some_and(|rs| {
                    rs.iter().any(|r| {
                        r.get("text")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .contains('Ω')
                    })
                })
            })
            .expect("typed character missing after reopen");
        assert_eq!(block["align"], "center");
        assert_eq!(block["runs"][0]["underline"], true);
        h.apply_edit(r#"{"command":"edit.undo"}"#)
            .expect("undo type");
        h.apply_edit(r#"{"command":"edit.undo"}"#)
            .expect("undo underline");
        h.apply_edit(r#"{"command":"edit.undo"}"#)
            .expect("undo align");
        assert_eq!(h.save_document().expect("undone"), DOCX);
    }

    #[test]
    fn catches_strike_and_highlight_missing_after_reopen_or_undo() {
        // Catches: strikethrough or highlight never reaching the saved package, or undo failing to restore it.
        let mut pptx = open_editable(PPTX, "slides.pptx");
        pptx.shell_action("cmd", "home.strikethrough")
            .expect("pptx strike");
        let saved = pptx.save_document().expect("pptx save");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("pptx reopen");
        assert!(
            again
                .session
                .host
                .slides
                .iter()
                .flat_map(|s| s.elements.iter())
                .any(|e| e.text_segments.iter().any(|seg| seg.strike)),
            "pptx strikethrough missing after reopen"
        );
        pptx.shell_action("cmd", "edit.undo").expect("pptx undo");
        assert_eq!(pptx.save_document().expect("pptx undone"), PPTX);

        let mut docx = open_editable(DOCX, "pages.docx");
        docx.shell_action("cmd", "home.strikethrough")
            .expect("docx strike");
        docx.shell_action("cmd", "home.highlight")
            .expect("docx highlight");
        let saved = docx.save_document().expect("docx save");
        assert_ne!(saved, DOCX);
        let again = open_editable(&saved, "pages.docx");
        let run = again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .iter()
            .find_map(|b| b.get("runs")?.as_array()?.first())
            .expect("run");
        assert_eq!(run["strike"], true, "docx strike missing: {run}");
        assert_eq!(run["highlight"], "yellow", "docx highlight missing: {run}");
        docx.shell_action("cmd", "edit.undo")
            .expect("undo highlight");
        docx.shell_action("cmd", "edit.undo").expect("undo strike");
        assert_eq!(docx.save_document().expect("docx undone"), DOCX);
    }

    #[test]
    fn catches_docx_shell_bold_leaving_the_package_unchanged() {
        // Catches: home.bold on a document staying on the xlsx command table, so w:b never flips.
        let mut h = open_editable(DOCX, "pages.docx");
        let before = h.save_document().expect("before");
        h.shell_action("cmd", "home.bold").expect("bold");
        assert_ne!(h.save_document().expect("after"), before);
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);
    }

    #[test]
    fn catches_readonly_office_save_rewriting_or_edit_slipping_through() {
        // Catches: a read-only pptx/docx save rewriting the package, or a shell cmd applying an edit.
        // applyEdit's Err(JsValue) cannot be observed in native tests (wasm_bindgen panics off-wasm).
        for (bytes, name) in [(PPTX, "slides.pptx"), (DOCX, "pages.docx")] {
            let mut h = Handle::new(bytes.to_vec());
            h.open_document(name).expect("open");
            h.load_editor_chrome();
            h.set_read_only(true);
            assert!(h.editor.read_only());
            assert_eq!(h.save_document().expect("save"), bytes);
            h.shell_action("cmd", "home.bold").expect("cmd");
            assert_eq!(h.save_document().expect("after"), bytes);
            let started = h.pptx.as_ref().and_then(|p| p.edit.as_ref()).is_some()
                || h.docx.as_ref().and_then(|d| d.edit.as_ref()).is_some();
            assert!(!started, "read-only open started an edit session");
        }
    }

    #[test]
    fn catches_pptx_slide_and_text_box_commands_missing_after_reopen() {
        // Catches: new, duplicate and delete slide, and text box, not changing the saved package.
        let mut h = open_editable(PPTX, "slides.pptx");
        let before = h.pptx.as_ref().unwrap().pres.slide_count();
        h.shell_action("cmd", "insert.duplicate-slide")
            .expect("dup");
        let saved = h.save_document().expect("save dup");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen dup");
        assert_eq!(again.session.host.slides.len(), before + 1);
        let text = |slide: &nitro_xml_pptx::edit::host::EditableSlide| {
            slide
                .elements
                .iter()
                .filter_map(|el| el.text.clone())
                .collect::<String>()
        };
        assert_eq!(
            text(&again.session.host.slides[1]),
            text(&again.session.host.slides[0])
        );
        h.shell_action("cmd", "edit.undo").expect("undo dup");
        assert_eq!(h.save_document().expect("undone dup"), PPTX);

        h.shell_action("cmd", "insert.slide").expect("slide");
        let saved = h.save_document().expect("save slide");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen");
        assert_eq!(again.session.host.slides.len(), before + 1);
        h.shell_action("cmd", "edit.undo").expect("undo slide");
        assert_eq!(h.save_document().expect("undone slide"), PPTX);

        h.shell_action("cmd", "home.delete-slide").expect("delete");
        let saved = h.save_document().expect("save delete");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen delete");
        assert_eq!(again.session.host.slides.len(), before - 1);
        h.shell_action("cmd", "edit.undo").expect("undo delete");
        assert_eq!(h.save_document().expect("undone delete"), PPTX);

        let shapes: usize = h
            .pptx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .slides
            .iter()
            .map(|s| s.elements.len())
            .sum();
        h.shell_action("cmd", "insert.text-box").expect("text box");
        let saved = h.save_document().expect("save text box");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen text box");
        let after: usize = again
            .session
            .host
            .slides
            .iter()
            .map(|s| s.elements.len())
            .sum();
        assert!(after > shapes, "text box missing after reopen");
        h.shell_action("cmd", "edit.undo").expect("undo text box");
        assert_eq!(h.save_document().expect("undone text box"), PPTX);
    }

    #[test]
    fn catches_pptx_reorder_and_table_edits_missing_after_reopen() {
        // Catches: move-slide and table row/column edits that vanish on save, or undo that keeps them.
        let mut h = open_editable(PPTX, "slides.pptx");
        let first = h
            .pptx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .slides[0]
            .path
            .clone();
        let second = h
            .pptx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .slides[1]
            .path
            .clone();
        h.shell_action("cmd", "insert.move-slide-up")
            .expect("noop up");
        assert_eq!(h.save_document().expect("noop"), PPTX);
        h.shell_action("cmd", "insert.move-slide-down")
            .expect("down");
        let saved = h.save_document().expect("save move");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen move");
        assert_eq!(again.session.host.slides[0].path, second);
        assert_eq!(again.session.host.slides[1].path, first);
        h.shell_action("cmd", "edit.undo").expect("undo move");
        assert_eq!(h.save_document().expect("undone move"), PPTX);

        h.shell_action("cmd", "insert.table").expect("table");
        h.shell_action("cmd", "table.insert-row").expect("row");
        h.shell_action("cmd", "table.insert-column").expect("col");
        let saved = h.save_document().expect("save table");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen table");
        let table = again
            .session
            .host
            .slides
            .iter()
            .flat_map(|s| s.elements.iter())
            .find(|el| el.type_ == "table")
            .expect("table missing after reopen");
        let model = table.table.as_ref().expect("model");
        assert_eq!(model.rows.len(), 3);
        assert_eq!(model.column_widths.len(), 3);
        h.shell_action("cmd", "table.delete-row")
            .expect("delete row");
        h.shell_action("cmd", "table.delete-column")
            .expect("delete col");
        let saved = h.save_document().expect("save trim");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen trim");
        let table = again
            .session
            .host
            .slides
            .iter()
            .flat_map(|s| s.elements.iter())
            .find(|el| el.type_ == "table")
            .expect("trimmed table missing");
        let model = table.table.as_ref().expect("trimmed model");
        assert_eq!(model.rows.len(), 2);
        assert_eq!(model.column_widths.len(), 2);
        for _ in 0..5 {
            h.shell_action("cmd", "edit.undo").expect("undo table");
        }
        assert_eq!(h.save_document().expect("undone table"), PPTX);
    }

    #[test]
    fn catches_docx_insert_table_missing_after_reopen() {
        // Catches: insert.table not writing a w:tbl, so a reopened document has no new table.
        let mut h = open_editable(DOCX, "pages.docx");
        let before = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
            .count();
        h.shell_action("cmd", "insert.table").expect("table");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, DOCX);
        let again = open_editable(&saved, "pages.docx");
        let after = again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
            .count();
        assert_eq!(after, before + 1, "reopen dropped the table");
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);
    }

    #[test]
    fn catches_pptx_picture_missing_after_reopen_or_undo() {
        // Catches: insert.picture leaving the saved deck without a picture, or undo keeping the PNG.
        let mut h = open_editable(PPTX, "slides.pptx");
        let before = h
            .pptx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .slides
            .iter()
            .flat_map(|s| s.elements.iter())
            .filter(|el| el.type_ == "picture")
            .count();
        h.shell_action("cmd", "insert.picture").expect("picture");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen");
        let after = again
            .session
            .host
            .slides
            .iter()
            .flat_map(|s| s.elements.iter())
            .filter(|el| el.type_ == "picture")
            .count();
        assert_eq!(after, before + 1, "reopen dropped the picture");
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), PPTX);
    }

    #[test]
    fn catches_docx_picture_missing_after_reopen_or_undo() {
        // Catches: insert.picture leaving the saved document without an inline image, or undo keeping it.
        let mut h = open_editable(DOCX, "pages.docx");
        h.shell_action("cmd", "insert.picture").expect("picture");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, DOCX);
        let again = open_editable(&saved, "pages.docx");
        let image = again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .iter()
            .filter_map(|b| b.get("runs")?.as_array())
            .flatten()
            .find_map(|run| run.get("image").cloned())
            .expect("image missing after reopen");
        let part = image.get("partName").and_then(|v| v.as_str()).unwrap_or("");
        assert!(part.contains("media"), "partName {part}");
        assert_eq!(
            image.get("contentType").and_then(|v| v.as_str()),
            Some("image/png")
        );
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);
    }

    #[test]
    fn catches_pptx_font_size_and_color_missing_after_reopen() {
        // Catches: home.font-size and home.font-color leaving the saved slide run unchanged.
        let mut h = open_editable(PPTX, "slides.pptx");
        h.shell_action("cmd", "home.font-size").expect("size");
        h.shell_action("cmd", "home.font-color").expect("color");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen");
        let style = again
            .session
            .host
            .slides
            .iter()
            .flat_map(|s| s.elements.iter())
            .find_map(|el| el.text_style.as_ref())
            .expect("styled shape");
        assert_eq!(style.font_size, Some(32.0));
        let color = style.color.as_deref().unwrap_or("");
        assert!(
            color.to_ascii_uppercase().contains("C00000"),
            "color {color}"
        );
        h.shell_action("cmd", "edit.undo").expect("undo color");
        h.shell_action("cmd", "edit.undo").expect("undo size");
        assert_eq!(h.save_document().expect("undone"), PPTX);
    }

    #[test]
    fn catches_docx_font_and_table_structure_missing_after_reopen() {
        // Catches: font size, font color, or a table row/column edit missing from the saved docx.
        let mut h = open_editable(DOCX, "pages.docx");
        h.shell_action("cmd", "home.font-size").expect("size");
        h.shell_action("cmd", "home.font-color").expect("color");
        let saved = h.save_document().expect("save font");
        let again = open_editable(&saved, "pages.docx");
        let run = &again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks[0]["runs"][0];
        assert_eq!(run["fontSize"].as_f64(), Some(18.0));
        let color = run["color"].as_str().unwrap_or("");
        assert!(
            color.eq_ignore_ascii_case("C00000") || color.eq_ignore_ascii_case("#C00000"),
            "color {color}"
        );
        h.shell_action("cmd", "edit.undo").expect("undo color");
        h.shell_action("cmd", "edit.undo").expect("undo size");
        assert_eq!(h.save_document().expect("font undone"), DOCX);

        let rows_of = |h: &Handle| {
            let host = &h.docx.as_ref().unwrap().edit.as_ref().unwrap().session.host;
            let table = host
                .blocks
                .iter()
                .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
                .expect("table");
            let rows = table["rows"].as_array().expect("rows");
            let cols = rows
                .first()
                .and_then(|r| r.as_array())
                .map(|c| c.len())
                .unwrap_or(0);
            (rows.len(), cols)
        };
        let (rows, cols) = rows_of(&h);
        h.shell_action("cmd", "table.insert-row").expect("row");
        h.shell_action("cmd", "table.delete-row")
            .expect("delete row");
        let saved = h.save_document().expect("save delete row");
        let again = open_editable(&saved, "pages.docx");
        assert_eq!(rows_of(&again).0, rows, "deleted row still in the package");
        h.shell_action("cmd", "edit.undo").expect("undo delete row");
        let saved = h.save_document().expect("save restored row");
        let again = open_editable(&saved, "pages.docx");
        assert_eq!(rows_of(&again).0, rows + 1, "undo did not restore the row");
        h.shell_action("cmd", "edit.undo").expect("undo insert row");
        assert_eq!(h.save_document().expect("row undone"), DOCX);

        h.shell_action("cmd", "table.insert-column")
            .expect("column");
        h.shell_action("cmd", "table.delete-column")
            .expect("delete column");
        let saved = h.save_document().expect("save delete column");
        let again = open_editable(&saved, "pages.docx");
        assert_eq!(
            rows_of(&again).1,
            cols,
            "deleted column still in the package"
        );
        h.shell_action("cmd", "edit.undo")
            .expect("undo delete column");
        let saved = h.save_document().expect("save restored column");
        let again = open_editable(&saved, "pages.docx");
        assert_eq!(
            rows_of(&again).1,
            cols + 1,
            "undo did not restore the column"
        );
        h.shell_action("cmd", "edit.undo")
            .expect("undo insert column");
        assert_eq!(h.save_document().expect("column undone"), DOCX);
    }

    #[test]
    fn catches_docx_deleted_table_still_present_after_reopen() {
        // Catches: table.delete leaving the w:tbl in the saved package, or undo failing to put it back.
        let mut h = open_editable(DOCX, "pages.docx");
        let before = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
            .count();
        assert!(before >= 1, "fixture has no table");
        let blocks = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .len();
        h.shell_action("cmd", "table.delete").expect("delete");
        let saved = h.save_document().expect("save");
        assert_ne!(saved, DOCX);
        let again = open_editable(&saved, "pages.docx");
        let after = again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
            .count();
        let after_blocks = again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host
            .blocks
            .len();
        assert_eq!(after, before - 1, "deleted table still in the package");
        assert_eq!(after_blocks, blocks - 1);
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);
    }
}
