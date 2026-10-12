// SPDX-License-Identifier: MIT
//! Print Layout view mode and click-to-caret (#83).

use crate::edit::host::DocxHost;
use crate::editor::search::paragraph_plain_text;

/// Word view modes (upstream `pages.setViewMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocxViewMode {
    #[default]
    Draft,
    PrintLayout,
}

#[derive(Debug, Clone, Default)]
pub struct CaretPosition {
    pub block_index: usize,
    pub block_id: String,
    /// UTF-8 byte offset in the paragraph plain text.
    pub offset: usize,
}

#[derive(Debug, Clone, Default)]
pub struct PrintEditState {
    pub mode: DocxViewMode,
    pub caret: CaretPosition,
    pub relayout_pending: bool,
}

impl PrintEditState {
    pub fn set_view_mode(&mut self, mode: DocxViewMode) {
        self.mode = mode;
    }

    /// Print-layout click: switch to draft and place the caret (upstream `moveCursorToBlock`).
    pub fn print_layout_click(
        &mut self,
        host: &DocxHost,
        block_id: &str,
        offset: usize,
    ) -> Option<CaretPosition> {
        self.mode = DocxViewMode::Draft;
        let (bi, block) = host
            .blocks
            .iter()
            .enumerate()
            .find(|(_, b)| b.get("id").and_then(|id| id.as_str()) == Some(block_id))?;
        if block.get("type").and_then(|t| t.as_str()) != Some("paragraph") {
            return None;
        }
        let text = paragraph_plain_text(block);
        let offset = offset.min(text.len());
        let offset = if text.is_char_boundary(offset) {
            offset
        } else {
            text[..offset]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0)
        };
        self.caret = CaretPosition {
            block_index: bi,
            block_id: block_id.to_string(),
            offset,
        };
        schedule_relayout(self);
        Some(self.caret.clone())
    }

    pub fn set_caret(&mut self, caret: CaretPosition) {
        self.caret = caret;
    }
}

/// Flag a full layout pass after model edits. Incremental relayout is TODO(#81).
pub fn schedule_relayout(state: &mut PrintEditState) {
    state.relayout_pending = true;
}

/// Run layout when pending. Returns true when a pass was scheduled (caller paints).
pub fn consume_relayout_pending(state: &mut PrintEditState) -> bool {
    let pending = state.relayout_pending;
    state.relayout_pending = false;
    pending
}
