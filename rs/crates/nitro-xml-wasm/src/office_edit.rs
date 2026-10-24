// SPDX-License-Identifier: MIT
//! Shared edit-effect and ribbon HTML for the pptx and docx WASM sessions.

/// Result of a chrome / headless command on a non-xlsx editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OfficeEffect {
    /// Host should download the current package (`shellAction` `save` flag).
    Save,
    /// Model changed (or an undo/redo restored it). The viewer should reload from saved bytes.
    Applied,
    /// Copy or cut captured `text`. `changed` is set when cut rewrote the package.
    Clipboard { text: String, changed: bool },
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
    fn catches_docx_cut_copy_paste_leaving_the_paragraph_unchanged() {
        // Catches: cut or paste of paragraph text not rewriting the package, so undo cannot restore it.
        let mut h = open_editable(DOCX, "pages.docx");
        let before = h.save_document().expect("before");
        let copied = h.shell_action("cmd", "home.copy").expect("copy");
        assert!(
            copied.contains("\"c\""),
            "copy did not return paragraph text: {copied}"
        );
        assert_eq!(
            h.save_document().expect("copy save"),
            before,
            "copy must not rewrite the package"
        );
        h.shell_action("cmd", "home.cut").expect("cut");
        assert_ne!(h.save_document().expect("cut save"), before);
        h.shell_action("cmd", "edit.undo").expect("undo cut");
        assert_eq!(h.save_document().expect("cut undone"), before);
        h.shell_action("cmd", "home.paste").expect("paste");
        assert_ne!(h.save_document().expect("paste save"), before);
        h.shell_action("cmd", "edit.undo").expect("undo paste");
        assert_eq!(h.save_document().expect("paste undone"), before);
    }

    #[test]
    fn catches_pptx_cut_copy_paste_leaving_the_shape_unchanged() {
        // Catches: cut or paste of shape text not rewriting the slide, so undo cannot restore it.
        let mut h = open_editable(PPTX, "slides.pptx");
        h.editor_key("Q", false, false, false, false, false)
            .expect("seed");
        let seeded = h.save_document().expect("seeded");
        let copied = h.shell_action("cmd", "home.copy").expect("copy");
        assert!(
            copied.contains("\"c\""),
            "copy did not return shape text: {copied}"
        );
        assert_eq!(h.save_document().expect("copy save"), seeded);
        h.shell_action("cmd", "home.cut").expect("cut");
        assert_ne!(h.save_document().expect("cut save"), seeded);
        h.shell_action("cmd", "edit.undo").expect("undo cut");
        assert_eq!(h.save_document().expect("cut undone"), seeded);
        h.shell_action("cmd", "home.paste").expect("paste");
        assert_ne!(h.save_document().expect("paste save"), seeded);
        h.shell_action("cmd", "edit.undo").expect("undo paste");
        assert_eq!(h.save_document().expect("paste undone"), seeded);
    }

    #[test]
    fn catches_pptx_paste_of_a_shape_only_appending_text() {
        // Catches: Paste of a copied shape appending its text instead of inserting the shape,
        // or one undo rolling the picture paste and the shape paste back together.
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
            .slides[0]
            .elements
            .len();
        h.shell_action("cmd", "home.copy").expect("copy");
        assert_eq!(h.save_document().expect("copy"), PPTX);
        h.shell_action("cmd", "home.paste").expect("paste shape");
        let pasted = h.save_document().expect("save paste");
        assert_ne!(pasted, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&pasted).expect("reopen");
        assert_eq!(again.session.host.slides[0].elements.len(), before + 1);
        h.shell_action("cmd", "edit.undo").expect("undo paste");
        assert_eq!(h.save_document().expect("paste undone"), PPTX);

        h.shell_action("cmd", "insert.picture").expect("picture");
        let with_picture = h.save_document().expect("save picture");
        h.shell_action("cmd", "home.copy").expect("copy picture");
        assert_eq!(h.save_document().expect("copy picture"), with_picture);
        h.shell_action("cmd", "home.paste").expect("paste picture");
        let pasted = h.save_document().expect("save picture paste");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&pasted).expect("reopen pics");
        let pictures = again.session.host.slides[0]
            .elements
            .iter()
            .filter(|el| el.type_ == "picture")
            .count();
        assert_eq!(pictures, 2, "paste did not insert a second picture");
        h.shell_action("cmd", "edit.undo")
            .expect("undo picture paste");
        assert_eq!(
            h.save_document().expect("picture paste undone"),
            with_picture
        );
        h.shell_action("cmd", "edit.undo").expect("undo picture");
        assert_eq!(h.save_document().expect("picture undone"), PPTX);
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
    fn catches_small_caps_leaving_cap_off_the_saved_slide() {
        // Catches: Small caps leaving a:rPr/@cap off the saved run, or undoing as more than one step.
        let mut pptx = open_editable(PPTX, "slides.pptx");
        let strip = pptx.pptx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.small-caps\""));
        pptx.shell_action("cmd", "home.small-caps")
            .expect("small caps");
        let saved = pptx.save_document().expect("save");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen");
        assert!(
            again
                .session
                .host
                .slides
                .iter()
                .flat_map(|s| s.elements.iter())
                .any(|e| e.text_segments.iter().any(|seg| seg.small_caps)),
            "small caps missing after reopen"
        );
        assert!(again
            .ribbon_html()
            .contains("data-command=\"home.small-caps\""));
        pptx.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(pptx.save_document().expect("undone"), PPTX);
    }

    #[test]
    fn catches_all_caps_leaving_cap_off_the_saved_slide() {
        // Catches: All caps leaving a:rPr/@cap="all" off the saved run, or undoing as more than one step.
        let mut pptx = open_editable(PPTX, "slides.pptx");
        let strip = pptx.pptx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.caps\""));
        pptx.shell_action("cmd", "home.caps").expect("all caps");
        let saved = pptx.save_document().expect("save");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen");
        assert!(
            again
                .session
                .host
                .slides
                .iter()
                .flat_map(|s| s.elements.iter())
                .any(|e| e.text_segments.iter().any(|seg| seg.all_caps)),
            "all caps missing after reopen"
        );
        assert!(again.ribbon_html().contains("data-command=\"home.caps\""));
        pptx.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(pptx.save_document().expect("undone"), PPTX);
        let docx = open_editable(DOCX, "pages.docx");
        assert!(docx
            .docx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("data-g=\"home.caps\""));
    }

    #[test]
    fn catches_double_strike_leaving_dbl_strike_off_the_saved_slide() {
        // Catches: Double strikethrough leaving a:rPr/@strike="dblStrike" off the saved run, or undoing as more than one step.
        let mut pptx = open_editable(PPTX, "slides.pptx");
        let strip = pptx.pptx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.double-strike\""));
        pptx.shell_action("cmd", "home.double-strike")
            .expect("double strike");
        let saved = pptx.save_document().expect("save");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen");
        assert!(
            again
                .session
                .host
                .slides
                .iter()
                .flat_map(|s| s.elements.iter())
                .any(|e| e.text_segments.iter().any(|seg| seg.double_strike)),
            "double strikethrough missing after reopen"
        );
        assert!(again
            .ribbon_html()
            .contains("data-command=\"home.double-strike\""));
        pptx.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(pptx.save_document().expect("undone"), PPTX);
        let docx = open_editable(DOCX, "pages.docx");
        assert!(docx
            .docx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("data-g=\"home.double-strike\""));
    }

    #[test]
    fn catches_indent_spacing_script_font_and_word_bullets_missing_after_reopen_or_undo() {
        // Catches: indent, line spacing, subscript/superscript, or font name never reaching the saved package, or undo failing to restore it. Word bullets too.
        let mut pptx = open_editable(PPTX, "slides.pptx");
        pptx.shell_action("cmd", "home.indent")
            .expect("pptx indent");
        pptx.shell_action("cmd", "home.line-spacing")
            .expect("pptx spacing");
        pptx.shell_action("cmd", "home.superscript")
            .expect("pptx superscript");
        pptx.shell_action("cmd", "home.font-name")
            .expect("pptx font");
        let saved = pptx.save_document().expect("pptx save");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("pptx reopen");
        let seg = again
            .session
            .host
            .slides
            .iter()
            .flat_map(|s| s.elements.iter())
            .flat_map(|e| e.text_segments.iter())
            .find(|s| s.typeface.as_deref() == Some("Georgia"))
            .expect("pptx font missing after reopen");
        assert_eq!(seg.line_spacing_pct, Some(150_000), "pptx line spacing");
        assert_eq!(seg.baseline, Some(30_000), "pptx superscript");
        assert_eq!(seg.typeface.as_deref(), Some("Georgia"), "pptx font");
        for _ in 0..4 {
            pptx.shell_action("cmd", "edit.undo").expect("pptx undo");
        }
        assert_eq!(pptx.save_document().expect("pptx undone"), PPTX);

        pptx.shell_action("cmd", "home.subscript")
            .expect("pptx subscript");
        let saved = pptx.save_document().expect("pptx sub save");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("pptx sub reopen");
        assert!(
            again
                .session
                .host
                .slides
                .iter()
                .flat_map(|s| s.elements.iter())
                .flat_map(|e| e.text_segments.iter())
                .any(|s| s.baseline == Some(-25_000)),
            "pptx subscript missing after reopen"
        );
        pptx.shell_action("cmd", "edit.undo")
            .expect("pptx sub undo");
        assert_eq!(pptx.save_document().expect("pptx sub undone"), PPTX);

        let mut docx = open_editable(DOCX, "pages.docx");
        docx.shell_action("cmd", "home.indent")
            .expect("docx indent");
        docx.shell_action("cmd", "home.line-spacing")
            .expect("docx spacing");
        docx.shell_action("cmd", "home.superscript")
            .expect("docx superscript");
        docx.shell_action("cmd", "home.font-name")
            .expect("docx font");
        docx.shell_action("cmd", "home.bullets")
            .expect("docx bullets");
        let saved = docx.save_document().expect("docx save");
        assert_ne!(saved, DOCX);
        let again = open_editable(&saved, "pages.docx");
        let host = &again
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .session
            .host;
        let block = host
            .blocks
            .iter()
            .find(|b| b.get("indentLeftTwips").and_then(|n| n.as_i64()) == Some(720))
            .expect("docx indent missing after reopen");
        assert_eq!(block["lineSpacingTwips"], 360, "docx line spacing: {block}");
        assert_eq!(block["lineSpacingRule"], "auto");
        assert_eq!(block["runs"][0]["verticalAlign"], "superscript");
        assert_eq!(block["runs"][0]["fontFamily"], "Georgia");
        let num_id = block["numbering"]["numId"]
            .as_i64()
            .or_else(|| block["numbering"]["numId"].as_u64().map(|n| n as i64))
            .expect("docx bullet numId");
        let fmt = host
            .numbering_catalog
            .as_ref()
            .and_then(|c| c.get("nums"))
            .and_then(|n| n.get(num_id.to_string()))
            .and_then(|n| n.get("abstractNumId"))
            .and_then(|v| v.as_str())
            .and_then(|abs| {
                host.numbering_catalog
                    .as_ref()
                    .unwrap()
                    .get("abstractNums")
                    .and_then(|a| a.get(abs))
                    .and_then(|a| a.get("levels"))
                    .and_then(|l| l.get("0"))
                    .and_then(|l| l.get("numFmt"))
                    .and_then(|v| v.as_str())
            });
        assert_eq!(fmt, Some("bullet"), "docx bullet definition missing");
        for _ in 0..5 {
            docx.shell_action("cmd", "edit.undo").expect("docx undo");
        }
        assert_eq!(docx.save_document().expect("docx undone"), DOCX);

        docx.shell_action("cmd", "home.subscript")
            .expect("docx subscript");
        let saved = docx.save_document().expect("docx sub save");
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
        assert_eq!(
            run["verticalAlign"], "subscript",
            "docx subscript missing: {run}"
        );
        docx.shell_action("cmd", "edit.undo")
            .expect("docx sub undo");
        assert_eq!(docx.save_document().expect("docx sub undone"), DOCX);

        docx.shell_action("cmd", "home.outdent")
            .expect("flush outdent");
        assert_eq!(
            docx.save_document().expect("outdent no-op"),
            DOCX,
            "outdent on a flush paragraph changed the package"
        );
    }

    #[test]
    fn catches_space_before_and_after_leaving_paragraph_spacing_unchanged() {
        // Catches: Space before or Space after leaving w:spacing unchanged, writing a gap
        // other than 12pt, or undo keeping the direct spacing.
        let gaps = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    let n = |k: &str| b.get(k).and_then(|v| v.as_i64());
                    (n("spacingBeforeTwips"), n("spacingAfterTwips"))
                })
                .collect::<Vec<_>>()
        };
        let want = nitro_xml_docx::edit::format::PARAGRAPH_SPACE_TWIPS;
        let mut h = open_editable(DOCX, "pages.docx");
        let original = gaps(DOCX);
        h.shell_action("cmd", "home.space-before")
            .expect("space before");
        let saved = h.save_document().expect("save before");
        assert_ne!(saved, DOCX);
        let after = gaps(&saved);
        let changed = original
            .iter()
            .zip(after.iter())
            .filter(|(a, b)| a.0 != b.0)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "space before touched {changed:?}");
        let (old, new) = changed[0];
        assert_eq!(old.1, new.1, "space before rewrote space after");
        if old.0 == Some(want) {
            assert_eq!(new.0, None, "12pt space before was not cleared");
        } else {
            assert_eq!(new.0, Some(want), "space before was not 12pt");
        }
        h.shell_action("cmd", "edit.undo").expect("undo before");
        assert_eq!(h.save_document().expect("undone before"), DOCX);

        h.shell_action("cmd", "home.space-after")
            .expect("space after");
        let saved = h.save_document().expect("save after");
        assert_ne!(saved, DOCX);
        let after = gaps(&saved);
        let changed = original
            .iter()
            .zip(after.iter())
            .filter(|(a, b)| a.1 != b.1)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "space after touched {changed:?}");
        let (old, new) = changed[0];
        assert_eq!(old.0, new.0, "space after rewrote space before");
        if old.1 == Some(want) {
            assert_eq!(new.1, None, "12pt space after was not cleared");
        } else {
            assert_eq!(new.1, Some(want), "space after was not 12pt");
        }
        h.shell_action("cmd", "edit.undo").expect("undo after");
        assert_eq!(h.save_document().expect("undone after"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.space-before\""));
        assert!(strip.contains("data-g=\"home.space-after\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.space-before"));
    }

    #[test]
    fn catches_hanging_indent_leaving_the_paragraph_indent_unchanged() {
        // Catches: Hanging indent leaving w:ind/@w:hanging off the saved paragraph, writing a
        // gap other than half an inch, keeping a first-line indent beside it, or undo keeping it.
        let gaps = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    let n = |k: &str| b.get(k).and_then(|v| v.as_i64());
                    (n("hangingTwips"), n("firstLineTwips"))
                })
                .collect::<Vec<_>>()
        };
        let want = nitro_xml_docx::edit::format::HANGING_INDENT_TWIPS;
        let mut h = open_editable(DOCX, "pages.docx");
        let original = gaps(DOCX);
        h.shell_action("cmd", "home.hanging-indent")
            .expect("hanging");
        let saved = h.save_document().expect("save hanging");
        assert_ne!(saved, DOCX);
        let after = gaps(&saved);
        let changed = original
            .iter()
            .zip(after.iter())
            .filter(|(a, b)| a.0 != b.0)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "hanging indent touched {changed:?}");
        let (old, new) = changed[0];
        if old.0 == Some(want) {
            assert_eq!(new.0, None, "half-inch hanging indent was not cleared");
            assert_eq!(old.1, new.1, "clearing hanging rewrote the first line");
        } else {
            assert_eq!(new.0, Some(want), "hanging indent was not half an inch");
            assert_eq!(new.1, None, "hanging indent left a first-line indent");
        }
        for ((old_h, old_f), (new_h, new_f)) in original.iter().zip(after.iter()) {
            if old_h == new_h {
                assert_eq!(old_f, new_f, "an untouched paragraph lost its first line");
            }
        }
        h.shell_action("cmd", "edit.undo").expect("undo hanging");
        assert_eq!(h.save_document().expect("undone hanging"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.hanging-indent\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"hanging-indent\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.hanging-indent"));
    }

    #[test]
    fn catches_first_line_indent_leaving_the_paragraph_indent_unchanged() {
        // Catches: First line indent leaving w:ind/@w:firstLine off the saved paragraph, writing a
        // gap other than half an inch, keeping a hanging indent beside it, or undo keeping it.
        let gaps = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    let n = |k: &str| b.get(k).and_then(|v| v.as_i64());
                    (n("firstLineTwips"), n("hangingTwips"))
                })
                .collect::<Vec<_>>()
        };
        let want = nitro_xml_docx::edit::format::FIRST_LINE_INDENT_TWIPS;
        let mut h = open_editable(DOCX, "pages.docx");
        let original = gaps(DOCX);
        h.shell_action("cmd", "home.first-line")
            .expect("first line");
        let saved = h.save_document().expect("save first line");
        assert_ne!(saved, DOCX);
        let after = gaps(&saved);
        let changed = original
            .iter()
            .zip(after.iter())
            .filter(|(a, b)| a.0 != b.0)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "first line indent touched {changed:?}");
        let (old, new) = changed[0];
        if old.0 == Some(want) {
            assert_eq!(new.0, None, "half-inch first line indent was not cleared");
            assert_eq!(
                old.1, new.1,
                "clearing first line rewrote the hanging indent"
            );
        } else {
            assert_eq!(new.0, Some(want), "first line indent was not half an inch");
            assert_eq!(new.1, None, "first line indent left a hanging indent");
        }
        for ((old_f, old_h), (new_f, new_h)) in original.iter().zip(after.iter()) {
            if old_f == new_f {
                assert_eq!(
                    old_h, new_h,
                    "an untouched paragraph lost its hanging indent"
                );
            }
        }
        h.shell_action("cmd", "edit.undo").expect("undo first line");
        assert_eq!(h.save_document().expect("undone first line"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.first-line\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"first-line\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.first-line"));
    }

    #[test]
    fn catches_right_indent_leaving_the_left_indent_unchanged() {
        // Catches: Increase Right Indent leaving w:ind/@w:right off the saved paragraph, writing
        // w:right="0" from a flush paragraph, or moving the left indent.
        let gaps = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    let n = |k: &str| b.get(k).and_then(|v| v.as_i64());
                    (n("indentRightTwips"), n("indentLeftTwips"))
                })
                .collect::<Vec<_>>()
        };
        let step = nitro_xml_docx::edit::format::INDENT_STEP_TWIPS;
        let mut h = open_editable(DOCX, "pages.docx");
        let original = gaps(DOCX);
        h.shell_action("cmd", "home.indent-right")
            .expect("indent right");
        let saved = h.save_document().expect("save right");
        assert_ne!(saved, DOCX);
        let after = gaps(&saved);
        let changed = original
            .iter()
            .zip(after.iter())
            .filter(|(a, b)| a.0 != b.0)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "right indent touched {changed:?}");
        let (old, new) = changed[0];
        assert_eq!(old.1, new.1, "right indent moved the left indent");
        assert_eq!(
            new.0,
            Some(old.0.unwrap_or(0) + step),
            "right indent was not half an inch"
        );
        for ((old_r, old_l), (new_r, new_l)) in original.iter().zip(after.iter()) {
            if old_r == new_r {
                assert_eq!(old_l, new_l, "an untouched paragraph lost its left indent");
            }
        }
        h.shell_action("cmd", "edit.undo").expect("undo right");
        assert_eq!(h.save_document().expect("undone right"), DOCX);

        h.shell_action("cmd", "home.outdent-right")
            .expect("outdent right");
        let saved = h.save_document().expect("save outdent right");
        if old.0.unwrap_or(0) == 0 {
            assert_eq!(
                saved, DOCX,
                "outdent on a flush right indent changed the package"
            );
        } else {
            assert_ne!(saved, DOCX);
            let after = gaps(&saved);
            let changed = original
                .iter()
                .zip(after.iter())
                .filter(|(a, b)| a.0 != b.0)
                .collect::<Vec<_>>();
            assert_eq!(changed.len(), 1, "right outdent touched {changed:?}");
            let (before, next) = changed[0];
            assert_eq!(before.1, next.1, "right outdent moved the left indent");
            let want = before.0.unwrap_or(0) - step;
            if want == 0 {
                assert_eq!(next.0, None, "flush right indent stored 0");
            } else {
                assert_eq!(next.0, Some(want));
            }
            h.shell_action("cmd", "edit.undo")
                .expect("undo outdent right");
            assert_eq!(h.save_document().expect("undone outdent right"), DOCX);
        }

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.indent-right\""));
        assert!(strip.contains("data-g=\"home.outdent-right\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"indent-right\""));
        assert!(home.contains("data-command=\"outdent-right\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.indent-right"));
    }

    #[test]
    fn catches_double_underline_on_a_single_underlined_run_writing_val_single() {
        // Catches: Double underline on a run that is already underlined leaving w:u val="single",
        // or the Underline button turning a cleared double underline back into val="double".
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .flat_map(|b| {
                    b.get("runs")
                        .and_then(|r| r.as_array())
                        .into_iter()
                        .flatten()
                })
                .map(|run| {
                    let on = run
                        .get("underline")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let style = run
                        .get("underlineStyle")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    (on, style)
                })
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.underline").expect("underline");
        let once = h.save_document().expect("save underline");
        let once_marks = marks(&once);
        let changed = start
            .iter()
            .zip(once_marks.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "underline touched {changed:?}");
        let idx = changed[0];
        if !once_marks[idx].0 {
            h.shell_action("cmd", "home.underline")
                .expect("underline on");
        }
        let single = h.save_document().expect("save single");
        let single_marks = marks(&single);
        assert!(single_marks[idx].0, "caret run is not underlined");
        assert_ne!(
            single_marks[idx].1.as_deref(),
            Some("double"),
            "single underline wrote val=double"
        );

        h.shell_action("cmd", "home.double-underline")
            .expect("double underline");
        let doubled = h.save_document().expect("save double");
        assert_ne!(doubled, single);
        let double_marks = marks(&doubled);
        assert!(double_marks[idx].0);
        assert_eq!(double_marks[idx].1.as_deref(), Some("double"));
        for (i, (before, after)) in single_marks.iter().zip(double_marks.iter()).enumerate() {
            if i != idx {
                assert_eq!(before, after, "double underline changed run {i}");
            }
        }

        h.shell_action("cmd", "home.underline")
            .expect("clear double");
        let off = h.save_document().expect("save cleared");
        let off_marks = marks(&off);
        assert!(!off_marks[idx].0, "underline left the run underlined");
        assert!(
            off_marks[idx].1.is_none(),
            "underline left a double style on the run"
        );
        h.shell_action("cmd", "home.underline")
            .expect("single again");
        let again = h.save_document().expect("save single again");
        let again_marks = marks(&again);
        assert!(again_marks[idx].0);
        assert_ne!(
            again_marks[idx].1.as_deref(),
            Some("double"),
            "underline rewrote a cleared double underline as double"
        );

        let mut restored = false;
        for _ in 0..8 {
            if h.save_document().expect("save undo") == DOCX {
                restored = true;
                break;
            }
            h.shell_action("cmd", "edit.undo").expect("undo");
        }
        assert!(restored, "undo did not restore the package");

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.double-underline\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"double-underline\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.double-underline"));
    }

    #[test]
    fn catches_all_caps_leaving_w_caps_off_the_saved_run() {
        // Catches: All Caps leaving w:caps off the saved run, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .flat_map(|b| {
                    b.get("runs")
                        .and_then(|r| r.as_array())
                        .into_iter()
                        .flatten()
                })
                .map(|run| run.get("caps").and_then(|v| v.as_bool()))
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.caps").expect("caps");
        let saved = h.save_document().expect("save caps");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "all caps touched {changed:?}");
        let idx = changed[0];
        let was = start[idx].unwrap_or(false);
        assert_eq!(after[idx], Some(!was));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.caps\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"caps\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("data-g=\"home.caps\""));
    }

    #[test]
    fn catches_small_caps_leaving_w_small_caps_off_the_saved_run() {
        // Catches: Small caps leaving w:smallCaps off the saved run, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .flat_map(|b| {
                    b.get("runs")
                        .and_then(|r| r.as_array())
                        .into_iter()
                        .flatten()
                })
                .map(|run| run.get("smallCaps").and_then(|v| v.as_bool()))
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.small-caps")
            .expect("small caps");
        let saved = h.save_document().expect("save small caps");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "small caps touched {changed:?}");
        let idx = changed[0];
        let was = start[idx].unwrap_or(false);
        assert_eq!(after[idx], Some(!was));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.small-caps\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"small-caps\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.small-caps"));
    }

    #[test]
    fn catches_double_strike_leaving_w_dstrike_off_the_saved_run() {
        // Catches: Double strikethrough leaving w:dstrike off the saved run, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .flat_map(|b| {
                    b.get("runs")
                        .and_then(|r| r.as_array())
                        .into_iter()
                        .flatten()
                })
                .map(|run| run.get("doubleStrike").and_then(|v| v.as_bool()))
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.double-strike")
            .expect("double strike");
        let saved = h.save_document().expect("save double strike");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "double strike touched {changed:?}");
        let idx = changed[0];
        let was = start[idx].unwrap_or(false);
        assert_eq!(after[idx], Some(!was));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.double-strike\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"double-strike\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("data-g=\"home.double-strike\""));
    }

    #[test]
    fn catches_hidden_leaving_w_vanish_off_the_saved_run() {
        // Catches: Hidden leaving w:vanish off the saved run, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .flat_map(|b| {
                    b.get("runs")
                        .and_then(|r| r.as_array())
                        .into_iter()
                        .flatten()
                })
                .map(|run| run.get("vanish").and_then(|v| v.as_bool()))
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.hidden").expect("hidden");
        let saved = h.save_document().expect("save hidden");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "hidden touched {changed:?}");
        let idx = changed[0];
        let was = start[idx].unwrap_or(false);
        assert_eq!(after[idx], Some(!was));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.hidden\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"hidden\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.hidden"));
    }

    #[test]
    fn catches_expand_spacing_leaving_w_spacing_off_the_saved_run() {
        // Catches: Expand leaving w:spacing off the saved run, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .flat_map(|b| {
                    b.get("runs")
                        .and_then(|r| r.as_array())
                        .into_iter()
                        .flatten()
                })
                .map(|run| run.get("characterSpacingTwips").and_then(|v| v.as_i64()))
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.expand-spacing")
            .expect("expand");
        let saved = h.save_document().expect("save expand");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "expand touched {changed:?}");
        let idx = changed[0];
        let was = start[idx].unwrap_or(0);
        assert_eq!(after[idx], Some(was + 20));
        h.shell_action("cmd", "home.condense-spacing")
            .expect("condense");
        let condensed = marks(&h.save_document().expect("save condense"));
        assert_eq!(condensed[idx], start[idx], "condense did not step back");
        h.shell_action("cmd", "edit.undo").expect("undo condense");
        assert_eq!(h.save_document().expect("expanded again"), saved);
        h.shell_action("cmd", "edit.undo").expect("undo expand");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.expand-spacing\""));
        assert!(strip.contains("data-g=\"home.condense-spacing\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"expand-spacing\""));
        assert!(home.contains("data-command=\"condense-spacing\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.expand-spacing"));
    }

    #[test]
    fn catches_scale_up_leaving_w_w_off_the_saved_run() {
        // Catches: Scale up leaving w:w off the saved run, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .flat_map(|b| {
                    b.get("runs")
                        .and_then(|r| r.as_array())
                        .into_iter()
                        .flatten()
                })
                .map(|run| run.get("textScalePercent").and_then(|v| v.as_i64()))
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.scale-up").expect("scale up");
        let saved = h.save_document().expect("save scale");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "scale up touched {changed:?}");
        let idx = changed[0];
        let was = start[idx].unwrap_or(100);
        assert_eq!(after[idx], Some(was + 10));
        h.shell_action("cmd", "home.scale-down")
            .expect("scale down");
        let narrowed = marks(&h.save_document().expect("save down"));
        assert_eq!(narrowed[idx], start[idx], "scale down did not step back");
        h.shell_action("cmd", "edit.undo").expect("undo down");
        assert_eq!(h.save_document().expect("scaled again"), saved);
        h.shell_action("cmd", "edit.undo").expect("undo up");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.scale-up\""));
        assert!(strip.contains("data-g=\"home.scale-down\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"scale-up\""));
        assert!(home.contains("data-command=\"scale-down\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.scale-up"));
    }

    #[test]
    fn catches_shade_leaving_w_shd_off_the_saved_paragraph() {
        // Catches: Shade leaving w:shd off the saved paragraph, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    b.get("shadingFill")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.shading").expect("shade");
        let saved = h.save_document().expect("save shade");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "shade touched {changed:?}");
        let idx = changed[0];
        assert_eq!(after[idx].as_deref(), Some("#FFFF00"));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.shading\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"shading\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.shading"));
    }

    #[test]
    fn catches_bottom_border_leaving_w_pbdr_off_the_saved_paragraph() {
        // Catches: Bottom border leaving w:pBdr off the saved paragraph, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    b.get("borders")
                        .and_then(|v| v.get("bottom"))
                        .and_then(|v| v.get("style"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.border-bottom").expect("border");
        let saved = h.save_document().expect("save border");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "border touched {changed:?}");
        let idx = changed[0];
        assert_eq!(after[idx].as_deref(), Some("single"));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.border-bottom\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"border-bottom\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.border-bottom"));
    }

    #[test]
    fn catches_left_border_leaving_w_pbdr_off_the_saved_paragraph() {
        // Catches: Left border leaving w:pBdr off the saved paragraph, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    b.get("borders")
                        .and_then(|v| v.get("left"))
                        .and_then(|v| v.get("style"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.border-left")
            .expect("left border");
        let saved = h.save_document().expect("save left border");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "left border touched {changed:?}");
        let idx = changed[0];
        assert_eq!(after[idx].as_deref(), Some("single"));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.border-left\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"border-left\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.border-left"));
    }

    #[test]
    fn catches_double_border_leaving_w_pbdr_off_the_saved_paragraph() {
        // Catches: Double border leaving w:pBdr off the saved paragraph, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    b.get("borders")
                        .and_then(|v| v.get("bottom"))
                        .and_then(|v| v.get("style"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.border-double")
            .expect("double border");
        let saved = h.save_document().expect("save double border");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "double border touched {changed:?}");
        let idx = changed[0];
        assert_eq!(after[idx].as_deref(), Some("double"));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.border-double\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"border-double\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.border-double"));
    }

    #[test]
    fn catches_dashed_border_leaving_w_pbdr_off_the_saved_paragraph() {
        // Catches: Dashed border leaving w:pBdr off the saved paragraph, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    b.get("borders")
                        .and_then(|v| v.get("bottom"))
                        .and_then(|v| v.get("style"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.border-dash")
            .expect("dashed border");
        let saved = h.save_document().expect("save dashed border");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "dashed border touched {changed:?}");
        let idx = changed[0];
        assert_eq!(after[idx].as_deref(), Some("dashed"));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.border-dash\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"border-dash\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.border-dash"));
    }

    #[test]
    fn catches_dotted_border_leaving_w_pbdr_off_the_saved_paragraph() {
        // Catches: Dotted border leaving w:pBdr off the saved paragraph, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    b.get("borders")
                        .and_then(|v| v.get("bottom"))
                        .and_then(|v| v.get("style"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.border-dot")
            .expect("dotted border");
        let saved = h.save_document().expect("save dotted border");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "dotted border touched {changed:?}");
        let idx = changed[0];
        assert_eq!(after[idx].as_deref(), Some("dotted"));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.border-dot\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"border-dot\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.border-dot"));
    }

    #[test]
    fn catches_narrow_margins_leaving_w_pg_mar_at_one_inch_on_the_saved_document() {
        // Catches: Narrow margins leaving w:pgMar at one inch, or undoing as more than one step.
        let margins = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            let section = &h
                .docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .sections
                .as_ref()
                .unwrap()[0];
            (
                section.get("marginTopTwips").and_then(|v| v.as_i64()),
                section.get("marginRightTwips").and_then(|v| v.as_i64()),
                section.get("marginBottomTwips").and_then(|v| v.as_i64()),
                section.get("marginLeftTwips").and_then(|v| v.as_i64()),
            )
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = margins(DOCX);
        assert_eq!(start, (Some(1440), Some(1440), Some(1440), Some(1440)));
        h.shell_action("cmd", "home.narrow-margins")
            .expect("narrow margins");
        let saved = h.save_document().expect("save narrow margins");
        assert_ne!(saved, DOCX);
        assert_eq!(
            margins(&saved),
            (Some(720), Some(720), Some(720), Some(720))
        );
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.narrow-margins\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"narrow-margins\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.narrow-margins"));
    }

    #[test]
    fn catches_moderate_margins_leaving_w_pg_mar_sides_at_one_inch_on_the_saved_document() {
        // Catches: Moderate margins leaving w:pgMar left and right at one inch, or undoing as more than one step.
        let margins = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            let section = &h
                .docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .sections
                .as_ref()
                .unwrap()[0];
            (
                section.get("marginTopTwips").and_then(|v| v.as_i64()),
                section.get("marginRightTwips").and_then(|v| v.as_i64()),
                section.get("marginBottomTwips").and_then(|v| v.as_i64()),
                section.get("marginLeftTwips").and_then(|v| v.as_i64()),
            )
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = margins(DOCX);
        assert_eq!(start, (Some(1440), Some(1440), Some(1440), Some(1440)));
        h.shell_action("cmd", "home.moderate-margins")
            .expect("moderate margins");
        let saved = h.save_document().expect("save moderate margins");
        assert_ne!(saved, DOCX);
        assert_eq!(
            margins(&saved),
            (Some(1440), Some(1080), Some(1440), Some(1080))
        );
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.moderate-margins\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"moderate-margins\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.moderate-margins"));
    }

    #[test]
    fn catches_page_break_before_leaving_the_flag_off_the_saved_paragraph() {
        // Catches: Page break before leaving w:pageBreakBefore off the saved paragraph, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| b.get("pageBreakBefore").and_then(|v| v.as_bool()))
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.page-break-before")
            .expect("page break before");
        let saved = h.save_document().expect("save page break before");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "page break before touched {changed:?}");
        let idx = changed[0];
        assert_eq!(after[idx], Some(true));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.page-break-before\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"page-break-before\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.page-break-before"));
    }

    #[test]
    fn catches_landscape_leaving_pg_sz_portrait_on_the_saved_document() {
        // Catches: Landscape leaving w:pgSz portrait, or undoing as more than one step.
        let dims = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            let section = &h
                .docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .sections
                .as_ref()
                .unwrap()[0];
            (
                section.get("pageWidthTwips").and_then(|v| v.as_i64()),
                section.get("pageHeightTwips").and_then(|v| v.as_i64()),
                section
                    .get("orientation")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            )
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = dims(DOCX);
        assert!(start.0.unwrap() < start.1.unwrap());
        h.shell_action("cmd", "home.landscape").expect("landscape");
        let saved = h.save_document().expect("save landscape");
        assert_ne!(saved, DOCX);
        let after = dims(&saved);
        assert_eq!(after.0, start.1);
        assert_eq!(after.1, start.0);
        assert_eq!(after.2.as_deref(), Some("landscape"));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.landscape\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"landscape\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.landscape"));
    }

    #[test]
    fn catches_wide_margins_leaving_w_pg_mar_sides_at_one_inch_on_the_saved_document() {
        // Catches: Wide margins leaving w:pgMar left and right at one inch, or undoing as more than one step.
        let margins = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            let section = &h
                .docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .sections
                .as_ref()
                .unwrap()[0];
            (
                section.get("marginTopTwips").and_then(|v| v.as_i64()),
                section.get("marginRightTwips").and_then(|v| v.as_i64()),
                section.get("marginBottomTwips").and_then(|v| v.as_i64()),
                section.get("marginLeftTwips").and_then(|v| v.as_i64()),
            )
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = margins(DOCX);
        assert_eq!(start, (Some(1440), Some(1440), Some(1440), Some(1440)));
        h.shell_action("cmd", "home.wide-margins")
            .expect("wide margins");
        let saved = h.save_document().expect("save wide margins");
        assert_ne!(saved, DOCX);
        assert_eq!(
            margins(&saved),
            (Some(1440), Some(2880), Some(1440), Some(2880))
        );
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.wide-margins\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"wide-margins\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.wide-margins"));
    }

    #[test]
    fn catches_a4_leaving_pg_sz_letter_on_the_saved_document() {
        // Catches: A4 leaving w:pgSz at US Letter, or undoing as more than one step.
        let dims = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            let section = &h
                .docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .sections
                .as_ref()
                .unwrap()[0];
            (
                section.get("pageWidthTwips").and_then(|v| v.as_i64()),
                section.get("pageHeightTwips").and_then(|v| v.as_i64()),
            )
        };
        let mut h = open_editable(DOCX, "pages.docx");
        assert_eq!(dims(DOCX), (Some(12240), Some(15840)));
        h.shell_action("cmd", "home.page-a4").expect("a4");
        let saved = h.save_document().expect("save a4");
        assert_ne!(saved, DOCX);
        assert_eq!(dims(&saved), (Some(11906), Some(16838)));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.page-a4\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"page-a4\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.page-a4"));
    }

    #[test]
    fn catches_legal_leaving_pg_sz_letter_on_the_saved_document() {
        // Catches: Legal leaving w:pgSz at US Letter, or undoing as more than one step.
        let dims = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            let section = &h
                .docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .sections
                .as_ref()
                .unwrap()[0];
            (
                section.get("pageWidthTwips").and_then(|v| v.as_i64()),
                section.get("pageHeightTwips").and_then(|v| v.as_i64()),
            )
        };
        let mut h = open_editable(DOCX, "pages.docx");
        assert_eq!(dims(DOCX), (Some(12240), Some(15840)));
        h.shell_action("cmd", "home.page-legal").expect("legal");
        let saved = h.save_document().expect("save legal");
        assert_ne!(saved, DOCX);
        assert_eq!(dims(&saved), (Some(12240), Some(20160)));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.page-legal\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"page-legal\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.page-legal"));
    }

    #[test]
    fn catches_top_border_leaving_w_pbdr_off_the_saved_paragraph() {
        // Catches: Top border leaving w:pBdr off the saved paragraph, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    b.get("borders")
                        .and_then(|v| v.get("top"))
                        .and_then(|v| v.get("style"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.border-top")
            .expect("top border");
        let saved = h.save_document().expect("save top border");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "top border touched {changed:?}");
        let idx = changed[0];
        assert_eq!(after[idx].as_deref(), Some("single"));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.border-top\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"border-top\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.border-top"));
    }

    #[test]
    fn catches_right_border_leaving_w_pbdr_off_the_saved_paragraph() {
        // Catches: Right border leaving w:pBdr off the saved paragraph, or undoing as more than one step.
        let marks = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .map(|b| {
                    b.get("borders")
                        .and_then(|v| v.get("right"))
                        .and_then(|v| v.get("style"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        };
        let mut h = open_editable(DOCX, "pages.docx");
        let start = marks(DOCX);
        h.shell_action("cmd", "home.border-right")
            .expect("right border");
        let saved = h.save_document().expect("save right border");
        assert_ne!(saved, DOCX);
        let after = marks(&saved);
        let changed = start
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "right border touched {changed:?}");
        let idx = changed[0];
        assert_eq!(after[idx].as_deref(), Some("single"));
        h.shell_action("cmd", "edit.undo").expect("undo");
        assert_eq!(h.save_document().expect("undone"), DOCX);

        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.border-right\""));
        let home = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"border-right\""));
        let pptx = open_editable(PPTX, "slides.pptx");
        assert!(!pptx
            .pptx
            .as_ref()
            .unwrap()
            .ribbon_html()
            .contains("home.border-right"));
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
    fn catches_grow_font_jumping_to_a_fixed_size() {
        // Catches: Grow Font writing a fixed size instead of the next Office size, or undo
        // rolling back the shrink and the grow together.
        let docx_sizes = |bytes: &[u8]| {
            let h = open_editable(bytes, "pages.docx");
            h.docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks
                .iter()
                .flat_map(|b| {
                    b.get("runs")
                        .and_then(|r| r.as_array())
                        .into_iter()
                        .flatten()
                })
                .map(|run| run.get("fontSize").and_then(|v| v.as_f64()))
                .collect::<Vec<_>>()
        };
        let pptx_sizes = |bytes: &[u8]| {
            nitro_xml_pptx::editor::PptxEditor::open(bytes)
                .expect("open pptx")
                .session
                .host
                .slides
                .iter()
                .flat_map(|s| s.elements.iter())
                .map(|el| el.text_style.as_ref().and_then(|style| style.font_size))
                .collect::<Vec<_>>()
        };
        let near = |a: f64, b: f64| (a - b).abs() < 0.05;

        let mut h = open_editable(DOCX, "pages.docx");
        let before = docx_sizes(DOCX);
        h.shell_action("cmd", "home.grow-font").expect("grow");
        let grown = h.save_document().expect("save grow");
        assert_ne!(grown, DOCX);
        let after = docx_sizes(&grown);
        let changed = before
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "grow touched {changed:?}");
        let idx = changed[0];
        let expected =
            nitro_xml_docx::edit::format::step_office_font_pt(before[idx].unwrap_or(11.0), true);
        assert!(
            after[idx].is_some_and(|n| near(n, expected)),
            "grew to {:?} want {expected}",
            after[idx]
        );
        h.shell_action("cmd", "home.shrink-font").expect("shrink");
        let shrunk = h.save_document().expect("save shrink");
        let down = docx_sizes(&shrunk);
        let expected_down =
            nitro_xml_docx::edit::format::step_office_font_pt(after[idx].unwrap(), false);
        assert!(
            down[idx].is_some_and(|n| near(n, expected_down)),
            "shrank to {:?} want {expected_down}",
            down[idx]
        );
        h.shell_action("cmd", "edit.undo").expect("undo shrink");
        assert_eq!(h.save_document().expect("shrink undone"), grown);
        h.shell_action("cmd", "edit.undo").expect("undo grow");
        assert_eq!(h.save_document().expect("grow undone"), DOCX);
        let strip = h.docx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.grow-font\""));
        assert!(strip.contains("data-g=\"home.shrink-font\""));

        let mut p = open_editable(PPTX, "slides.pptx");
        let before = pptx_sizes(PPTX);
        p.shell_action("cmd", "home.grow-font").expect("grow slide");
        let grown = p.save_document().expect("save slide grow");
        assert_ne!(grown, PPTX);
        let after = pptx_sizes(&grown);
        let changed = before
            .iter()
            .zip(after.iter())
            .enumerate()
            .filter(|(_, (a, b))| match (a, b) {
                (Some(x), Some(y)) => !near(*x, *y),
                _ => a != b,
            })
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert_eq!(changed.len(), 1, "slide grow touched {changed:?}");
        let idx = changed[0];
        let expected = nitro_xml_pptx::editor::step_shape_font_px(before[idx], true);
        assert!(
            after[idx].is_some_and(|n| near(n, expected)),
            "slide grew to {:?} want {expected}",
            after[idx]
        );
        p.shell_action("cmd", "edit.undo").expect("undo slide grow");
        assert_eq!(p.save_document().expect("slide undone"), PPTX);
        let strip = p.pptx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.grow-font\""));
        assert!(strip.contains("data-g=\"home.shrink-font\""));
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

    #[test]
    fn catches_table_cell_text_missing_after_reopen_or_undo() {
        // Catches: table.cell-text leaving the saved cell unchanged, or a second click pushing another undo step.
        let mut h = open_editable(DOCX, "pages.docx");
        h.shell_action("cmd", "table.cell-text").expect("cell");
        let saved = h.save_document().expect("save cell");
        assert_ne!(saved, DOCX);
        let again = open_editable(&saved, "pages.docx");
        let text = docx_first_cell(&again);
        assert_eq!(text, "Nitro cell", "reopen dropped cell text");
        h.shell_action("cmd", "table.cell-text").expect("noop");
        assert_eq!(h.save_document().expect("second click"), saved);
        h.shell_action("cmd", "edit.undo").expect("undo cell");
        assert_eq!(h.save_document().expect("undone cell"), DOCX);

        let mut h = open_editable(PPTX, "slides.pptx");
        h.shell_action("cmd", "insert.table").expect("table");
        let inserted = h.save_document().expect("save table");
        h.shell_action("cmd", "table.cell-text").expect("cell");
        let saved = h.save_document().expect("save cell");
        assert_ne!(saved, inserted);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen");
        let cell = again
            .session
            .host
            .slides
            .iter()
            .flat_map(|s| s.elements.iter())
            .find(|el| el.type_ == "table")
            .and_then(|el| el.table.as_ref())
            .and_then(|t| t.rows.first())
            .and_then(|r| r.cells.first())
            .map(|c| c.text.as_str())
            .unwrap_or("");
        assert_eq!(cell, "Nitro cell", "reopen dropped slide cell text");
        h.shell_action("cmd", "table.cell-text").expect("noop");
        assert_eq!(h.save_document().expect("second click"), saved);
        h.shell_action("cmd", "edit.undo").expect("undo cell");
        assert_eq!(h.save_document().expect("undone cell"), inserted);
        h.shell_action("cmd", "edit.undo").expect("undo table");
        assert_eq!(h.save_document().expect("undone table"), PPTX);
    }

    fn docx_first_cell(h: &Handle) -> String {
        let host = &h.docx.as_ref().unwrap().edit.as_ref().unwrap().session.host;
        let table = host
            .blocks
            .iter()
            .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("table"))
            .expect("table");
        let runs = table["rows"][0][0]["paragraphs"][0]["runs"]
            .as_array()
            .expect("runs");
        runs.iter()
            .filter_map(|run| run.get("text").and_then(|v| v.as_str()))
            .collect()
    }

    fn page_breaks(bytes: &[u8]) -> usize {
        open_editable(bytes, "pages.docx")
            .docx
            .unwrap()
            .edit
            .unwrap()
            .session
            .host
            .blocks
            .iter()
            .filter_map(|b| b.get("runs")?.as_array())
            .flatten()
            .filter(|run| run.get("break").and_then(|v| v.as_str()) == Some("page"))
            .count()
    }

    #[test]
    fn catches_word_style_gallery_and_page_break_missing_after_reopen_or_undo() {
        // Catches: a style-gallery or page-break command leaving the saved docx unchanged,
        // dropping w:pStyle on reopen, or undo keeping the edit.
        let styles = [
            ("home.style-normal", "Normal"),
            ("home.style-title", "Title"),
            ("home.style-heading-1", "Heading1"),
            ("home.style-heading-2", "Heading2"),
            ("home.style-heading-3", "Heading3"),
        ];
        for (cmd, style_id) in styles {
            let mut h = open_editable(DOCX, "pages.docx");
            h.shell_action("cmd", cmd).expect(cmd);
            let saved = h.save_document().expect("save");
            assert_ne!(saved, DOCX, "{cmd} did not change the package");
            let again = open_editable(&saved, "pages.docx");
            let got = again
                .docx
                .as_ref()
                .unwrap()
                .edit
                .as_ref()
                .unwrap()
                .session
                .host
                .blocks[0]["style"]
                .as_str()
                .map(str::to_owned);
            assert_eq!(
                got.as_deref(),
                Some(style_id),
                "{cmd} style missing after reopen"
            );
            h.shell_action("cmd", cmd).expect("second click");
            assert_eq!(
                h.save_document().expect("second save"),
                saved,
                "{cmd} twice rewrote an already-applied style"
            );
            h.shell_action("cmd", "edit.undo").expect("undo");
            assert_eq!(h.save_document().expect("undone"), DOCX, "{cmd} undo");
        }

        let before = page_breaks(DOCX);
        let mut h = open_editable(DOCX, "pages.docx");
        h.shell_action("cmd", "insert.page-break").expect("break");
        let saved = h.save_document().expect("save break");
        assert_ne!(saved, DOCX);
        assert_eq!(
            page_breaks(&saved),
            before + 1,
            "reopen dropped the page break"
        );
        h.shell_action("cmd", "edit.undo").expect("undo break");
        assert_eq!(h.save_document().expect("break undone"), DOCX);
    }

    #[test]
    fn catches_shape_fill_outline_and_layout_missing_after_reopen_or_undo() {
        // Catches: ribbon Fill, Outline, or Layout leaving the saved package unchanged or undo keeping the edit.
        let mut h = open_editable(PPTX, "slides.pptx");
        let ui = h.shell_action("ui", "").expect("ui");
        assert!(
            ui.contains("home.shape-fill"),
            "fill is missing from the ribbon"
        );
        assert!(
            ui.contains("home.shape-outline"),
            "outline is missing from the ribbon"
        );
        assert!(
            ui.contains("home.layout"),
            "layout is missing from the ribbon"
        );
        h.shell_action("cmd", "home.layout").expect("one layout");
        assert_eq!(
            h.save_document().expect("one"),
            PPTX,
            "one-layout deck changed"
        );
        h.shell_action("cmd", "home.shape-fill").expect("fill");
        let filled = h.save_document().expect("filled");
        assert_ne!(filled, PPTX);
        assert!(
            reopened_paint(&filled).contains("5B2C6F"),
            "reopen lost the shape fill"
        );
        h.shell_action("cmd", "home.shape-fill")
            .expect("fill again");
        assert_eq!(h.save_document().expect("fill again"), filled);
        h.shell_action("cmd", "edit.undo").expect("undo fill");
        assert_eq!(h.save_document().expect("undone fill"), PPTX);
        h.shell_action("cmd", "home.shape-outline")
            .expect("outline");
        let outlined = h.save_document().expect("outlined");
        assert_ne!(outlined, PPTX);
        assert!(
            reopened_paint(&outlined).contains("00B050"),
            "reopen lost the outline"
        );
        h.shell_action("cmd", "edit.undo").expect("undo outline");
        assert_eq!(h.save_document().expect("undone outline"), PPTX);

        let layout =
            include_bytes!("../../../../fixtures/upstream/pptx/51edb06a949d_text-layout.pptx");
        let mut h = open_editable(layout, "text-layout.pptx");
        h.shell_action("cmd", "home.layout").expect("layout");
        let saved = h.save_document().expect("layout save");
        assert_ne!(saved, layout);
        let mut pres = nitro_xml_pptx::Presentation::open(&saved).expect("reopen");
        let slide = pres.slide(0).expect("slide");
        assert_eq!(
            slide.layout_path.as_deref(),
            Some("ppt/slideLayouts/slideLayout2.xml")
        );
        h.shell_action("cmd", "edit.undo").expect("undo layout");
        assert_eq!(h.save_document().expect("undone layout"), layout);
    }

    fn reopened_paint(bytes: &[u8]) -> String {
        let mut pres = nitro_xml_pptx::Presentation::open(bytes).expect("reopen");
        let slide = pres.slide(0).expect("slide");
        let path = slide.path.clone();
        slide
            .elements
            .iter()
            .filter(|el| el.id.starts_with(&path))
            .map(|el| {
                format!(
                    "{} {} {}",
                    el.fill_color.as_deref().unwrap_or(""),
                    el.fill_mode.as_deref().unwrap_or(""),
                    el.stroke_color.as_deref().unwrap_or("")
                )
            })
            .collect::<String>()
            .to_ascii_uppercase()
    }

    fn slide_elements(bytes: &[u8]) -> Vec<nitro_xml_pptx::edit::host::EditableElement> {
        nitro_xml_pptx::editor::PptxEditor::open(bytes)
            .expect("reopen")
            .session
            .host
            .slides
            .into_iter()
            .next()
            .expect("slide")
            .elements
    }

    #[test]
    fn catches_insert_shape_duplicate_delete_and_z_order_missing_after_reopen_or_undo() {
        // Catches: insert shape, duplicate, delete, or z-order leaving the saved slide
        // unchanged, a no-op z-order rewriting a one-shape slide, or undo keeping the edit.
        let count = |bytes: &[u8]| slide_elements(bytes).len();
        let before = count(PPTX);
        assert_eq!(before, 1, "slides_10 slide 1 is the one-shape case");

        let mut h = open_editable(PPTX, "slides.pptx");
        h.shell_action("cmd", "home.bring-forward")
            .expect("noop forward");
        h.shell_action("cmd", "home.send-backward")
            .expect("noop back");
        assert_eq!(
            h.save_document().expect("noop z-order"),
            PPTX,
            "z-order on the only shape changed the package"
        );

        h.shell_action("cmd", "insert.shape").expect("shape");
        let saved = h.save_document().expect("save shape");
        assert_ne!(saved, PPTX);
        let inserted = slide_elements(&saved);
        assert_eq!(inserted.len(), before + 1, "reopen dropped the shape");
        let rect = inserted.iter().find(|el| {
            el.shape_type.as_deref() == Some("rect")
                && (el.x - 100.0).abs() < 1.0
                && (el.y - 100.0).abs() < 1.0
                && (el.width - 200.0).abs() < 1.0
                && (el.height - 120.0).abs() < 1.0
        });
        assert!(rect.is_some(), "inserted rect missing after reopen");
        let title_at = inserted.iter().position(|el| {
            el.text.as_deref().unwrap_or("").contains("Slide 1")
                || el.text_segments.iter().any(|s| s.text.contains("Slide 1"))
        });
        assert_eq!(
            title_at,
            Some(0),
            "insert put the title behind the new shape"
        );

        h.shell_action("cmd", "home.send-backward")
            .expect("send back");
        let ordered = h.save_document().expect("save order");
        assert_ne!(ordered, saved);
        let after_order = slide_elements(&ordered);
        assert_eq!(after_order.len(), before + 1);
        let title_at = after_order.iter().position(|el| {
            el.text.as_deref().unwrap_or("").contains("Slide 1")
                || el.text_segments.iter().any(|s| s.text.contains("Slide 1"))
        });
        assert_eq!(
            title_at,
            Some(1),
            "send backward did not move the new shape behind the title"
        );
        let front = &after_order[0];
        assert_eq!(front.shape_type.as_deref(), Some("rect"));
        assert!((front.x - 100.0).abs() < 1.0, "x {}", front.x);

        h.shell_action("cmd", "home.bring-forward")
            .expect("bring forward");
        let forward = h.save_document().expect("save forward");
        let after_forward = slide_elements(&forward);
        let title_at = after_forward.iter().position(|el| {
            el.text.as_deref().unwrap_or("").contains("Slide 1")
                || el.text_segments.iter().any(|s| s.text.contains("Slide 1"))
        });
        assert_eq!(
            title_at,
            Some(0),
            "bring forward did not restore the title to the back"
        );
        h.shell_action("cmd", "edit.undo").expect("undo forward");
        h.shell_action("cmd", "edit.undo").expect("undo back");
        h.shell_action("cmd", "edit.undo").expect("undo shape");
        assert_eq!(h.save_document().expect("shape undone"), PPTX);

        let mut h = open_editable(PPTX, "slides.pptx");
        h.shell_action("cmd", "home.duplicate").expect("duplicate");
        let saved = h.save_document().expect("save duplicate");
        assert_ne!(saved, PPTX);
        assert_eq!(count(&saved), before + 1, "reopen dropped the duplicate");
        h.shell_action("cmd", "edit.undo").expect("undo duplicate");
        assert_eq!(h.save_document().expect("duplicate undone"), PPTX);

        let mut h = open_editable(PPTX, "slides.pptx");
        h.shell_action("cmd", "home.delete").expect("delete");
        let saved = h.save_document().expect("save delete");
        assert_ne!(saved, PPTX);
        assert_eq!(
            count(&saved),
            before - 1,
            "deleted shape still in the package"
        );
        h.shell_action("cmd", "edit.undo").expect("undo delete");
        assert_eq!(h.save_document().expect("delete undone"), PPTX);
    }

    #[test]
    fn catches_bring_to_front_and_send_to_back_saving_one_step_order() {
        // Catches: Bring to Front or Send to Back leaving the saved spTree in one-step
        // order, a no-op on a single shape rewriting the package, or undo keeping the jump.
        let order = |bytes: &[u8]| {
            slide_elements(bytes)
                .into_iter()
                .map(|el| {
                    let text = el.text.clone().unwrap_or_default();
                    let titled = text.contains("Slide 1")
                        || el.text_segments.iter().any(|s| s.text.contains("Slide 1"));
                    if titled {
                        "title"
                    } else if (el.x - 100.0).abs() < 1.0 && (el.width - 200.0).abs() < 1.0 {
                        "rect"
                    } else if (el.x - 80.0).abs() < 1.0 && (el.width - 240.0).abs() < 1.0 {
                        "box"
                    } else {
                        "other"
                    }
                })
                .collect::<Vec<_>>()
        };

        let mut h = open_editable(PPTX, "slides.pptx");
        h.shell_action("cmd", "home.bring-to-front")
            .expect("noop front");
        h.shell_action("cmd", "home.send-to-back")
            .expect("noop back");
        assert_eq!(
            h.save_document().expect("noop z-order"),
            PPTX,
            "z-order on the only shape changed the package"
        );

        h.shell_action("cmd", "insert.shape").expect("shape");
        h.shell_action("cmd", "insert.text-box").expect("text box");
        let stacked = h.save_document().expect("stacked");
        let stacked_ids = order(&stacked);
        assert_eq!(
            stacked_ids,
            ["title", "rect", "box"],
            "insert order {stacked_ids:?}"
        );

        h.shell_action("cmd", "home.send-to-back")
            .expect("send to back");
        let sent = h.save_document().expect("save back");
        assert_ne!(sent, stacked);
        let sent_ids = order(&sent);
        assert_eq!(
            sent_ids,
            ["box", "title", "rect"],
            "send to back did not jump the text box behind both others"
        );
        assert_ne!(
            sent_ids,
            ["title", "box", "rect"],
            "send to back only swapped with the neighbor"
        );

        h.shell_action("cmd", "home.bring-to-front")
            .expect("bring to front");
        let brought = h.save_document().expect("save front");
        assert_ne!(brought, sent);
        let brought_ids = order(&brought);
        assert_eq!(
            brought_ids,
            ["title", "rect", "box"],
            "bring to front did not jump the text box over both others"
        );
        assert_ne!(
            brought_ids,
            ["title", "box", "rect"],
            "bring to front only swapped with the neighbor"
        );

        h.shell_action("cmd", "edit.undo").expect("undo front");
        assert_eq!(order(&h.save_document().expect("undone front")), sent_ids);
        h.shell_action("cmd", "edit.undo").expect("undo back");
        assert_eq!(order(&h.save_document().expect("undone back")), stacked_ids);
        h.shell_action("cmd", "edit.undo").expect("undo text");
        h.shell_action("cmd", "edit.undo").expect("undo shape");
        assert_eq!(h.save_document().expect("clean"), PPTX);

        let strip = h.pptx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.bring-to-front\""));
        assert!(strip.contains("data-g=\"home.send-to-back\""));
        assert!(strip.contains("data-g=\"home.group\""));
        assert!(strip.contains("data-g=\"home.ungroup\""));
        let home = h
            .pptx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(home.contains("data-command=\"home.bring-to-front\""));
        assert!(home.contains("data-command=\"home.send-to-back\""));
        assert!(home.contains("data-command=\"home.group\""));
        assert!(home.contains("data-command=\"home.ungroup\""));
    }

    #[test]
    fn catches_group_dropping_shapes_or_undoing_with_ungroup() {
        // Catches: Group saving no p:grpSp, or one undo rolling group and ungroup back together.
        let mut h = open_editable(PPTX, "slides.pptx");
        h.shell_action("cmd", "insert.shape").expect("shape");
        let with_shape = h.save_document().expect("save shape");
        let ids = {
            let edit = h.pptx.as_ref().unwrap().edit.as_ref().unwrap();
            edit.session.host.slides[0]
                .elements
                .iter()
                .take(2)
                .map(|el| el.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids.len(), 2, "group needs two shapes");
        h.pptx.as_mut().unwrap().selected_ids = ids;
        h.shell_action("cmd", "home.group").expect("group");
        let grouped = h.save_document().expect("save group");
        assert_ne!(grouped, with_shape);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&grouped).expect("reopen group");
        assert_eq!(
            again.session.host.slides[0]
                .elements
                .iter()
                .filter(|el| el.type_ == "group")
                .count(),
            1,
            "group did not survive save"
        );
        h.shell_action("cmd", "edit.undo").expect("undo group");
        assert_eq!(h.save_document().expect("group undone"), with_shape);
        h.shell_action("cmd", "edit.undo").expect("undo shape");
        assert_eq!(h.save_document().expect("shape undone"), PPTX);

        h.shell_action("cmd", "insert.shape").expect("shape again");
        let with_shape = h.save_document().expect("save shape again");
        h.pptx.as_mut().unwrap().selected_ids = {
            let edit = h.pptx.as_ref().unwrap().edit.as_ref().unwrap();
            edit.session.host.slides[0]
                .elements
                .iter()
                .take(2)
                .map(|el| el.id.clone())
                .collect()
        };
        h.shell_action("cmd", "home.group").expect("group again");
        let grouped = h.save_document().expect("save group again");
        h.shell_action("cmd", "home.ungroup").expect("ungroup");
        let flat = h.save_document().expect("save ungroup");
        let flat_host = nitro_xml_pptx::editor::PptxEditor::open(&flat).expect("reopen flat");
        assert!(
            flat_host.session.host.slides[0]
                .elements
                .iter()
                .all(|el| el.type_ != "group"),
            "ungroup left a group"
        );
        h.shell_action("cmd", "edit.undo").expect("undo ungroup");
        assert_eq!(h.save_document().expect("ungroup undone"), grouped);
        h.shell_action("cmd", "edit.undo").expect("undo group");
        assert_eq!(h.save_document().expect("group undone"), with_shape);

        let strip = h.pptx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"home.group\""));
        assert!(strip.contains("data-g=\"home.ungroup\""));
        let word = open_editable(DOCX, "pages.docx");
        let word_strip = word.docx.as_ref().unwrap().ribbon_html();
        assert!(!word_strip.contains("home.group"));
        assert!(!word_strip.contains("home.ungroup"));
    }

    #[test]
    fn catches_pptx_delete_table_removing_the_selected_shape_or_undoing_with_insert() {
        // Catches: Delete Table removing the selected shape instead of the table, or one undo
        // rolling the delete and the insert back together.
        let open = |bytes: &[u8]| nitro_xml_pptx::editor::PptxEditor::open(bytes).expect("open");
        let tables = |bytes: &[u8]| {
            open(bytes)
                .session
                .host
                .slides
                .iter()
                .flat_map(|s| s.elements.iter())
                .filter(|el| el.type_ == "table")
                .count()
        };
        let titled = |bytes: &[u8]| {
            open(bytes).session.host.slides[0]
                .elements
                .iter()
                .filter(|el| {
                    el.text.as_deref().unwrap_or("").contains("Slide 1")
                        || el.text_segments.iter().any(|s| s.text.contains("Slide 1"))
                })
                .count()
        };
        let mut h = open_editable(PPTX, "slides.pptx");
        h.shell_action("cmd", "table.delete").expect("noop");
        assert_eq!(
            h.save_document().expect("noop save"),
            PPTX,
            "delete with no table changed the package"
        );
        let before = tables(PPTX);
        let title_before = titled(PPTX);
        assert!(title_before >= 1, "fixture title missing");
        h.shell_action("cmd", "insert.table").expect("table");
        let with_table = h.save_document().expect("save table");
        assert_eq!(tables(&with_table), before + 1);
        let title_id = {
            let edit = h.pptx.as_ref().unwrap().edit.as_ref().unwrap();
            edit.session.host.slides[0]
                .elements
                .iter()
                .find(|el| el.type_ != "table")
                .map(|el| el.id.clone())
                .expect("non-table shape")
        };
        h.pptx.as_mut().unwrap().selected_ids = vec![title_id];
        h.shell_action("cmd", "table.delete").expect("delete");
        let deleted = h.save_document().expect("save delete");
        assert_ne!(deleted, with_table);
        assert_eq!(tables(&deleted), before, "table still in the package");
        assert_eq!(
            titled(&deleted),
            title_before,
            "delete removed the selected shape"
        );
        h.shell_action("cmd", "edit.undo").expect("undo delete");
        assert_eq!(h.save_document().expect("delete undone"), with_table);
        h.shell_action("cmd", "edit.undo").expect("undo insert");
        assert_eq!(h.save_document().expect("insert undone"), PPTX);
        let strip = h.pptx.as_ref().unwrap().ribbon_html();
        assert!(strip.contains("data-g=\"table.delete\""));
    }

    fn apply_cmd(h: &mut Handle, cmd: &'static str) {
        h.apply_edit(&format!(r#"{{"command":"{cmd}"}}"#))
            .expect(cmd);
    }

    #[test]
    fn catches_slide_and_table_commands_missing_after_apply_edit_save_or_undo() {
        // Catches: applyEdit of add/delete/duplicate/reorder slide or table row/column
        // leaving the saved package unchanged, or undo keeping that edit.
        let mut h = open_editable(PPTX, "slides.pptx");
        let before = h.pptx.as_ref().unwrap().pres.slide_count();
        apply_cmd(&mut h, "insert.duplicate-slide");
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
            text(&again.session.host.slides[0]),
            "applyEdit duplicate did not copy the slide"
        );
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(h.save_document().expect("undone dup"), PPTX);

        apply_cmd(&mut h, "insert.slide");
        let saved = h.save_document().expect("save slide");
        assert_ne!(saved, PPTX);
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen slide");
        assert_eq!(again.session.host.slides.len(), before + 1);
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(h.save_document().expect("undone slide"), PPTX);

        apply_cmd(&mut h, "home.delete-slide");
        let saved = h.save_document().expect("save delete");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen delete");
        assert_eq!(again.session.host.slides.len(), before - 1);
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(h.save_document().expect("undone delete"), PPTX);

        {
            let edit = h.pptx.as_mut().unwrap().edit.as_mut().unwrap();
            let home = edit.ribbon_html();
            assert!(home.contains("data-command=\"home.group\""));
            assert!(home.contains("data-command=\"home.ungroup\""));
            edit.set_active_tab("insert");
            let insert = edit.ribbon_html();
            for id in [
                "insert.slide",
                "insert.duplicate-slide",
                "insert.move-slide-up",
                "insert.move-slide-down",
                "home.delete-slide",
                "insert.table",
                "table.insert-row",
                "table.delete-row",
                "table.insert-column",
                "table.delete-column",
                "table.delete",
            ] {
                assert!(
                    insert.contains(&format!("data-command=\"{id}\"")),
                    "insert ribbon dropped {id}"
                );
            }
            edit.set_active_tab("home");
        }
        let strip = h.pptx.as_ref().unwrap().ribbon_html();
        for id in [
            "insert.slide",
            "insert.duplicate-slide",
            "insert.move-slide-down",
            "home.delete-slide",
            "insert.table",
            "table.insert-row",
            "table.delete-column",
            "table.delete",
        ] {
            assert!(
                strip.contains(&format!("data-g=\"{id}\"")),
                "wasm strip dropped {id}"
            );
        }
        assert!(strip.contains("data-g=\"home.group\""));
        assert!(strip.contains("data-g=\"home.ungroup\""));

        let mut h = open_editable(PPTX, "slides.pptx");
        let first = h.pptx.as_ref().unwrap().pres.slide_count();
        assert!(first >= 2, "reorder needs two slides");
        apply_cmd(&mut h, "insert.move-slide-up");
        assert_eq!(
            h.save_document().expect("noop up"),
            PPTX,
            "move up on the first slide changed the package"
        );
        let paths = {
            let edit = h.pptx.as_ref().unwrap().edit.as_ref().unwrap();
            (
                edit.session.host.slides[0].path.clone(),
                edit.session.host.slides[1].path.clone(),
            )
        };
        apply_cmd(&mut h, "insert.move-slide-down");
        let saved = h.save_document().expect("save move");
        let again = nitro_xml_pptx::editor::PptxEditor::open(&saved).expect("reopen move");
        assert_eq!(again.session.host.slides[0].path, paths.1);
        assert_eq!(again.session.host.slides[1].path, paths.0);
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(h.save_document().expect("undone move"), PPTX);

        apply_cmd(&mut h, "insert.table");
        apply_cmd(&mut h, "table.insert-row");
        apply_cmd(&mut h, "table.insert-column");
        let saved = h.save_document().expect("save table");
        let dims = |bytes: &[u8]| {
            let again = nitro_xml_pptx::editor::PptxEditor::open(bytes).expect("reopen table");
            let table = again
                .session
                .host
                .slides
                .iter()
                .flat_map(|s| s.elements.iter())
                .find(|el| el.type_ == "table")
                .expect("table missing after applyEdit");
            let model = table.table.as_ref().expect("model");
            (model.rows.len(), model.column_widths.len())
        };
        assert_eq!(dims(&saved), (3, 3));
        apply_cmd(&mut h, "table.delete-row");
        apply_cmd(&mut h, "table.delete-column");
        let saved = h.save_document().expect("save trim");
        assert_eq!(dims(&saved), (2, 2));
        for _ in 0..5 {
            apply_cmd(&mut h, "edit.undo");
        }
        assert_eq!(h.save_document().expect("undone table"), PPTX);

        let mut h = open_editable(DOCX, "pages.docx");
        // Empty undo only opens the session. It must not rewrite the package.
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(h.save_document().expect("warm"), DOCX);
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
        let tables_of = |h: &Handle| {
            h.docx
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
                .count()
        };
        let reopen = |bytes: &[u8]| {
            let mut again = open_editable(bytes, "pages.docx");
            apply_cmd(&mut again, "edit.undo");
            again
        };
        let before_tables = tables_of(&h);
        let (rows, cols) = rows_of(&h);
        apply_cmd(&mut h, "insert.table");
        let saved = h.save_document().expect("save docx table");
        assert_ne!(saved, DOCX);
        assert_eq!(
            tables_of(&reopen(&saved)),
            before_tables + 1,
            "applyEdit insert.table dropped the table"
        );
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(h.save_document().expect("undone docx table"), DOCX);
        assert_eq!(tables_of(&h), before_tables);

        apply_cmd(&mut h, "table.insert-row");
        let saved = h.save_document().expect("save row");
        assert_eq!(rows_of(&reopen(&saved)), (rows + 1, cols));
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(rows_of(&h), (rows, cols));
        assert_eq!(h.save_document().expect("undone row"), DOCX);

        apply_cmd(&mut h, "table.insert-row");
        apply_cmd(&mut h, "table.delete-row");
        let saved = h.save_document().expect("save delete row");
        assert_eq!(
            rows_of(&reopen(&saved)).0,
            rows,
            "deleted row still in the package"
        );
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(
            rows_of(&h).0,
            rows + 1,
            "undo did not restore the inserted row"
        );
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(h.save_document().expect("row undone"), DOCX);

        apply_cmd(&mut h, "table.insert-column");
        let saved = h.save_document().expect("save column");
        assert_eq!(rows_of(&reopen(&saved)).1, cols + 1);
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(h.save_document().expect("undone column"), DOCX);

        apply_cmd(&mut h, "table.insert-column");
        apply_cmd(&mut h, "table.delete-column");
        let saved = h.save_document().expect("save delete column");
        assert_eq!(
            rows_of(&reopen(&saved)).1,
            cols,
            "deleted column still in the package"
        );
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(rows_of(&h).1, cols + 1);
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(h.save_document().expect("column undone"), DOCX);

        apply_cmd(&mut h, "table.delete");
        let saved = h.save_document().expect("save delete table");
        assert_ne!(saved, DOCX);
        assert_eq!(tables_of(&reopen(&saved)), before_tables - 1);
        apply_cmd(&mut h, "edit.undo");
        assert_eq!(h.save_document().expect("undone delete table"), DOCX);

        let engine = h
            .docx
            .as_ref()
            .unwrap()
            .edit
            .as_ref()
            .unwrap()
            .ribbon_html();
        assert!(
            !engine.contains("data-command=\"paste\""),
            "paste is shown with no runner"
        );
        assert!(!engine.contains("data-command=\"cut\""));
        assert!(!engine.contains("data-command=\"copy\""));
        for id in [
            "insert.table",
            "table.insert-row",
            "table.delete-row",
            "table.insert-column",
            "table.delete-column",
            "table.delete",
        ] {
            assert!(
                engine.contains(&format!("data-command=\"{id}\"")),
                "word ribbon dropped {id}"
            );
        }
    }
}
