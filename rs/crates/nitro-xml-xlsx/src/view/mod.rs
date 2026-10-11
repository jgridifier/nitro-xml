// SPDX-License-Identifier: MIT
//! Grid layout and CellItem view-IR (#18).

pub mod axis_metrics;
pub mod cell_items;
pub mod cell_paint;
pub mod cell_view;
pub mod colors;
pub mod css_font_family;
pub mod fake_measure;
pub mod metrics;
pub mod number_fit;
pub mod sheet_queries;
pub mod sparkline_data;
pub mod sparkline_view;
pub mod style_view;
pub mod tint;
pub mod types;
pub mod units;
pub mod view_ir;
pub mod viewport;

pub use view_ir::{dump_xlsx_view_ir, dump_xlsx_view_ir_string};
