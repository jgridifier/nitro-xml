// SPDX-License-Identifier: MIT

use crate::model::ActiveWorksheet;

/// Viewer slots (`Option` per sheet) or a dense edit-host grid without cloning for paint.
#[derive(Clone, Copy)]
pub enum AllSheetsRef<'a> {
    Sparse(&'a [Option<ActiveWorksheet>]),
    Dense(&'a [ActiveWorksheet]),
}

impl<'a> AllSheetsRef<'a> {
    pub fn get(&self, index: usize) -> Option<&'a ActiveWorksheet> {
        match self {
            AllSheetsRef::Sparse(s) => s.get(index).and_then(|o| o.as_ref()),
            AllSheetsRef::Dense(s) => s.get(index),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            AllSheetsRef::Sparse(s) => s.is_empty(),
            AllSheetsRef::Dense(s) => s.is_empty(),
        }
    }
}
