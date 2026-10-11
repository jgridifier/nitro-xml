// SPDX-License-Identifier: Apache-2.0
// Ported from ChristopherVR/ooxml@e66aaec src/core/xlsx/display-text.ts; changes listed in PROVENANCE.md.

use crate::model::{RichTextRun, WorkbookSlice};
use crate::numfmt::{format_value, CellValue, FormatOptions};
use crate::styles::style_at;

/// Minimal cell view for formatted display text (worksheet columnar store is #16).
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub value: CellValue,
    /// Index into [`WorkbookSlice::styles`]; absent means style 0.
    pub style_id: Option<u32>,
    /// Rich text runs for a string value with mixed formatting.
    pub rich_text: Option<Vec<RichTextRun>>,
}

/// The formatted text used by value filters and editing commands.
pub fn display_text(workbook: &WorkbookSlice, cell: Option<&Cell>) -> String {
    let Some(cell) = cell else {
        return String::new();
    };
    match &cell.value {
        CellValue::Blank => String::new(),
        CellValue::Error(error) => error.clone(),
        CellValue::Bool(true) => "TRUE".into(),
        CellValue::Bool(false) => "FALSE".into(),
        CellValue::Text(text) if cell.rich_text.is_some() => text.clone(),
        other => {
            let style = style_at(&workbook.styles, cell.style_id);
            format_value(
                other,
                &style.num_fmt,
                &FormatOptions {
                    date1904: workbook.date1904,
                    width: None,
                    measure: None,
                },
            )
            .text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::numfmt::CellValue;
    use crate::workbook::default_cell_style;

    fn book_with_fmt(num_fmt: &str) -> WorkbookSlice {
        let mut style = default_cell_style();
        style.num_fmt = num_fmt.into();
        WorkbookSlice {
            format: "xlsx".into(),
            date1904: false,
            active_sheet: 0,
            sheets: vec![],
            defined_names: vec![],
            theme: crate::workbook::default_theme(),
            styles: vec![default_cell_style(), style],
            named_styles: vec![],
            xf_map: vec![0, 1],
            palette: None,
            dxfs: vec![],
            shared_strings: vec![],
            warnings: vec![],
            full_calc_on_load: None,
            structure_locked: None,
            calc_mode: None,
            iterate: None,
            workbook_password_hash: None,
            workbook_modern_hash: None,
            active_worksheet: None,
        }
    }

    #[test]
    fn rich_text_skips_number_format() {
        // Catches: a rich-text string must return the raw value, not a General-formatted string.
        let book = book_with_fmt("0.00");
        let cell = Cell {
            value: CellValue::Text("001".into()),
            style_id: Some(1),
            rich_text: Some(vec![RichTextRun {
                text: "001".into(),
                font: None,
            }]),
        };
        assert_eq!(display_text(&book, Some(&cell)), "001");
    }
}
