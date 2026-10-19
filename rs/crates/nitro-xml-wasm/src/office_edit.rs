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
        let mut h = Handle::new(bytes);
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
        let mut again = Handle::new(&saved);
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
            let mut h = Handle::new(bytes);
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
}
