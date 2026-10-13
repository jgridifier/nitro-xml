// SPDX-License-Identifier: MIT
//! Stale-formula paint hooks (#68 `stale_cells`, shown on the grid in #69).

/// Append `xg-stale` when `(row, col)` is in the active sheet's stale set.
pub fn mark_stale_class(class: &mut String, stale: bool) {
    if stale {
        class.push_str(" xg-stale");
    }
}
