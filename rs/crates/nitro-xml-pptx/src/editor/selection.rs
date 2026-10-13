// SPDX-License-Identifier: MIT
//! Active slide and selected shape ids for the pptx editor chrome (#88).

/// Current slide index plus selected element ids (z-order / multi-select).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectionState {
    pub slide_index: u32,
    pub element_ids: Vec<String>,
}

impl SelectionState {
    pub fn new(slide_index: u32) -> Self {
        Self {
            slide_index,
            element_ids: Vec::new(),
        }
    }

    pub fn primary_id(&self) -> Option<&str> {
        self.element_ids.first().map(String::as_str)
    }

    pub fn has_selection(&self) -> bool {
        !self.element_ids.is_empty()
    }

    pub fn set_slide(&mut self, index: u32) {
        if self.slide_index != index {
            self.slide_index = index;
            self.element_ids.clear();
        }
    }

    pub fn select(&mut self, ids: impl IntoIterator<Item = String>) {
        self.element_ids = ids.into_iter().collect();
    }

    pub fn clear(&mut self) {
        self.element_ids.clear();
    }
}
