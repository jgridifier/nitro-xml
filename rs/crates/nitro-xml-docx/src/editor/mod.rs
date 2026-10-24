// SPDX-License-Identifier: MIT
//! Word editor UI: ribbon, shortcuts, find/replace, print-layout edit hooks (#83).

mod commands;
mod print_edit;
mod ribbon;
mod search;
mod shortcuts;

pub use commands::{CommandMeta, DocxCommandSurface, SearchPanelFocus, DOCX_EDITOR_COMMANDS};
pub use print_edit::{
    consume_relayout_pending, schedule_relayout, CaretPosition, DocxViewMode, PrintEditState,
};
pub use ribbon::{ribbon_html, search_panel_html};
pub use search::{SearchController, SearchMatch, SearchStatus};
pub use shortcuts::{find_shortcut, matches_keys, KeyEvent, Shortcut, ShortcutMatch, SHORTCUTS};

use nitro_xml_opc::edit::{EditError, EditHistory, EditSession};
use serde_json::Value;

use crate::edit::host::DocxHost;
use crate::layout::{create_fake_measurer, layout_document_model, LayoutReviewOptions};

/// Editable Word document shell: edit session + chrome command surface.
pub struct DocxEditor {
    pub session: EditSession<DocxHost, EditHistory>,
    pub commands: DocxCommandSurface,
    pub print: PrintEditState,
    active_tab: String,
    read_only: bool,
    caret_run: usize,
}

impl DocxEditor {
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        Ok(Self::from_host(DocxHost::open(bytes)?))
    }

    pub fn from_host(host: DocxHost) -> Self {
        Self {
            session: EditSession::new(host, EditHistory::new()),
            commands: DocxCommandSurface::new(false),
            print: PrintEditState::default(),
            active_tab: "home".to_string(),
            read_only: false,
            caret_run: 0,
        }
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
        self.commands.set_read_only(read_only);
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn active_tab(&self) -> &str {
        &self.active_tab
    }

    pub fn set_active_tab(&mut self, tab: &str) {
        self.active_tab = tab.to_string();
    }

    pub fn caret(&self) -> &CaretPosition {
        &self.print.caret
    }

    pub fn set_caret_block(&mut self, block_index: usize, offset: usize) {
        let block_id = self
            .session
            .host
            .blocks
            .get(block_index)
            .and_then(|b| b.get("id"))
            .and_then(|id| id.as_str())
            .unwrap_or("")
            .to_string();
        self.print.set_caret(CaretPosition {
            block_index,
            block_id,
            offset,
        });
    }

    pub fn ribbon_html(&self) -> String {
        format!(
            "{}{}",
            ribbon::ribbon_html(&self.commands, &self.active_tab),
            ribbon::search_panel_html(&self.commands)
        )
    }

    pub fn dispatch_command(&mut self, id: &str, args: &Value) -> Result<(), EditError> {
        let block = self.print.caret.block_index;
        let run = self.caret_run;
        self.commands
            .dispatch(id, args, &mut self.print, &mut self.session, block, run)
    }

    pub fn match_shortcut(&self, event: &KeyEvent, mac: bool) -> Option<ShortcutMatch> {
        if self.read_only && !shortcut_allowed_readonly(event, mac) {
            return None;
        }
        find_shortcut(event, mac)
    }

    pub fn handle_shortcut(&mut self, event: &KeyEvent, mac: bool) -> bool {
        let Some(m) = self.match_shortcut(event, mac) else {
            return false;
        };
        self.dispatch_command(m.command, &Value::Null).is_ok()
    }

    /// After edits, run a full layout when pending. Incremental path is TODO(#81).
    pub fn relayout_if_needed(&mut self) -> Option<crate::layout::LayoutResult> {
        if !consume_relayout_pending(&mut self.print) {
            return None;
        }
        let model = document_model_from_host(&self.session.host);
        let result = layout_document_model(
            &model,
            &create_fake_measurer(),
            LayoutReviewOptions::default(),
        );
        Some(result)
    }

    pub fn print_layout_click(&mut self, block_id: &str, offset: usize) -> Option<CaretPosition> {
        self.print
            .print_layout_click(&self.session.host, block_id, offset)
    }

    pub fn set_view_mode(&mut self, mode: DocxViewMode) {
        self.print.set_view_mode(mode);
    }

    pub fn view_mode(&self) -> DocxViewMode {
        self.print.mode
    }

    // TODO(#62): crash recovery snapshot / restore hooks for the editor shell.
}

fn shortcut_allowed_readonly(event: &KeyEvent, mac: bool) -> bool {
    let Some(m) = find_shortcut(event, mac) else {
        return false;
    };
    matches!(
        m.command,
        "find-and-replace" | "replace" | "print" | "layout-view"
    )
}

/// Minimal `DocumentModel` shape for [`layout_document_model`] after edits.
fn document_model_from_host(host: &DocxHost) -> crate::body::DocumentModel {
    crate::body::DocumentModel {
        format: host.format.clone(),
        blocks: host.blocks.clone(),
        page: host.page.clone().unwrap_or_default(),
        warnings: Vec::new(),
        paragraph_styles: host.paragraph_styles.clone(),
        character_styles: host.character_styles.clone(),
        table_styles: None,
        numbering_catalog: host.numbering_catalog.clone(),
        theme: None,
        sections: host.sections.as_ref().and_then(|v| v.as_array().cloned()),
        footnotes: None,
        endnotes: None,
        footnote_num_fmt: None,
        endnote_num_fmt: None,
        even_and_odd_headers: host.even_and_odd_headers,
        auto_hyphenation: false,
        comments: host.comments.clone(),
        track_changes: None,
        track_formatting: None,
        track_moves: None,
        media_part_names: Vec::new(),
        source_document_xml: Some(host.source_document_xml.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn catches_find_shortcut_opens_search_panel() {
        // Catches: Mod+F not wiring to find-and-replace (#83).
        let mut editor = DocxEditor {
            session: EditSession::new(
                DocxHost::stub(vec![
                    json!({ "type": "paragraph", "id": "p", "runs": [{ "text": "a" }] }),
                ]),
                EditHistory::new(),
            ),
            commands: DocxCommandSurface::new(false),
            print: PrintEditState::default(),
            active_tab: "home".into(),
            read_only: false,
            caret_run: 0,
        };
        let ev = KeyEvent {
            key: "f".into(),
            code: Some("KeyF".into()),
            ctrl: true,
            meta: false,
            shift: false,
            alt: false,
        };
        assert!(editor.handle_shortcut(&ev, false));
        assert!(editor.commands.search_panel_open);
        assert!(editor.ribbon_html().contains("Find and replace"));
    }
}
