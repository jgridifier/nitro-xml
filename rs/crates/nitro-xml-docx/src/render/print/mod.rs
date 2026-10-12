// SPDX-License-Identifier: MIT
//! Print Layout painter (D1–D7): one DOM command buffer per frame.

mod blocks;
mod chart_cache;
mod dom;
mod header_footer;
mod line_numbers;
mod load;
mod options;
mod page_borders;
mod pages;
mod picture;
mod styles;
mod table;

pub use dom::StringIntern;
pub use line_numbers::{
    line_numbering_from_section, suppressed_line_number_ids, PrintLineNumbering,
};
pub use load::PrintSession;
pub use options::{PrintLayoutOptions, PrintNow};
pub use page_borders::{page_borders_from_section, PageBorders};
pub use pages::paint_print_layout;
