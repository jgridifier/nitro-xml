// SPDX-License-Identifier: MIT
//! Xlsx editor UI helpers: formula bar, cell editor, stale marks, live session (#69),
//! and session calculation after edits (#77).
//!
//! The live [`EditorSession`] owns the workbook the ribbon and formula bar mutate.
//! The wasm handle keeps that session and paints from it. Crash recovery stays a
//! loader journal (`TODO(#62)`); this session is in-memory only.

mod cell_editor;
mod commands;
mod formula_bar;
mod input_text;
mod session;
mod session_calc;
mod stale_mark;

pub use cell_editor::CellEditor;
pub use commands::SelectionTarget;
pub use formula_bar::{formula_bar_html, FormulaBarView};
pub use input_text::cell_input_text;
pub use session::{ChromeEffect, EditorSession};
pub use session_calc::{
    formula_cells_json, CalcHint, EditorStep, SessionCalculator, ShiftAxis, StepMode,
};
pub use stale_mark::mark_stale_class;
